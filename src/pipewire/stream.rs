use std::{
    io::Cursor,
    os::fd::OwnedFd,
    sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
    time::Instant,
};

use ::pipewire as pw;
use pw::{
    context::ContextRc,
    loop_::Timeout,
    main_loop::MainLoopRc,
    properties::properties,
    spa::{
        param::{
            format::{MediaSubtype, MediaType},
            video::{VideoFormat, VideoInfoRaw},
            ParamType,
        },
        pod::{Pod, Value},
        utils::{Direction, Fraction, Rectangle, SpaTypes},
    },
    stream::{StreamFlags, StreamListener, StreamRc},
};
use tracing::{error, info, warn};

use crate::{
    error::{CaptureError, Result},
    frame::Frame,
};

const BYTES_PER_PIXEL: usize = 4;
const MAX_FRAME_WIDTH: u32 = 8192;
const MAX_FRAME_HEIGHT: u32 = 8192;
// Limita a alocação de um frame a 128 MiB. O portal é uma fronteira externa:
// metadados incorretos não devem conseguir esgotar a memória do processo.
const MAX_FRAME_BYTES: usize = 128 * 1024 * 1024;

struct StreamData {
    sender: SyncSender<Frame>,
    format: VideoInfoRaw,
    frame_number: u64,
    started_at: Instant,
    receiver_closed: bool,
    warned_unmapped: bool,
    warned_bad_frame: bool,
}

pub struct PipeWireStream {
    // A ordem é importante: listener -> stream -> main loop.
    _listener: StreamListener<StreamData>,
    stream: StreamRc,
    mainloop: MainLoopRc,
    receiver: Receiver<Frame>,
    width: u32,
    height: u32,
    stopped: bool,
}

impl PipeWireStream {
    pub fn new(
        fd: OwnedFd,
        node_id: u32,
        requested_width: u32,
        requested_height: u32,
        fps: u32,
    ) -> Result<Self> {
        pw::init();

        if fps == 0 {
            return Err(CaptureError::Frame(
                "FPS deve ser maior que zero".to_string(),
            ));
        }

        if requested_width > MAX_FRAME_WIDTH || requested_height > MAX_FRAME_HEIGHT {
            return Err(CaptureError::Frame(format!(
                "resolução solicitada excede o limite de {MAX_FRAME_WIDTH}x{MAX_FRAME_HEIGHT}: \
                 {requested_width}x{requested_height}"
            )));
        }

        // Alguns portais não informam o tamanho no retorno de `Start`.
        // Use uma resolução inicial sensata sem limitar a negociação posterior.
        let width = if requested_width == 0 {
            1920
        } else {
            requested_width
        };
        let height = if requested_height == 0 {
            1080
        } else {
            requested_height
        };
        let mainloop = MainLoopRc::new(None)?;
        let context = ContextRc::new(&mainloop, None)?;
        let core = context.connect_fd_rc(fd, None)?;

        let stream = StreamRc::new(
            core,
            "vibesvr-screen-capture",
            properties! {
                *pw::keys::MEDIA_TYPE => "Video",
                *pw::keys::MEDIA_CATEGORY => "Capture",
                *pw::keys::MEDIA_ROLE => "Screen",
            },
        )?;

        let (sender, receiver) = mpsc::sync_channel(2);
        let user_data = StreamData {
            sender,
            format: VideoInfoRaw::default(),
            frame_number: 0,
            started_at: Instant::now(),
            receiver_closed: false,
            warned_unmapped: false,
            warned_bad_frame: false,
        };

        let listener = stream
            .add_local_listener_with_user_data(user_data)
            .state_changed(|_, _, old, new| {
                info!(?old, ?new, "Estado do stream PipeWire");
            })
            .param_changed(|_, state, id, param| {
                if id != ParamType::Format.as_raw() {
                    return;
                }

                let Some(param) = param else {
                    return;
                };

                let Ok((media_type, media_subtype)) =
                    pw::spa::param::format_utils::parse_format(param)
                else {
                    error!("Não foi possível interpretar o formato do stream");
                    return;
                };

                if media_type != MediaType::Video || media_subtype != MediaSubtype::Raw {
                    error!(?media_type, ?media_subtype, "O stream não contém vídeo RAW");
                    return;
                }

                if let Err(error) = state.format.parse(param) {
                    error!(?error, "Falha ao interpretar VideoInfoRaw");
                    return;
                }

                let size = state.format.size();
                info!(
                    width = size.width,
                    height = size.height,
                    format = ?state.format.format(),
                    "Formato de captura negociado"
                );
            })
            .process(|stream, state| {
                if state.receiver_closed {
                    return;
                }

                let size = state.format.size();
                let width = size.width;
                let height = size.height;

                if width == 0 || height == 0 {
                    return;
                }

                let Some(mut buffer) = stream.dequeue_buffer() else {
                    return;
                };

                let datas = buffer.datas_mut();
                let Some(plane) = datas.first_mut() else {
                    return;
                };

                let (offset, chunk_size, stride) = {
                    let chunk = plane.chunk();
                    (
                        chunk.offset() as usize,
                        chunk.size() as usize,
                        chunk.stride(),
                    )
                };

                let Some(mapped) = plane.data() else {
                    if !state.warned_unmapped {
                        state.warned_unmapped = true;
                        warn!("Buffer não mapeado; o compositor pode ter enviado DMA-BUF");
                    }
                    return;
                };

                if offset >= mapped.len() || chunk_size == 0 {
                    return;
                }

                let available = chunk_size.min(mapped.len() - offset);
                let source = &mapped[offset..offset + available];

                let rgba =
                    match convert_to_rgba(state.format.format(), width, height, stride, source) {
                        Ok(rgba) => rgba,
                        Err(message) => {
                            if !state.warned_bad_frame {
                                state.warned_bad_frame = true;
                                warn!(%message, "Frame PipeWire inválido; avisos repetidos serão omitidos");
                            }
                            return;
                        }
                    };

                state.warned_bad_frame = false;

                state.frame_number = state.frame_number.saturating_add(1);
                let frame = Frame::with_metadata(
                    width,
                    height,
                    rgba,
                    state.started_at.elapsed(),
                    state.frame_number,
                );

                match state.sender.try_send(frame) {
                    Ok(()) | Err(TrySendError::Full(_)) => {}
                    Err(TrySendError::Disconnected(_)) => {
                        state.receiver_closed = true;
                    }
                }
            })
            .register()?;

        let format_object = pw::spa::pod::object!(
            SpaTypes::ObjectParamFormat,
            ParamType::EnumFormat,
            pw::spa::pod::property!(
                pw::spa::param::format::FormatProperties::MediaType,
                Id,
                MediaType::Video
            ),
            pw::spa::pod::property!(
                pw::spa::param::format::FormatProperties::MediaSubtype,
                Id,
                MediaSubtype::Raw
            ),
            pw::spa::pod::property!(
                pw::spa::param::format::FormatProperties::VideoFormat,
                Choice,
                Enum,
                Id,
                VideoFormat::BGRx,
                VideoFormat::BGRx,
                VideoFormat::BGRA,
                VideoFormat::RGBx,
                VideoFormat::RGBA,
                VideoFormat::xRGB,
                VideoFormat::xBGR,
                VideoFormat::ARGB,
                VideoFormat::ABGR
            ),
            pw::spa::pod::property!(
                pw::spa::param::format::FormatProperties::VideoSize,
                Choice,
                Range,
                Rectangle,
                Rectangle { width, height },
                Rectangle {
                    width: 1,
                    height: 1
                },
                Rectangle {
                    width: 8192,
                    height: 8192
                }
            ),
            pw::spa::pod::property!(
                pw::spa::param::format::FormatProperties::VideoFramerate,
                Choice,
                Range,
                Fraction,
                Fraction { num: fps, denom: 1 },
                Fraction { num: 0, denom: 1 },
                Fraction { num: 240, denom: 1 }
            ),
        );

        let values = pw::spa::pod::serialize::PodSerializer::serialize(
            Cursor::new(Vec::new()),
            &Value::Object(format_object),
        )
        .map_err(|error| CaptureError::Spa(format!("falha ao serializar formato: {error:?}")))?
        .0
        .into_inner();

        let pod = Pod::from_bytes(&values)
            .ok_or_else(|| CaptureError::Spa("POD de formato inválido".into()))?;
        let mut params = [pod];

        stream.connect(
            Direction::Input,
            Some(node_id),
            StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS,
            &mut params,
        )?;

        Ok(Self {
            _listener: listener,
            stream,
            mainloop,
            receiver,
            width,
            height,
            stopped: false,
        })
    }

    pub fn try_receive_frame(&mut self) -> Option<Frame> {
        if self.stopped {
            return None;
        }

        // Processa todos os eventos PipeWire já disponíveis sem bloquear o winit.
        for _ in 0..8 {
            if self.mainloop.loop_().iterate(Timeout::None) <= 0 {
                break;
            }
        }

        let mut latest = None;

        loop {
            match self.receiver.try_recv() {
                Ok(frame) => latest = Some(frame),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.stopped = true;
                    break;
                }
            }
        }

        if let Some(frame) = latest.as_ref() {
            self.width = frame.width();
            self.height = frame.height();
        }

        latest
    }

    pub fn stop(&mut self) {
        if self.stopped {
            return;
        }

        if let Err(error) = self.stream.disconnect() {
            warn!(%error, "Falha ao desconectar o stream PipeWire");
        }
        self.stopped = true;
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }
}

fn convert_to_rgba(
    format: VideoFormat,
    width: u32,
    height: u32,
    stride: i32,
    source: &[u8],
) -> std::result::Result<Vec<u8>, String> {
    if width == 0 || height == 0 {
        return Err("dimensões do frame não podem ser zero".to_string());
    }
    if width > MAX_FRAME_WIDTH || height > MAX_FRAME_HEIGHT {
        return Err(format!(
            "dimensões do frame excedem o limite: {width}x{height}"
        ));
    }

    let width =
        usize::try_from(width).map_err(|_| "largura do frame excede a plataforma".to_string())?;
    let height =
        usize::try_from(height).map_err(|_| "altura do frame excede a plataforma".to_string())?;
    let row_bytes = width
        .checked_mul(BYTES_PER_PIXEL)
        .ok_or_else(|| "largura do frame excede o limite".to_string())?;
    let output_len = row_bytes
        .checked_mul(height)
        .ok_or_else(|| "tamanho do frame excede o limite".to_string())?;
    if output_len > MAX_FRAME_BYTES {
        return Err(format!(
            "frame de {output_len} bytes excede o limite de {MAX_FRAME_BYTES} bytes"
        ));
    }

    let source_stride = if stride == 0 {
        row_bytes
    } else {
        stride
            .checked_abs()
            .ok_or_else(|| "stride inválido".to_string())? as usize
    };

    if source_stride < row_bytes {
        return Err(format!(
            "stride {source_stride} menor que a linha RGBA {row_bytes}"
        ));
    }

    let required = height
        .saturating_sub(1)
        .checked_mul(source_stride)
        .and_then(|value| value.checked_add(row_bytes))
        .ok_or_else(|| "tamanho do buffer excede o limite".to_string())?;

    if source.len() < required {
        return Err(format!(
            "frame incompleto: recebidos {} bytes, necessários {required}",
            source.len()
        ));
    }

    // Um único buffer e escritas indexadas evitam milhões de chamadas a
    // `extend_from_slice` por frame, que são especialmente caras em debug.
    let mut rgba = vec![0_u8; output_len];

    for output_y in 0..height {
        let source_y = if stride < 0 {
            height - 1 - output_y
        } else {
            output_y
        };
        let row_start = source_y * source_stride;
        let source_row = &source[row_start..row_start + row_bytes];
        let output_start = output_y * row_bytes;
        let output_row = &mut rgba[output_start..output_start + row_bytes];

        match format {
            VideoFormat::RGBA => output_row.copy_from_slice(source_row),
            VideoFormat::RGBx => {
                for (source_pixel, output_pixel) in source_row
                    .chunks_exact(4)
                    .zip(output_row.chunks_exact_mut(4))
                {
                    output_pixel.copy_from_slice(&[
                        source_pixel[0],
                        source_pixel[1],
                        source_pixel[2],
                        255,
                    ]);
                }
            }
            VideoFormat::BGRA => {
                for (source_pixel, output_pixel) in source_row
                    .chunks_exact(4)
                    .zip(output_row.chunks_exact_mut(4))
                {
                    output_pixel.copy_from_slice(&[
                        source_pixel[2],
                        source_pixel[1],
                        source_pixel[0],
                        source_pixel[3],
                    ]);
                }
            }
            VideoFormat::BGRx => {
                for (source_pixel, output_pixel) in source_row
                    .chunks_exact(4)
                    .zip(output_row.chunks_exact_mut(4))
                {
                    output_pixel.copy_from_slice(&[
                        source_pixel[2],
                        source_pixel[1],
                        source_pixel[0],
                        255,
                    ]);
                }
            }
            VideoFormat::ARGB => {
                for (source_pixel, output_pixel) in source_row
                    .chunks_exact(4)
                    .zip(output_row.chunks_exact_mut(4))
                {
                    output_pixel.copy_from_slice(&[
                        source_pixel[1],
                        source_pixel[2],
                        source_pixel[3],
                        source_pixel[0],
                    ]);
                }
            }
            VideoFormat::ABGR => {
                for (source_pixel, output_pixel) in source_row
                    .chunks_exact(4)
                    .zip(output_row.chunks_exact_mut(4))
                {
                    output_pixel.copy_from_slice(&[
                        source_pixel[3],
                        source_pixel[2],
                        source_pixel[1],
                        source_pixel[0],
                    ]);
                }
            }
            VideoFormat::xRGB => {
                for (source_pixel, output_pixel) in source_row
                    .chunks_exact(4)
                    .zip(output_row.chunks_exact_mut(4))
                {
                    output_pixel.copy_from_slice(&[
                        source_pixel[1],
                        source_pixel[2],
                        source_pixel[3],
                        255,
                    ]);
                }
            }
            VideoFormat::xBGR => {
                for (source_pixel, output_pixel) in source_row
                    .chunks_exact(4)
                    .zip(output_row.chunks_exact_mut(4))
                {
                    output_pixel.copy_from_slice(&[
                        source_pixel[3],
                        source_pixel[2],
                        source_pixel[1],
                        255,
                    ]);
                }
            }
            unsupported => return Err(format!("formato não suportado: {unsupported:?}")),
        }
    }

    Ok(rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_bgrx_and_ignores_row_padding() {
        let source = [3, 2, 1, 0, 6, 5, 4, 0, 99, 99, 99, 99];
        let result = convert_to_rgba(VideoFormat::BGRx, 2, 1, 12, &source).unwrap();
        assert_eq!(result, [1, 2, 3, 255, 4, 5, 6, 255]);
    }

    #[test]
    fn handles_negative_stride_as_bottom_up() {
        let source = [30, 20, 10, 255, 60, 50, 40, 255];
        let result = convert_to_rgba(VideoFormat::RGBA, 1, 2, -4, &source).unwrap();
        assert_eq!(result, [60, 50, 40, 255, 30, 20, 10, 255]);
    }

    #[test]
    fn rejects_short_and_oversized_frames() {
        assert!(convert_to_rgba(VideoFormat::RGBA, 2, 2, 8, &[0; 15]).is_err());
        assert!(convert_to_rgba(VideoFormat::RGBA, 8192, 8192, 0, &[]).is_err());
    }
}
