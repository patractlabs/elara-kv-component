use std::io::{Result, Write};

use flate2::{write, Compression};

/// Encode for WebSocket data.
pub trait Encoder: Send + Sync {
    fn encode<T: AsRef<[u8]>, W: Write>(&self, input: &T, output: W) -> Result<W>;
}

#[derive(Debug, Copy, Clone)]
pub struct GzipEncoder(Compression);

impl GzipEncoder {
    pub fn new(level: Compression) -> Self {
        Self(level)
    }
}

impl Encoder for GzipEncoder {
    fn encode<T: AsRef<[u8]>, W: Write>(&self, input: &T, output: W) -> Result<W> {
        let mut enc = write::GzEncoder::new(output, self.0);
        enc.write_all(input.as_ref())?;
        enc.finish()
    }
}

#[derive(Debug, Copy, Clone)]
pub struct ZlibEncoder(Compression);

impl ZlibEncoder {
    pub fn new(level: Compression) -> Self {
        Self(level)
    }
}

impl Encoder for ZlibEncoder {
    fn encode<T: AsRef<[u8]>, W: Write>(&self, input: &T, output: W) -> Result<W> {
        let mut enc = write::ZlibEncoder::new(output, self.0);
        enc.write_all(input.as_ref())?;
        enc.finish()
    }
}
