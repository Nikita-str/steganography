// S3 stands for state space size
// For example
// * 3 bits have S3 = 8
// * HM: [00:00 ..= 23:59] have S3 = 24 * 60 = 1440

use rand::{Rng, RngCore};
use std::{io::Read as ReadIO, u64};
use crate::text::str_writer::WriteExt;

pub trait S3WriterInfo {
    /// How many bits are needed to write once?
    fn bits_once(&self) -> u8;

    /// How many S3 are needed to write once?
    fn s3_once(&self) -> u64;
}

pub trait S3Writer<W>: S3WriterInfo {
    type Error;

    /// # Return
    /// * Was something written?
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
    /// * Was something written?
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

    fn write_fake(&mut self, w: &mut W, rng: &mut Rng) -> Result<(), Self::Error>
    where Rng: RngMinimal
    {
        let x = rng.r64_range_excl(0..self.s3_once());
        self.write(x , w, rng)
    }
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
    fn r64_range_excl(&mut self, range: std::ops::Range<u64>) -> u64;

    /// Return random value in '0'..='9'
    fn r_char_num(&mut self) -> char {
        (self.r8_range(0..=9) + b'0') as char
    }
}

impl RngMinimal for rand::rngs::ThreadRng {
    fn r8(&mut self) -> u8 {
        self.next_u32() as u8
    }

    fn r64(&mut self) -> u64 {
        self.next_u64()
    }

    fn r8_range(&mut self, range: std::ops::RangeInclusive<u8>) -> u8 {
        self.random_range(range)
    }
    
    fn r64_range_excl(&mut self, range: std::ops::Range<u64>) -> u64 {
        self.random_range(range)
    }
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
    pub fn new<R: ReadIO, Rng: RngMinimal>(r: &mut R, rng: &mut Rng) -> Result<Self, std::io::Error> {
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

        //TODO: test `after_bit_len` with s3 = [3, 7] & with s3 = [2, 2, 2] & with s3 = [4, 4]
        //MAYBE: there needed `written_after - 1`
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

    pub fn fill_buf<R: ReadIO + ?Sized>(&mut self, r: &mut R) -> Result<(), std::io::Error> {
        self.buf.fill(r)
    }

    pub fn fill_rng<R: RngMinimal + ?Sized>(&mut self, rng: &mut R) {
        self.rng_buf.fill(rng)
    }
       
    pub fn fill(&mut self, chunk_sz: u8) -> bool {
        debug_assert!(chunk_sz <= 64);

        if self.max_written_value != 0 {
            return false
        }

        let (bits, real_sz) = self.buf.try_take_bits(chunk_sz);
        if real_sz != chunk_sz {
            assert!(self.buf.is_reader_eof(), "seems like you forget to fill readder buffer");
            if real_sz == 0 {
                self.eof_stream = true;
                return false
            }
        }

        self.bits_to_write = bits;
        self.total_bits_to_write = chunk_sz;
        self.max_written_value = 1;
        true
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

mod s3_type_writer {
    use crate::text::id::IdWriter;
    use crate::text::num::{S3NumsWriter, S3RevNumsWriter};
    use crate::text::price::{S3FloatPriceWriter, S3IntPriceWriter};
    use crate::text::s3::{RngMinimal, S3WriterInfo, S3WriterRand};
    use crate::text::str_writer::WriteExt;
    use crate::text::time::S3TimeWriter;

    use crate::text::s3::S3WriterRandWrap as WrapR;

    pub enum S3TypeWriter<W, R> {
        Time(WrapR<S3TimeWriter>),
        IntPrice(S3IntPriceWriter),
        FloatPrice(S3FloatPriceWriter),
        Id(IdWriter),
        IntNumRev(WrapR<S3RevNumsWriter>),
        IntNum(WrapR<S3NumsWriter>),
        Dyn(Box<dyn S3WriterRand<W, R, Error = std::io::Error>>),
    }

    macro_rules! sub_call_impl {
        ($self:ident [$($var:ident),+] => $x:ident $call_expr:expr ) => {
            match $self {
                $(
                    S3TypeWriter::$var($x) => $call_expr
                ),+
            }
        };

        ($self:ident $fn_name:ident ($($arg_name:ident),*) ) => {
            sub_call_impl!($self [Time, IntPrice, FloatPrice, Id, IntNumRev, IntNum, Dyn] => x x.$fn_name($($arg_name),*) )
        };

        ($self:ident $fn_name:ident) => {
            sub_call_impl!($self [Time, IntPrice, FloatPrice, Id, IntNumRev, IntNum, Dyn] $fn_name)
        };
    }

    impl<W: WriteExt, Rng: RngMinimal> S3WriterInfo for S3TypeWriter<W, Rng> {
        fn bits_once(&self) -> u8 {
            sub_call_impl!(self bits_once())
        }

        fn s3_once(&self) -> u64 {
            sub_call_impl!(self s3_once())
        }
    }

    impl<W: WriteExt, Rng: RngMinimal> S3WriterRand<W, Rng> for S3TypeWriter<W, Rng> {
        type Error = std::io::Error;

        fn write(&mut self, x: u64, w: &mut W, rng: &mut Rng) -> Result<(), Self::Error> {
            sub_call_impl!(self write(x, w, rng))
        }
    }
}
pub use s3_type_writer::S3TypeWriter;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub trait S3ChunkWriter {

}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct S3Full<'x, R: ?Sized, W: ?Sized, Rng: ?Sized> {
    r: &'x mut R,
    w: &'x mut W,
    rng: &'x mut Rng,
    bit_r: S3BitReader,
    chunk_sz: Option<u8>,
}

#[must_use]
pub struct S3FillResult {
    pub eof_stream: bool,
    pub need_chunk: bool,
}

impl<'x, R: ReadIO, W: WriteExt, Rng: RngMinimal> S3Full<'x, R, W, Rng> {
    pub fn new(r: &'x mut R, w: &'x mut W, rng: &'x mut Rng) -> Result<Self, std::io::Error> {
        Ok(Self {
            bit_r: S3BitReader::new(r, rng)?,
            r,
            w,
            rng,
            chunk_sz: None,
        })
    }

    #[inline(always)]
    pub fn writer_mut(&mut self) -> &mut W {
        self.w
    }

    #[inline(always)]
    pub fn is_eof_stream(&self) -> bool {
        self.bit_r.is_eof()
    }

    #[inline(always)]
    pub fn is_need_chunk(&self) -> bool {
        self.chunk_sz.is_none()
    }

    /// # Panics
    /// * if `self.is_need_chunk()`
    #[inline(always)]
    pub fn set_next_chunk(&mut self, chunk_sz: u8) {
        assert!(self.is_need_chunk());
        self.chunk_sz = Some(chunk_sz)
    }

    pub fn write_s3<S3W>(&mut self, s3w: &mut S3W, fake: bool) -> Result<bool, std::io::Error>
    where S3W: S3WriterRand<W, Rng, Error = std::io::Error> + ?Sized
    {
        self.bit_r.fill_buf(self.r)?;
        self.bit_r.fill_rng(self.rng);
        
        if let Some(chunk_sz) = self.chunk_sz {
            if self.bit_r.fill(chunk_sz) {
                self.chunk_sz = None;
            }
        } else {
            // if !self.bit_r.is_eof() {
            //     return Err(std::io::Error::other("You cannot fill while chunk is needed!"));
            // }
        }

        if self.bit_r.is_eof() {
            s3w.write_fake(self.w, self.rng)?;
            return Ok(true)
        }
        
        if fake {
            s3w.write_fake(self.w, self.rng)?;
        } else {
            let was_written = s3w.write_full(&mut self.bit_r, self.w, self.rng)?;
            debug_assert!(was_written);
        }

        Ok(false)
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

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
    pub fn fill<R: RngMinimal + ?Sized>(&mut self, rng: &mut R) {
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
    pub fn fill<R: std::io::Read + ?Sized>(&mut self, r: &mut R) -> Result<(), std::io::Error> {
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
    pub fn fill_trimed(&mut self, bits: u64, n: u8) {
        let bits = bits & (Self::MASK >> (64 - n));
        self.fill(bits, n);
    }

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

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  ━━━   ━━━   ━━━   ━━━   ━━━   ━━━   ━━━   ━━━   ━━━   ━━━   ━━━   ━━━   ━━━
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub trait S3Reader<R>: S3WriterInfo {
    type Error;

    /// # Return
    /// * Was something read?
    fn read_full(&mut self, writer: &mut S3BitWriter, r: &mut R) -> Result<bool, Self::Error> {
        if writer.need_fill() {
            Ok(false)
        } else {
            let s3 = self.s3_once();
            let s3_value = self.read(r)?;
            writer.write_s3(s3_value, s3);
            Ok(true)
        }
    }

    fn read(&mut self, r: &mut R) -> Result<u64, Self::Error>;

    fn read_fake(&mut self, r: &mut R) -> Result<(), Self::Error> {
        self.read(r)?;
        Ok(())
    }
}

pub struct S3BitWriter {
    buf: S3BitBuf,
    bits_read: u64,
    max_read_value: u64,
    chunk_sz: u8,
    is_eof: bool,
}

impl S3BitWriter {
    pub fn new() -> Self {
        Self {
            buf: S3BitBuf {
                lower_bits: 0,
                upper_bits: 0,
                bit_rest: 0,
            },
            bits_read: 0,
            max_read_value: 0,
            chunk_sz: 0,
            is_eof: false,
        }
    }    

    pub fn need_fill(&self) -> bool {
        self.max_read_value == 0
    }
    
    pub fn is_eof(&self) -> bool {
        self.is_eof
    }

    /// # Panic
    /// * if `self.need_fill()`
    pub fn write_s3(&mut self, s3_value: u64, s3: u64) {
        if self.need_fill() {
            panic!("S3BitWriter: Need fill! (call `fill` & test if by `need_fill`)")
        }

        self.bits_read += self.max_read_value * s3_value;

        let after_bit_len;
        if let Some(read_after) = self.max_read_value.checked_mul(s3) {
            after_bit_len = 64 - read_after.leading_zeros() as u8;
            self.max_read_value = read_after;
        } else {
            after_bit_len = 65;
        }

        //TODO: test `<` with s3 = [3, 7] & with s3 = [2, 2, 2] & with s3 = [4, 4]
        if self.chunk_sz < after_bit_len {
            self.buf.fill_trimed(self.bits_read, self.chunk_sz);
            self.max_read_value = 0;
            self.bits_read = 0;
            self.chunk_sz = 0;
        }
    }

    pub fn set_chunk_size(&mut self, chunk_sz: u8) -> bool {
        debug_assert!(chunk_sz <= 64);

        if !self.need_fill() {
            return false
        }

        self.bits_read = 0;
        self.max_read_value = 1;
        self.chunk_sz = chunk_sz;
        true
    }

    #[inline(always)]
    pub fn has_chunk(&self) -> bool {
        !self.is_eof && self.buf.bit_rest >= 64
    }
    
    #[inline(always)]
    pub fn try_take_chunk(&mut self) -> Option<u64> {
        self.has_chunk().then(||self.buf.take_bits(64))
    }
    
    /// # Paincs
    /// * if `self.has_chunk()` 
    /// # Returns
    /// `(rest_bits, amount_of_bits)`
    pub fn take_on_eof(&mut self) -> (u64, u8) {
        let rest = self.buf.bit_rest;
        let is_eof = self.is_eof;
        self.is_eof = true;
        
        if is_eof || rest == 0 {
            return (0, 0);
        }

        if rest > 64 {
            panic!("You cannot `take_on_eof` when there still is a full chunk");
        }
        
        if rest % 8 != 0 {
            panic!("You cannot `take_on_eof` with not an integer number of bytes");
        }

        (self.buf.take_bits(rest), rest)
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

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