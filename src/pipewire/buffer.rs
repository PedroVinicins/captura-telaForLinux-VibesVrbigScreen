use pipewire::buffer::Buffer;
use crate::frame::Frame;

pub fn extract_frame(
    buffer: &mut Buffer,
    width: u32,
    height: u32,
) -> Option<Frame> {
    let datas = buffer.datas_mut();

    if datas.is_empty() {
        return None;
    }

    let data = &mut datas[0];

    let _chunk = data.chunk();

    let bytes = data.data()?;

    if bytes.is_empty() {
        return None;
    }

    Some(Frame::new(
        width,
        height,
        bytes.to_vec(),
    ))
}