use sha2::{Digest, Sha512};
use std::io::{self, Read};

pub const HASH_LEN: usize = 64;
pub type HashResult = [u8; HASH_LEN];

pub struct StreamHasher<R: Read> {
    hasher: Sha512,
    input: R,
}

impl<R: Read> StreamHasher<R> {
    pub fn new(input: R) -> Self {
        StreamHasher {
            hasher: Sha512::default(),
            input,
        }
    }

    pub fn get_result(self) -> HashResult {
        let mut result = [0; HASH_LEN];
        result.copy_from_slice(self.hasher.finalize().as_slice());
        result
    }
}

impl<R: Read> Read for StreamHasher<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let size = self.input.read(buf)?;
        self.hasher.update(&buf[..size]);
        Ok(size)
    }
}
