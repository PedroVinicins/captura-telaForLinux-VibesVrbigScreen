use std::{
    env,
    io::{self, IsTerminal, Write},
    slice,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
        Arc,
    },
    time::{Duration, Instant},
};

use core_foundation::base::TCFType;
use core_media::{
    sample_buffer::{CMSampleBuffer, CMSampleBufferRef},
    time::CMTime,
};
use core_video::pixel_buffer::{
    kCVPixelBufferLock_ReadOnly, kCVPixelFormatType_32BGRA, CVPixelBuffer,
};
use dispatch2::{DispatchQueue, DispatchQueueAttr, DispatchRetained};
use objc2::{
    define_class, msg_send, rc::Retained, runtime::ProtocolObject, AnyThread, DefinedClass,
};
use objc2_foundation::{NSError, NSObject, NSObjectProtocol};
use screen_capture_kit::{
    shareable_content::{SCShareableContent, SCWindow},
    stream::{
        SCContentFilter, SCStream, SCStreamConfiguration, SCStreamDelegate, SCStreamOutput,
        SCStreamOutputType,
    },
};
use tracing::{info, warn};

use crate::{
    error::{CaptureError, Result},
    frame::Frame,
};

const MAX_FRAME_WIDTH: u32 = 8192;
const MAX_FRAME_HEIGHT: u32 = 8192;
const MAX_FRAME_BYTES: usize = 128 * 1024 * 1024;
const PERMISSION_TIMEOUT: Duration = Duration::from_secs(120);
const START_TIMEOUT: Duration = Duration::from_secs(15);
const WINDOW_SELECTOR_ENV: &str = "VIBESVR_WINDOW";

struct WindowChoice {
    window: Retained<SCWindow>,
    id: u32,
    app_name: String,
    title: String,
    width: u32,
    height: u32,
}

struct CaptureDelegateIvars {
    sender: SyncSender<Frame>,
    frame_number: AtomicU64,
    started_at: Instant,
    receiver_closed: AtomicBool,
    warned_bad_frame: AtomicBool,
    running: Arc<AtomicBool>,
}

define_class!(
    // SAFETY: NSObject não impõe requisitos adicionais para subclasses e os
    // ivars são válidos durante toda a vida do objeto Objective-C.
    #[unsafe(super(NSObject))]
    #[name = "VibesVRCaptureDelegate"]
    #[ivars = CaptureDelegateIvars]
    struct CaptureDelegate;

    // SAFETY: NSObjectProtocol não acrescenta invariantes.
    unsafe impl NSObjectProtocol for CaptureDelegate {}

    // SAFETY: As assinaturas correspondem aos seletores de SCStreamOutput.
    unsafe impl SCStreamOutput for CaptureDelegate {
        #[unsafe(method(stream:didOutputSampleBuffer:ofType:))]
        unsafe fn stream_did_output_sample_buffer(
            &self,
            _stream: &SCStream,
            sample_buffer: CMSampleBufferRef,
            output_type: SCStreamOutputType,
        ) {
            self.handle_sample_buffer(sample_buffer, output_type);
        }
    }

    // SAFETY: A assinatura corresponde ao seletor de SCStreamDelegate.
    unsafe impl SCStreamDelegate for CaptureDelegate {
        #[unsafe(method(stream:didStopWithError:))]
        unsafe fn stream_did_stop_with_error(&self, _stream: &SCStream, error: &NSError) {
            self.ivars().running.store(false, Ordering::Release);
            warn!(
                error = %error.localizedDescription(),
                "O ScreenCaptureKit interrompeu a captura"
            );
        }
    }
);

impl CaptureDelegate {
    fn new(sender: SyncSender<Frame>, running: Arc<AtomicBool>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(CaptureDelegateIvars {
            sender,
            frame_number: AtomicU64::new(0),
            started_at: Instant::now(),
            receiver_closed: AtomicBool::new(false),
            warned_bad_frame: AtomicBool::new(false),
            running,
        });

        // SAFETY: `this` foi alocado como CaptureDelegate, teve todos os ivars
        // inicializados e NSObject implementa o inicializador padrão.
        unsafe { msg_send![super(this), init] }
    }

    fn handle_sample_buffer(
        &self,
        sample_buffer: CMSampleBufferRef,
        output_type: SCStreamOutputType,
    ) {
        let ivars = self.ivars();
        if output_type != SCStreamOutputType::Screen
            || !ivars.running.load(Ordering::Acquire)
            || ivars.receiver_closed.load(Ordering::Relaxed)
        {
            return;
        }

        // SAFETY: o ScreenCaptureKit mantém o CMSampleBuffer válido durante o
        // callback. A regra get faz uma retenção para o wrapper local.
        let sample = unsafe { CMSampleBuffer::wrap_under_get_rule(sample_buffer) };
        let Some(image_buffer) = sample.get_image_buffer() else {
            return;
        };
        let Some(pixel_buffer) = image_buffer.downcast::<CVPixelBuffer>() else {
            self.warn_once("o sample não contém um CVPixelBuffer");
            return;
        };

        if pixel_buffer.get_pixel_format() != kCVPixelFormatType_32BGRA {
            self.warn_once("o macOS retornou um formato de pixels diferente de BGRA");
            return;
        }

        let lock_status = pixel_buffer.lock_base_address(kCVPixelBufferLock_ReadOnly);
        if lock_status != 0 {
            self.warn_once(&format!(
                "não foi possível bloquear o buffer de pixels (código {lock_status})"
            ));
            return;
        }

        let result = copy_pixel_buffer(&pixel_buffer);
        let unlock_status = pixel_buffer.unlock_base_address(kCVPixelBufferLock_ReadOnly);
        if unlock_status != 0 {
            self.warn_once(&format!(
                "não foi possível desbloquear o buffer de pixels (código {unlock_status})"
            ));
        }

        let (width, height, rgba) = match result {
            Ok(frame) => frame,
            Err(message) => {
                self.warn_once(&message);
                return;
            }
        };

        ivars.warned_bad_frame.store(false, Ordering::Relaxed);
        let frame_number = ivars.frame_number.fetch_add(1, Ordering::Relaxed) + 1;
        let frame = Frame::with_metadata(
            width,
            height,
            rgba,
            ivars.started_at.elapsed(),
            frame_number,
        );

        match ivars.sender.try_send(frame) {
            Ok(()) | Err(TrySendError::Full(_)) => {}
            Err(TrySendError::Disconnected(_)) => {
                ivars.receiver_closed.store(true, Ordering::Relaxed);
            }
        }
    }

    fn warn_once(&self, message: &str) {
        if !self.ivars().warned_bad_frame.swap(true, Ordering::Relaxed) {
            warn!(%message, "Frame do ScreenCaptureKit inválido; avisos repetidos serão omitidos");
        }
    }
}

pub struct ScreenCapture {
    // SCStream usa referências Objective-C aos objetos abaixo. Mantê-los no
    // mesmo owner deixa explícita a ordem de vida de todo o pipeline nativo.
    stream: Retained<SCStream>,
    _delegate: Retained<CaptureDelegate>,
    _queue: DispatchRetained<DispatchQueue>,
    receiver: Receiver<Frame>,
    running: Arc<AtomicBool>,
}

impl ScreenCapture {
    pub fn new(fps: u32) -> Result<Self> {
        let fps_timescale = i32::try_from(fps)
            .ok()
            .filter(|fps| *fps > 0)
            .ok_or_else(|| CaptureError::Frame("FPS deve ser maior que zero".to_string()))?;

        let content = request_shareable_content()?;
        let choice = select_window(&content)?;
        validate_dimensions(choice.width, choice.height)?;

        let filter = SCContentFilter::init_with_desktop_independent_window(
            SCContentFilter::alloc(),
            &choice.window,
        );
        let configuration = SCStreamConfiguration::new();
        configuration.set_width(choice.width as usize);
        configuration.set_height(choice.height as usize);
        configuration.set_minimum_frame_interval(CMTime::make(1, fps_timescale));
        configuration.set_pixel_format(kCVPixelFormatType_32BGRA);
        configuration.set_queue_depth(3);
        configuration.set_scales_to_fit(true);
        // screen-capture-kit 0.7.1 envia por engano `setShowCursor:`. A
        // propriedade pública do ScreenCaptureKit se chama `showsCursor`, logo
        // o setter Objective-C correto é `setShowsCursor:`.
        // SAFETY: SCStreamConfiguration declara a propriedade BOOL showsCursor
        // desde macOS 12.3, que é também a versão mínima deste aplicativo.
        unsafe {
            let _: () = msg_send![&*configuration, setShowsCursor: false];
        }

        let (sender, receiver) = mpsc::sync_channel(2);
        let running = Arc::new(AtomicBool::new(true));
        let delegate = CaptureDelegate::new(sender, Arc::clone(&running));
        let stream_delegate: &ProtocolObject<dyn SCStreamDelegate> =
            ProtocolObject::from_ref(&*delegate);
        let stream =
            SCStream::init_with_filter(SCStream::alloc(), &filter, &configuration, stream_delegate);
        let queue = DispatchQueue::new("io.vibesvr.screen-capture", DispatchQueueAttr::SERIAL);
        let stream_output: &ProtocolObject<dyn SCStreamOutput> =
            ProtocolObject::from_ref(&*delegate);
        stream
            .add_stream_output(stream_output, SCStreamOutputType::Screen, &queue)
            .map_err(|error| {
                CaptureError::ScreenCaptureKit(format!(
                    "o macOS recusou o callback de vídeo: {}",
                    error.localizedDescription()
                ))
            })?;

        wait_for_capture_start(&stream).inspect_err(|_| {
            running.store(false, Ordering::Release);
        })?;

        info!(
            window_id = choice.id,
            application = %choice.app_name,
            title = %choice.title,
            width = choice.width,
            height = choice.height,
            fps,
            "ScreenCaptureKit conectado"
        );

        Ok(Self {
            stream,
            _delegate: delegate,
            _queue: queue,
            receiver,
            running,
        })
    }

    pub fn try_receive_frame(&mut self) -> Option<Frame> {
        if !self.running.load(Ordering::Acquire) {
            return None;
        }

        let mut latest = None;
        loop {
            match self.receiver.try_recv() {
                Ok(frame) => latest = Some(frame),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.running.store(false, Ordering::Release);
                    break;
                }
            }
        }

        latest
    }

    pub fn stop(&mut self) {
        if !self.running.swap(false, Ordering::AcqRel) {
            return;
        }

        self.stream.stop_capture(|error| {
            if let Some(error) = error {
                warn!(
                    error = %error.localizedDescription(),
                    "Falha ao parar o stream do ScreenCaptureKit"
                );
            }
        });
        info!("Captura parada");
    }
}

impl Drop for ScreenCapture {
    fn drop(&mut self) {
        self.stop();
    }
}

fn request_shareable_content() -> Result<Retained<SCShareableContent>> {
    let (sender, receiver) = mpsc::sync_channel(1);
    SCShareableContent::get_shareable_content_excluding_desktop_windows(
        true,
        true,
        move |content, error| {
            let result = content.ok_or_else(|| {
                error.map_or_else(
                    || "o macOS não informou a causa".to_string(),
                    |error| error.localizedDescription().to_string(),
                )
            });
            let _ = sender.send(result);
        },
    );

    receiver
        .recv_timeout(PERMISSION_TIMEOUT)
        .map_err(|_| {
            CaptureError::ScreenCaptureKit(
                "o macOS não respondeu ao pedido para listar janelas".to_string(),
            )
        })?
        .map_err(|error| {
            CaptureError::ScreenCaptureKit(format!(
                "não foi possível listar as janelas. Autorize a Gravação da Tela em Ajustes do \
                 Sistema > Privacidade e Segurança e abra o programa novamente: {error}"
            ))
        })
}

fn wait_for_capture_start(stream: &SCStream) -> Result<()> {
    let (sender, receiver) = mpsc::sync_channel(1);
    stream.start_capture(move |error| {
        let result = error.map_or_else(
            || Ok(()),
            |error| Err(error.localizedDescription().to_string()),
        );
        let _ = sender.send(result);
    });

    receiver
        .recv_timeout(START_TIMEOUT)
        .map_err(|_| {
            CaptureError::ScreenCaptureKit(
                "o macOS não respondeu ao pedido para iniciar a captura".to_string(),
            )
        })?
        .map_err(|error| {
            CaptureError::ScreenCaptureKit(format!(
                "não foi possível iniciar a captura da janela: {error}"
            ))
        })
}

fn select_window(content: &SCShareableContent) -> Result<WindowChoice> {
    let choices: Vec<WindowChoice> = content
        .windows()
        .to_vec()
        .into_iter()
        .filter_map(|window| {
            let frame = window.frame();
            let title = window.title()?.to_string().trim().to_string();
            if !window.on_screen()
                || window.window_layer() != 0
                || title.is_empty()
                || frame.size.width < 64.0
                || frame.size.height < 64.0
            {
                return None;
            }

            let app_name = window
                .owning_application()?
                .application_name()
                .to_string()
                .trim()
                .to_string();
            let width = dimension_from_points(frame.size.width)?;
            let height = dimension_from_points(frame.size.height)?;

            Some(WindowChoice {
                id: window.window_id(),
                window,
                app_name,
                title,
                width,
                height,
            })
        })
        .collect();

    if choices.is_empty() {
        return Err(CaptureError::ScreenCaptureKit(
            "nenhuma janela visível pode ser capturada".to_string(),
        ));
    }

    if let Ok(query) = env::var(WINDOW_SELECTOR_ENV) {
        return select_window_from_query(choices, &query);
    }

    if !io::stdin().is_terminal() {
        return Err(CaptureError::ScreenCaptureKit(format!(
            "a seleção interativa precisa de um terminal; defina {WINDOW_SELECTOR_ENV} com o \
             título, aplicativo ou ID da janela"
        )));
    }

    println!("Selecione a janela que será exibida no VibesVR:\n");
    for (index, choice) in choices.iter().enumerate() {
        println!(
            "  {:>2}. {} — {} ({}x{}, ID {})",
            index + 1,
            choice.app_name,
            choice.title,
            choice.width,
            choice.height,
            choice.id
        );
    }

    loop {
        print!("\nNúmero da janela: ");
        io::stdout().flush()?;

        let mut input = String::new();
        if io::stdin().read_line(&mut input)? == 0 {
            return Err(CaptureError::ScreenCaptureKit(
                "a seleção de janela foi cancelada".to_string(),
            ));
        }

        if let Ok(index) = input.trim().parse::<usize>() {
            if (1..=choices.len()).contains(&index) {
                return Ok(choices.into_iter().nth(index - 1).expect("índice validado"));
            }
        }

        eprintln!("Escolha um número entre 1 e {}.", choices.len());
    }
}

fn select_window_from_query(choices: Vec<WindowChoice>, query: &str) -> Result<WindowChoice> {
    let query = query.trim();
    if query.is_empty() {
        return Err(CaptureError::ScreenCaptureKit(format!(
            "{WINDOW_SELECTOR_ENV} não pode estar vazio"
        )));
    }

    if let Ok(id) = query.parse::<u32>() {
        return choices
            .into_iter()
            .find(|choice| choice.id == id)
            .ok_or_else(|| {
                CaptureError::ScreenCaptureKit(format!("nenhuma janela visível possui o ID {id}"))
            });
    }

    let query_lowercase = query.to_lowercase();
    choices
        .into_iter()
        .find(|choice| {
            choice.title.eq_ignore_ascii_case(query)
                || choice.app_name.eq_ignore_ascii_case(query)
                || choice.title.to_lowercase().contains(&query_lowercase)
                || choice.app_name.to_lowercase().contains(&query_lowercase)
        })
        .ok_or_else(|| {
            CaptureError::ScreenCaptureKit(format!(
                "nenhuma janela visível corresponde a {WINDOW_SELECTOR_ENV}={query:?}"
            ))
        })
}

fn copy_pixel_buffer(
    pixel_buffer: &CVPixelBuffer,
) -> std::result::Result<(u32, u32, Vec<u8>), String> {
    if pixel_buffer.is_planar() {
        return Err("o macOS retornou um buffer de pixels planar".to_string());
    }

    let width = u32::try_from(pixel_buffer.get_width())
        .map_err(|_| "a largura do frame excede a plataforma".to_string())?;
    let height = u32::try_from(pixel_buffer.get_height())
        .map_err(|_| "a altura do frame excede a plataforma".to_string())?;
    validate_dimensions(width, height).map_err(|error| error.to_string())?;

    let source_stride = pixel_buffer.get_bytes_per_row();
    let required = source_stride
        .checked_mul(height as usize)
        .ok_or_else(|| "o buffer de origem excede a plataforma".to_string())?;
    if pixel_buffer.get_data_size() < required {
        return Err(format!(
            "buffer BGRA truncado: {} bytes recebidos, {required} necessários",
            pixel_buffer.get_data_size()
        ));
    }

    // SAFETY: o endereço foi bloqueado pelo chamador e permanece válido até
    // unlock_base_address. O tamanho foi validado contra get_data_size.
    let base_address = unsafe { pixel_buffer.get_base_address() }.cast::<u8>();
    if base_address.is_null() {
        return Err("o buffer de pixels não possui endereço base".to_string());
    }
    let source = unsafe { slice::from_raw_parts(base_address, required) };
    let rgba = bgra_to_rgba(width, height, source_stride, source)?;

    Ok((width, height, rgba))
}

fn dimension_from_points(value: f64) -> Option<u32> {
    if value.is_finite() && value >= 1.0 && value <= f64::from(u32::MAX) {
        Some(value.round() as u32)
    } else {
        None
    }
}

fn validate_dimensions(width: u32, height: u32) -> Result<()> {
    if width == 0 || height == 0 {
        return Err(CaptureError::Frame(
            "as dimensões da janela não podem ser zero".to_string(),
        ));
    }
    if width > MAX_FRAME_WIDTH || height > MAX_FRAME_HEIGHT {
        return Err(CaptureError::Frame(format!(
            "a janela excede o limite de {MAX_FRAME_WIDTH}x{MAX_FRAME_HEIGHT}: {width}x{height}"
        )));
    }

    let bytes = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| CaptureError::Frame("o tamanho do frame excede a plataforma".to_string()))?;
    if bytes > MAX_FRAME_BYTES {
        return Err(CaptureError::Frame(format!(
            "o frame de {bytes} bytes excede o limite de {MAX_FRAME_BYTES} bytes"
        )));
    }

    Ok(())
}

fn bgra_to_rgba(
    width: u32,
    height: u32,
    source_stride: usize,
    source: &[u8],
) -> std::result::Result<Vec<u8>, String> {
    validate_dimensions(width, height).map_err(|error| error.to_string())?;

    let width = width as usize;
    let height = height as usize;
    let row_bytes = width
        .checked_mul(4)
        .ok_or_else(|| "a largura do frame excede a plataforma".to_string())?;
    if source_stride < row_bytes {
        return Err(format!(
            "stride {source_stride} menor que a linha BGRA de {row_bytes} bytes"
        ));
    }
    let required = source_stride
        .checked_mul(height)
        .ok_or_else(|| "o buffer de origem excede a plataforma".to_string())?;
    if source.len() < required {
        return Err(format!(
            "buffer BGRA truncado: {} bytes recebidos, {required} necessários",
            source.len()
        ));
    }

    let output_len = row_bytes
        .checked_mul(height)
        .ok_or_else(|| "o frame de saída excede a plataforma".to_string())?;
    let mut rgba = vec![0; output_len];

    for row in 0..height {
        let source_start = row * source_stride;
        let destination_start = row * row_bytes;
        let source_row = &source[source_start..source_start + row_bytes];
        let destination_row = &mut rgba[destination_start..destination_start + row_bytes];

        for (bgra, rgba) in source_row
            .chunks_exact(4)
            .zip(destination_row.chunks_exact_mut(4))
        {
            rgba.copy_from_slice(&[bgra[2], bgra[1], bgra[0], bgra[3]]);
        }
    }

    Ok(rgba)
}

#[cfg(test)]
mod tests {
    use super::bgra_to_rgba;

    #[test]
    fn converts_bgra_and_ignores_row_padding() {
        let source = [
            30, 20, 10, 255, 60, 50, 40, 128, 99, 99, 99, 99, 90, 80, 70, 64, 120, 110, 100, 0, 88,
            88, 88, 88,
        ];

        let converted = bgra_to_rgba(2, 2, 12, &source).expect("frame válido");

        assert_eq!(
            converted,
            [10, 20, 30, 255, 40, 50, 60, 128, 70, 80, 90, 64, 100, 110, 120, 0]
        );
    }

    #[test]
    fn rejects_truncated_or_short_stride_buffers() {
        assert!(bgra_to_rgba(2, 1, 7, &[0; 8]).is_err());
        assert!(bgra_to_rgba(2, 2, 8, &[0; 8]).is_err());
    }
}
