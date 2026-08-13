use std::{sync::Arc, time::Duration};

#[derive(Clone, Debug)]
pub struct Frame {
    width: u32,
    height: u32,
    stride: u32,
    data: Arc<[u8]>,
    timestamp: Duration,
    frame_number: u64,
}

impl Frame {
    pub fn new(width: u32, height: u32, data: Vec<u8>) -> Self {
        Self::with_metadata(width, height, data, Duration::ZERO, 0)
    }

    pub fn with_metadata(
        width: u32,
        height: u32,
        data: Vec<u8>,
        timestamp: Duration,
        frame_number: u64,
    ) -> Self {
        Self {
            width,
            height,
            stride: width.saturating_mul(4),
            data: Arc::from(data),
            timestamp,
            frame_number,
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn stride(&self) -> u32 {
        self.stride
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn timestamp(&self) -> Duration {
        self.timestamp
    }

    pub fn frame_number(&self) -> u64 {
        self.frame_number
    }

    pub fn size_bytes(&self) -> usize {
        self.data.len()
    }
}
