use std::io;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub mod client;
pub mod server;

/// Maximum frame size: 16 MB
const MAX_FRAME_SIZE: u32 = 16 * 1024 * 1024;

/// Frame type markers (first byte of payload)
pub(crate) const FRAME_MESSAGE: u8 = 0x00;
pub(crate) const FRAME_PING: u8 = 0x01;
pub(crate) const FRAME_PONG: u8 = 0x02;

/// Write a length-prefixed frame to the writer.
///
/// Frame format: [u32 LE: payload_length][payload bytes]
pub(crate) async fn write_frame(writer: &mut (impl AsyncWriteExt + Unpin), data: &[u8]) -> io::Result<()> {
    writer.write_u32_le(data.len() as u32).await?;
    writer.write_all(data).await?;
    Ok(())
}

/// Read a length-prefixed frame from the reader.
///
/// Frame format: [u32 LE: payload_length][payload bytes]
pub(crate) async fn read_frame(reader: &mut (impl AsyncReadExt + Unpin)) -> io::Result<Vec<u8>> {
    let len = reader.read_u32_le().await?;
    if len > MAX_FRAME_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("frame size {} exceeds maximum {}", len, MAX_FRAME_SIZE),
        ));
    }
    let mut buf = vec![0u8; len as usize];
    reader.read_exact(&mut buf).await?;
    Ok(buf)
}
