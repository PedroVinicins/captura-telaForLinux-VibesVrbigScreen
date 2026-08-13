use anyhow::{ensure, Context, Result};
use openh264::{
    encoder::{BitRate, Encoder, EncoderConfig, FrameRate, UsageType},
    formats::YUVSlices,
    OpenH264API,
};

use crate::frame::Frame;

/// Codifica frames RGBA em um fluxo H.264 Annex B.
pub struct H264Encoder {
    encoder: Encoder,
    width: u32,
    height: u32,
    frames: u64,
    i420: Vec<u8>,
}

impl H264Encoder {
    /// `bitrate_kbps` usa quilobits por segundo. Exemplo: `4000` = 4 Mbps.
    pub fn new(
        width: u32,
        height: u32,
        bitrate_kbps: u32,
        fps: u32,
    ) -> Result<Self> {
        ensure!(width > 0 && height > 0, "resolução não pode ser zero");
        ensure!(
            width % 2 == 0 && height % 2 == 0,
            "OpenH264 exige largura e altura pares; recebido {width}x{height}"
        );
        ensure!(bitrate_kbps > 0, "bitrate deve ser maior que zero");
        ensure!(fps > 0, "FPS deve ser maior que zero");

        let bitrate_bps = bitrate_kbps
            .checked_mul(1_000)
            .context("bitrate excede o limite de u32")?;

        let config = EncoderConfig::new()
            .bitrate(BitRate::from_bps(bitrate_bps))
            .max_frame_rate(FrameRate::from_hz(fps as f32))
            .usage_type(UsageType::ScreenContentRealTime);

        let encoder = Encoder::with_api_config(OpenH264API::from_source(), config)
            .context("falha ao criar o encoder OpenH264")?;

        let width_usize = width as usize;
        let height_usize = height as usize;
        let y_len = width_usize
            .checked_mul(height_usize)
            .context("resolução excede o limite da plataforma")?;
        let uv_len = (width_usize / 2)
            .checked_mul(height_usize / 2)
            .context("resolução cromática excede o limite da plataforma")?;
        let i420_len = uv_len
            .checked_mul(2)
            .and_then(|value| y_len.checked_add(value))
            .context("buffer I420 excede o limite da plataforma")?;

        Ok(Self {
            encoder,
            width,
            height,
            frames: 0,
            i420: vec![0; i420_len],
        })
    }

    pub fn encode(&mut self, frame: &Frame) -> Result<Vec<u8>> {
        ensure!(
            frame.width() == self.width && frame.height() == self.height,
            "dimensão do frame {}x{} difere da dimensão do encoder {}x{}",
            frame.width(),
            frame.height(),
            self.width,
            self.height
        );

        let expected_rgba_len = (self.width as usize)
            .checked_mul(self.height as usize)
            .and_then(|pixels| pixels.checked_mul(4))
            .context("tamanho RGBA excede o limite da plataforma")?;

        ensure!(
            frame.data().len() == expected_rgba_len,
            "frame RGBA possui {} bytes; esperados {expected_rgba_len}",
            frame.data().len()
        );

        self.rgba_to_i420(frame.data());

        let width = self.width as usize;
        let height = self.height as usize;
        let y_len = width * height;
        let uv_len = (width / 2) * (height / 2);

        let (y, chroma) = self.i420.split_at(y_len);
        let (u, v) = chroma.split_at(uv_len);
        let source = YUVSlices::new(
            (y, u, v),
            (width, height),
            (width, width / 2, width / 2),
        );

        let encoded = self
            .encoder
            .encode(&source)
            .context("falha ao codificar frame H.264")?
            .to_vec();

        self.frames = self.frames.saturating_add(1);
        Ok(encoded)
    }

    /// Faz o próximo frame ser um I-frame, útil após perda de pacotes.
    pub fn force_keyframe(&mut self) {
        self.encoder.force_intra_frame();
    }

    pub fn frames_encoded(&self) -> u64 {
        self.frames
    }

    fn rgba_to_i420(&mut self, rgba: &[u8]) {
        let width = self.width as usize;
        let height = self.height as usize;
        let y_len = width * height;
        let uv_width = width / 2;
        let uv_height = height / 2;
        let uv_len = uv_width * uv_height;

        let (y_plane, chroma) = self.i420.split_at_mut(y_len);
        let (u_plane, v_plane) = chroma.split_at_mut(uv_len);

        // Conversão BT.601 em faixa limitada, esperada normalmente pelo H.264.
        for y in 0..height {
            for x in 0..width {
                let offset = (y * width + x) * 4;
                let r = i32::from(rgba[offset]);
                let g = i32::from(rgba[offset + 1]);
                let b = i32::from(rgba[offset + 2]);

                y_plane[y * width + x] =
                    (((66 * r + 129 * g + 25 * b + 128) >> 8) + 16)
                        .clamp(0, 255) as u8;
            }
        }

        // I420 usa uma amostra U e V para cada bloco de 2x2 pixels.
        for y in 0..uv_height {
            for x in 0..uv_width {
                let mut r_sum = 0_i32;
                let mut g_sum = 0_i32;
                let mut b_sum = 0_i32;

                for dy in 0..2 {
                    for dx in 0..2 {
                        let source_x = x * 2 + dx;
                        let source_y = y * 2 + dy;
                        let offset = (source_y * width + source_x) * 4;

                        r_sum += i32::from(rgba[offset]);
                        g_sum += i32::from(rgba[offset + 1]);
                        b_sum += i32::from(rgba[offset + 2]);
                    }
                }

                let r = (r_sum + 2) / 4;
                let g = (g_sum + 2) / 4;
                let b = (b_sum + 2) / 4;
                let chroma_offset = y * uv_width + x;

                u_plane[chroma_offset] =
                    (((-38 * r - 74 * g + 112 * b + 128) >> 8) + 128)
                        .clamp(0, 255) as u8;
                v_plane[chroma_offset] =
                    (((112 * r - 94 * g - 18 * b + 128) >> 8) + 128)
                        .clamp(0, 255) as u8;
            }
        }
    }
}