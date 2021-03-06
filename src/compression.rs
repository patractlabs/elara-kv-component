use flate2::write::GzEncoder;
use flate2::Compression;
use once_cell::sync::OnceCell;
use std::io::{Result, Write};

pub trait Encoder: Send + Sync {
    fn encode<T: AsRef<[u8]>, W: Write>(&self, input: &T, output: W) -> Result<W>;
}

#[derive(Debug, Copy, Clone)]
pub struct GzipEncoder(Compression);

static GZIP_ENCODER: OnceCell<GzipEncoder> = OnceCell::new();

impl GzipEncoder {
    pub fn new(level: Compression) -> Self {
        Self(level)
    }

    pub fn global() -> &'static Self {
        GZIP_ENCODER.get().expect("GZIP_ENCODER is not initialized")
    }

    pub fn set_global(enc: GzipEncoder) {
        GZIP_ENCODER
            .set(enc)
            .expect("GZIP_ENCODER is initialized already");
    }
}

impl Encoder for GzipEncoder {
    fn encode<T: AsRef<[u8]>, W: Write>(&self, input: &T, output: W) -> Result<W> {
        let mut enc = GzEncoder::new(output, self.0);
        enc.write_all(input.as_ref()).expect("encode gzip data");
        enc.finish()
    }
}

