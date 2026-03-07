use crate::text::num::{S3NumsReader, S3NumsWriter, S3RevNumsWriter};
use crate::text::s3::{RngMinimal, S3Reader, S3Writer, S3WriterInfo, S3WriterRand};
use crate::text::str_reader::StrReadWraper;
use crate::text::str_writer::WriteExt;

#[derive(Clone)]
pub struct IdWriter {
    prefix_wr: S3NumsWriter,
    prefix: u64,
    hide_wr: S3RevNumsWriter,
    postfix_len: u8,
}

impl IdWriter {
    pub fn new(prefix_start_from: u64, hide_len: u8, postfix_len: u8) -> Self {
        Self {
            prefix_wr: S3NumsWriter::new(13, false),
            hide_wr: S3RevNumsWriter::new(hide_len, true),
            prefix: prefix_start_from.max(1),
            postfix_len,
        }
    }

    #[inline(always)]
    pub fn inc_prefix_u8(&mut self, inc_u8: u8) {
        self.prefix += inc_u8 as u64;
    }
}

impl S3WriterInfo for IdWriter {
    fn bits_once(&self) -> u8 {
        self.hide_wr.bits_once()
    }

    fn s3_once(&self) -> u64 {
        self.hide_wr.s3_once()
    }
}

impl<W: WriteExt, Rng: RngMinimal> S3WriterRand<W, Rng> for IdWriter {
    type Error = std::io::Error;

    fn write(&mut self, x: u64, w: &mut W, rng: &mut Rng) -> Result<(), Self::Error> {
        self.prefix_wr.write(self.prefix, w)?;
        self.inc_prefix_u8(rng.r8_range(0..=2) + 1);
        self.hide_wr.write(x, w)?;

        for _ in 0..self.postfix_len {
            w.write_char(rng.r_char_num())?;
        }

        Ok(())
    }
}
#[derive(Clone)]
pub struct IdReader {
    hide_r: S3NumsReader,
    postfix_len: u8,
}

impl IdReader {
    pub fn new(hide_len: u8, postfix_len: u8) -> Self {
        Self {
            hide_r: S3NumsReader::new(hide_len, true, true),
            postfix_len,
        }
    }
}

impl S3WriterInfo for IdReader {
    fn bits_once(&self) -> u8 {
        self.hide_r.bits_once()
    }

    fn s3_once(&self) -> u64 {
        self.hide_r.s3_once()
    }
}

impl<R: std::io::Read> S3Reader<StrReadWraper<R>> for IdReader {
    type Error = std::io::Error;
    
    fn read(&mut self, r: &mut StrReadWraper<R>) -> Result<u64, Self::Error> {
        let id_str = r.read_nums(true)?;
        if id_str.is_empty() {
            return Err(std::io::Error::other("IdReader: empty id?!"));
        }

        let id_str = id_str.as_bytes();
        let id_str = &id_str[..id_str.len() - self.postfix_len as usize];
        let mut id_str = &id_str[id_str.len() - self.hide_r.len() as usize..];

        Ok(self.hide_r.read(&mut id_str)?)
    }
}

#[cfg(test)]
mod tests {
    use rand::rng;
    use crate::text::str_writer::WriterFmt;
    use super::*;

    #[test]
    fn test_id_writer() {
        let nums = vec![27u8, 73, 25, 10, 0, 1, 2, 9];

        let mut rng = rng();
        let str = String::with_capacity(10);
        let mut str = WriterFmt::new(str);

        let mut wr = IdWriter::new(12, 2, 2);

        for num in nums {
            str.clear();
            wr.write(num as u64, &mut str, &mut rng).unwrap();
            
            let str = str.as_bytes();
            let len = str.len();
            assert_eq!(num % 10, str[len - 4] - b'0');
            assert_eq!(num / 10, str[len - 3] - b'0');
        }
    }
}