// S3 stands for state space size
// For example
// * 3 bits have S3 = 8
// * HM: [00:00 ..= 23:59] have S3 = 24 * 60 = 1440

pub trait S3WriterInfo {
    /// How many bits are needed to write once?
    fn bits_once(&self) -> u8;

    /// How many S3 are needed to write once?
    fn s3_once(&self) -> u64;
}

pub trait S3Writer<W>: S3WriterInfo {
    type Error;

    /// # Return
    /// * Was something write?
    fn write_full(&mut self, reader: &mut S3BitReader, w: &mut W) -> Result<bool, Self::Error> {
        if reader.need_fill() {
            Ok(false)
        } else {
            let bits = reader.take_bits_from_writer(self);
            self.write(bits, w)?;
            Ok(true)
        }
    }

    fn write(&mut self, x: u64, w: &mut W) -> Result<(), Self::Error>;
}

pub trait S3WriterRand<W, Rng>: S3WriterInfo {
    type Error;

    /// # Return
    /// * Was something write?
    fn write_full(&mut self, reader: &mut S3BitReader, w: &mut W, rng: &mut Rng) -> Result<bool, Self::Error> {
        if reader.need_fill() {
            Ok(false)
        } else {
            let bits = reader.take_bits_from_writer(self);
            self.write(bits, w, rng)?;
            Ok(true)
        }
    }

    fn write(&mut self, x: u64, w: &mut W, rng: &mut Rng) -> Result<(), Self::Error>;
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct S3WriterRandWrap<T>(pub T);

impl<T: S3WriterInfo> S3WriterInfo for S3WriterRandWrap<T> {
    fn bits_once(&self) -> u8 {
        self.0.bits_once()
    }

    fn s3_once(&self) -> u64 {
        self.0.s3_once()
    }
}

impl<W, Rng, Err, T: S3Writer<W, Error = Err>> S3WriterRand<W, Rng> for S3WriterRandWrap<T> {
    type Error = Err;

    fn write(&mut self, x: u64, w: &mut W, _: &mut Rng) -> Result<(), Self::Error> {
        S3Writer::write(&mut self.0, x, w)
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub trait RngMinimal {
    fn r8(&mut self) -> u8;
    fn r64(&mut self) -> u64;

    fn r8_range(&mut self, range: std::ops::RangeInclusive<u8>) -> u8;
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct S3BitReader {
    buf: S3BitBufReader,
    rng_buf: RngBuf,
    bits_to_write: u64,
    max_written_value: u64,
    total_bits_to_write: u8,
    eof_stream: bool,
}

impl S3BitReader {
    pub fn new<R: std::io::Read, Rng: RngMinimal>(&mut self, r: &mut R, rng: &mut Rng) -> Result<Self, std::io::Error> {
        Ok(Self {
            buf: S3BitBufReader::new(r)?,
            rng_buf: RngBuf::new(rng),
            bits_to_write: 0,
            max_written_value: 0,
            total_bits_to_write: 0,
            eof_stream: false,
        })
    }

    pub fn need_fill(&self) -> bool {
        self.max_written_value == 0
    }
    
    pub fn is_eof(&self) -> bool {
        self.eof_stream
    }

    pub fn take_bits_from_writer<W: S3WriterInfo + ?Sized>(&mut self, s3w: &W) -> u64 {
        self.take_bits(s3w.s3_once(), s3w.bits_once())
    }

    /// # Panic
    /// * if `self.need_fill()`
    pub fn take_bits(&mut self, s3: u64, n: u8) -> u64 {
        if self.max_written_value == 0 {
            panic!("S3BitReader: Need fill! (call `fill` & test if by `need_fill`)")
        }

        let after_bit_len;
        if let Some(written_after) = self.max_written_value.checked_mul(s3) {
            after_bit_len = 64 - written_after.leading_zeros() as u8;
            self.max_written_value = written_after;

            if after_bit_len > self.total_bits_to_write {
                self.max_written_value = 0;
            }
        } else {
            let written_after = self.max_written_value as u128 * s3 as u128;
            after_bit_len = 128 - written_after.leading_zeros() as u8;
            self.max_written_value = 0;
        }

        if self.total_bits_to_write >= after_bit_len {
            let bits = self.bits_to_write % s3;
            self.bits_to_write /= s3;
            bits
        } else {
            debug_assert!(self.max_written_value == 0);

            let delta = after_bit_len - self.total_bits_to_write;
            debug_assert!(delta > 0);
            let bits = self.bits_to_write;
            let mut bits = self.rng_buf.concat(bits, n - delta, delta);

            if bits >= s3 {
                bits ^= 1 << (n - 1);
                debug_assert!(bits < s3);
            }

            bits
        }
    }

    pub fn fill_buf<R: std::io::Read>(&mut self, r: &mut R) -> Result<(), std::io::Error> {
        self.buf.fill(r)
    }

    pub fn fill_rng<R: RngMinimal>(&mut self, rng: &mut R) {
        self.rng_buf.fill(rng)
    }
       
    pub fn fill(&mut self, chunk_sz: u8) {
        debug_assert!(chunk_sz <= 64);

        if self.max_written_value != 0 {
            return
        }

        let (bits, real_sz) = self.buf.try_take_bits(chunk_sz);
        if real_sz != chunk_sz {
            assert!(self.buf.is_reader_eof(), "seems like you forget to fill readder buffer");
            if real_sz == 0 {
                self.eof_stream = true;
            }
        }

        self.bits_to_write = bits;
        self.total_bits_to_write = chunk_sz;
        self.max_written_value = 1;
    }
}

#[derive(Debug)]
pub struct RngBuf {
    buf: S3BitBuf,
}
impl RngBuf {
    pub fn new<R: RngMinimal>(rng: &mut R) -> Self {
        let buf = S3BitBuf {
            lower_bits: rng.r64(),
            upper_bits: rng.r64(),
            bit_rest: S3BitBuf::FULL_SIZE,
        };
        Self { buf }
    }
    
    #[inline]
    pub fn fill<R: RngMinimal>(&mut self, rng: &mut R) {
        if self.buf.bit_rest >= S3BitBuf::HALF_SIZE {
            return
        }

        let r_bits = rng.r64();
        self.buf.fill(r_bits, S3BitBuf::HALF_SIZE);
    }
    
    #[inline]
    pub fn r_bits(&mut self, n: u8) -> u64 {
        self.buf.take_bits(n)
    }
    
    #[inline]
    pub fn concat(&mut self, bits: u64, bits_n: u8, more_n: u8) -> u64 {
        bits | (self.r_bits(more_n)) << bits_n
    }
}

#[derive(Debug)]
pub struct S3BitBufReader {
    buf: S3BitBuf,
    reader_eof: bool,
}

impl S3BitBufReader {
    pub fn new<R: std::io::Read>(r: &mut R) -> Result<Self, std::io::Error> {
        const BUF_SZ: usize = 8 * 2;
        let mut byte_buf = [0u8; BUF_SZ];
        let mut bytes_readed = 0;
        
        loop {
            let readed = r.read(&mut byte_buf[bytes_readed..])?;
            bytes_readed += readed;
            if readed == 0 {
                break
            }
            if bytes_readed == BUF_SZ {
                break
            }
        }

        macro_rules! part {
            ($buf:ident, [$($parts:literal)+]) => {
                [$($buf[$parts],)+]
            };
        }

        let buf = S3BitBuf {
            lower_bits: u64::from_le_bytes(part![byte_buf, [0 1 2 3 4 5 6 7]]),
            upper_bits: u64::from_le_bytes(part![byte_buf, [8 9 10 11 12 13 14 15]]),
            bit_rest: bytes_readed as u8 * 8,
        };

        Ok(Self {
            buf,
            reader_eof: bytes_readed < BUF_SZ
        })
    }

    #[inline]
    pub fn fill<R: std::io::Read>(&mut self, r: &mut R) -> Result<(), std::io::Error> {
        if self.reader_eof {
            return Ok(())
        }
        if self.buf.bit_rest >= S3BitBuf::HALF_SIZE {
            return Ok(())
        }

        let mut byte_buf = [0u8; 8];
        let mut bytes_readed = 0;
        
        loop {
            let readed = r.read(&mut byte_buf[bytes_readed..])?;
            bytes_readed += readed;
            if readed == 0 {
                break
            }
            if bytes_readed == 8 {
                break
            }
        }

        let bits = u64::from_le_bytes(byte_buf);
        self.buf.fill(bits, bytes_readed as u8 * 8);

        self.reader_eof = bytes_readed != 8;

        Ok(())
    }

    #[inline]
    pub fn try_take_bits(&mut self, n: u8) -> (u64, u8) {
        self.buf.try_take_bits(n)
    }

    pub fn is_reader_eof(&self) -> bool {
        self.reader_eof
    }
    pub fn is_eof(&self) -> bool {
        self.reader_eof && self.buf.bit_rest == 0
    }
}


#[derive(Debug)]
pub struct S3BitBuf {
    lower_bits: u64,
    upper_bits: u64,
    bit_rest: u8,
}

impl S3BitBuf {
    const HALF_SIZE: u8 = 64;
    const FULL_SIZE: u8 = Self::HALF_SIZE * 2;
    
    const MASK: u64 = !0;

    #[inline]
    pub fn fill(&mut self, bits: u64, n: u8) {
        if Self::FULL_SIZE < self.bit_rest + n {
            return
        }
        
        let bits_lower_part = bits.unbounded_shl(self.bit_rest as u32);
        let bits_upper_part = bits.unbounded_shr((S3BitBuf::HALF_SIZE - self.bit_rest) as u32);

        self.lower_bits |= bits_lower_part;
        self.upper_bits |= bits_upper_part;

        self.bit_rest += n;
    }

    #[inline]
    pub fn try_take_bits(&mut self, n: u8) -> (u64, u8) {
        if self.bit_rest < n {
            let ret = self.lower_bits;
            let n_ret = self.bit_rest;
            
            self.lower_bits = 0;
            self.bit_rest = 0;

            (ret, n_ret)
        } else {
            (self.take_bits(n), n)
        }
    }

    #[inline]
    pub fn take_bits(&mut self, n: u8) -> u64 {
        debug_assert!(self.bit_rest >= n);
        if n == 0 { return 0 }

        let mask = Self::MASK >> (64 - n);
        let ret = self.lower_bits & mask;

        let upper = self.upper_bits & mask;
        self.upper_bits = self.upper_bits.unbounded_shr(n as u32);
        self.lower_bits = self.lower_bits.unbounded_shr(n as u32);
        self.lower_bits |= upper << (64 - n);

        self.bit_rest -= n;

        ret
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // TODO: test RngBuf
    // TODO: more tests for Reader

    // TODO: test `S3BitReader::take_bits`

    #[test]
    fn test_bit_buf() {
        struct Reader {
            vec: Vec<u8>,
            ptr: usize,
        }
        impl std::io::Read for Reader {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                if buf.is_empty() { return Ok(0) }

                let vec_sz = self.vec.len();
                let rest = vec_sz - self.ptr;
                let sz = if rest <= buf.len() {
                    rest.min(3)  
                } else {
                    buf.len().min(3)
                };

                for i in 0..sz {
                    buf[i] = self.vec[self.ptr + i];
                }

                self.ptr += sz;

                Ok(sz)
            }
        }

        let mut reader = Reader {
            vec: vec![
                0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x11, 0x0F, 0x01,
                0x2A, 0x2B, 0x2C, 0x2D, 0x2E, 0x2F, 0x21, 0x22,
                0x5A, 0x5B, 0x5C, 0x5D, 0x5E, 0x5F, 0x55, 0x77,
                0x30, 0x31, 0x32, 0x33, 0x34,
            ],
            ptr: 0,
        };

        let mut buf = S3BitBufReader::new(&mut reader).unwrap();
        assert_eq!(buf.try_take_bits(8), (0x0A, 8));
        assert_eq!(buf.try_take_bits(7 * 8), (0x010F_110E_0D0C_0B, 7 * 8));

        buf.fill(&mut reader).unwrap();
        assert_eq!(buf.try_take_bits(64), (0x2221_2F2E_2D2C_2B2A, 64));
        assert!(!buf.is_reader_eof());

        buf.fill(&mut reader).unwrap();
        assert_eq!(buf.try_take_bits(64), (0x7755_5F5E_5D5C_5B5A, 64));
        assert!(!buf.is_reader_eof());

        buf.fill(&mut reader).unwrap();
        buf.fill(&mut reader).unwrap();
        assert!(!buf.is_eof());
        assert!(buf.is_reader_eof());
        assert_eq!(buf.try_take_bits(64), (0x0000_0034_3332_3130, 8 * 5));
        assert!(buf.is_eof());
        buf.fill(&mut reader).unwrap();
        buf.fill(&mut reader).unwrap();
        assert!(buf.is_eof());
        assert!(buf.is_reader_eof());

        assert_eq!(buf.try_take_bits(8), (0x0, 0));
    }
}