use std::f32::consts::LOG2_10;

use crate::text::s3::{S3Writer, S3WriterInfo};
use crate::text::str_writer::WriteExt;

pub struct Num10ToBits {
    bits: u64,
    mul: u64,
    rest_n: u8,
}

impl Num10ToBits {
    pub const fn new(chunk_bit_sz: u8) -> Self {
        debug_assert!(chunk_bit_sz <= 64);

        Self {
            bits: 0,
            mul: 1,
            rest_n: f32::ceil(chunk_bit_sz as f32 / LOG2_10) as u8,
        }
    }
    
    pub const fn new_const<const CHUNK_SZ: u8>() -> Self {
        debug_assert!(CHUNK_SZ <= 64);

        Self {
            bits: 0,
            mul: 1,
            rest_n: f32::ceil(CHUNK_SZ as f32 / LOG2_10) as u8,
        }
    }

    pub const fn new_u32() -> Self {
        Self::new_const::<32>()
    }

    /// The loss is about `~3.8%` ~ `2.43` bits
    pub const fn new_u64() -> Self {
        Self::new_const::<64>()
    }

    /// The loss is about `~0.185%` ~ `0.12` bits
    pub const fn new_low_loss() -> Self {
        Self::new_const::<63>()
    }

    pub const fn is_done(&self) -> bool {
        self.rest_n == 0
    }
    
    pub fn try_take(&self) -> Option<u64> {
        self.is_done().then_some(self.bits)
    }

    /// Handle num char, ignores all others.
    /// 
    /// # Panic
    /// * if `self.is_done()` & `ch` is num char
    pub fn next_any_char(&mut self, ch: char) -> Option<u64> {
        match ch {
            '0'..='9' => self.next_n10_char(ch),
            _ => None
        }
    }

    /// # Panic
    /// * if `self.is_done()`
    pub fn next_n10_char(&mut self, num_ch: char) -> Option<u64> {
        let n = num_ch as u8 - b'0';
        self.next_n10(n)
    }

    /// # Panic
    /// * if `self.is_done()`
    pub fn next_n10(&mut self, n: u8) -> Option<u64> {
        if self.is_done() {
            panic!("Num10ToBits: you lost a number ({n})")
        }

        self.rest_n -= 1;
        self.bits = (n as u64 * self.mul) + self.bits;
        self.mul = self.mul.wrapping_mul(10);

        self.try_take()
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct BitsToNum10 {
    bits: u64,
    rest_n: u8,
}

impl BitsToNum10 {
    pub const fn new(chunk: u64, chunk_bit_sz: u8) -> Self {
        debug_assert!(chunk_bit_sz <= 64);
        debug_assert!((64 - chunk.leading_zeros()) as u8 <= chunk_bit_sz);

        Self {
            bits: chunk,
            rest_n: f32::ceil(chunk_bit_sz as f32 / LOG2_10) as u8,
        }
    }

    pub const fn new_const<const CHUNK_SZ: u8>(chunk: u64) -> Self {
        debug_assert!(CHUNK_SZ <= 64);
        debug_assert!((64 - chunk.leading_zeros()) as u8 <= CHUNK_SZ);

        Self {
            bits: chunk,
            rest_n: f32::ceil(CHUNK_SZ as f32 / LOG2_10) as u8,
        }
    }

    pub const fn new_empty() -> Self {
        Self::new_const::<0>(0)
    }

    pub const fn new_u32(chunk: u64) -> Self {
        Self::new_const::<32>(chunk)
    }

    /// The loss is about `~3.8%` ~ `2.43` bits
    pub const fn new_u64(chunk: u64) -> Self {
        Self::new_const::<64>(chunk)
    }

    /// The loss is about `~0.185%` ~ `0.12` bits
    pub const fn new_low_loss(chunk: u64) -> Self {
        Self::new_const::<63>(chunk)
    }

    pub const fn is_done(&self) -> bool {
        self.rest_n == 0
    }

    pub fn next_n10_char(&mut self) -> Option<char> {
        self.next_n10().map(|x| (b'0' + x) as char)
    }

    pub fn next_n10(&mut self) -> Option<u8> {
        (!self.is_done()).then(||{
            self.rest_n -= 1;
            let ret = (self.bits % 10) as u8;
            self.bits /= 10;
            ret
        })
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Clone, Copy)]
pub struct S3NumWriter<const IS_ZERO_DISPLAY: bool> { }

impl S3NumWriter<true> {
    #[inline(always)]
    pub fn new_display_zero() -> S3NumWriter<true> {
        Self { }
    }
}
impl S3NumWriter<false> {
    #[inline(always)]
    pub fn new_non_display_zero() -> S3NumWriter<false> {
        Self { }
    }
}

impl<const IS_ZERO_DISPLAY: bool> S3NumWriter<IS_ZERO_DISPLAY> {
    #[inline]
    pub fn write_u8<W: WriteExt>(&mut self, w: &mut W, x: u8) -> Result<(), std::io::Error> {
        debug_assert!(x < 10);

        if !IS_ZERO_DISPLAY && x == 0 {
            return Ok(())
        }

        w.write_char((b'0' + x as u8) as char)
    }
}

impl<const IS_ZERO_DISPLAY: bool> S3WriterInfo for S3NumWriter<IS_ZERO_DISPLAY> {
    fn bits_once(&self) -> u8 {
        4 // ceil(log2(10))
    }

    fn s3_once(&self) -> u64 {
        10
    }
}

impl<W: WriteExt, const IS_ZERO_DISPLAY: bool> S3Writer<W> for S3NumWriter<IS_ZERO_DISPLAY> {
    type Error = std::io::Error;

    /// # Panics
    /// * if `self.s3_once() <= x`
    fn write(&mut self, x: u64, w: &mut W) -> Result<(), Self::Error> {
        debug_assert!(x < self.s3_once());
        self.write_u8(w, x as u8)
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Clone)]
pub struct S3NumsWriter {
    buf: Vec<u8>,
    s3_once: u64,
    len: u8,
    zeroed: bool,
}

impl S3NumsWriter {
    #[inline(always)]
    pub fn new(num_len: u8, zeroed: bool) -> S3NumsWriter {
        Self {
            buf: Vec::with_capacity(num_len as usize),
            s3_once: 10u64.pow(num_len as u32),
            len: num_len,
            zeroed,
        }
    }
}

impl S3WriterInfo for S3NumsWriter {
    fn bits_once(&self) -> u8 {
        (self.s3_once.ilog2() + 1) as u8
    }

    fn s3_once(&self) -> u64 {
        self.s3_once
    }
}

impl<W: WriteExt> S3Writer<W> for S3NumsWriter {
    type Error = std::io::Error;

    /// # Panics
    /// * if `self.s3_once() <= x`
    fn write(&mut self, mut x: u64, w: &mut W) -> Result<(), Self::Error> {
        debug_assert!(x < self.s3_once());

        self.buf.clear();
        for _ in 0..self.len {
            let num = (x % 10) as u8;
            x = x / 10;
            self.buf.push(num);
        }

        let mut display_zero = self.zeroed;
        for x in self.buf.drain(..).rev() {
            let is_zero = x == 0;
            display_zero |= !is_zero;

            if display_zero || !is_zero {
                S3NumWriter::new_display_zero().write_u8(w, x)?;
            }
        }

        if !display_zero {
            S3NumWriter::new_display_zero().write_u8(w, 0)?;
        }

        Ok(())
    }
}

// ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━

#[derive(Clone, Copy)]
pub struct S3RevNumsWriter {
    s3_once: u64,
    len: u8,
    zeroed: bool,
}

impl S3RevNumsWriter {
    #[inline(always)]
    pub fn new(num_len: u8, zeroed: bool) -> S3RevNumsWriter {
        Self {
            s3_once: 10u64.pow(num_len as u32),
            len: num_len,
            zeroed,
        }
    }
}

impl S3WriterInfo for S3RevNumsWriter {
    fn bits_once(&self) -> u8 {
        (self.s3_once.ilog2() + 1) as u8
    }

    fn s3_once(&self) -> u64 {
        self.s3_once
    }
}

impl<W: WriteExt> S3Writer<W> for S3RevNumsWriter {
    type Error = std::io::Error;

    /// # Panics
    /// * if `self.s3_once() <= x`
    fn write(&mut self, mut x: u64, w: &mut W) -> Result<(), Self::Error> {
        debug_assert!(x < self.s3_once());

        let mut display_zero = self.zeroed;
        for _ in 0..self.len {
            let num = (x % 10) as u8;
            x = x / 10;

            let is_zero = num == 0;
            display_zero |= !is_zero;
            
            if display_zero || !is_zero {
                S3NumWriter::new_display_zero().write_u8(w, num)?;
            }
        }

        if !display_zero {
            S3NumWriter::new_display_zero().write_u8(w, 0)?;
        }

        Ok(())
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use crate::text::str_writer::WriterFmt;

    use super::*;

    #[test]
    fn test_num_conv() {
        let chunks = vec![
            0b_010_1010_1101,
            0b_110_1010_1101,
            0b_001_0011_0001,
            0b_111_1111_1111,
            0b_100_1000_1000,
            0b_111_1100_1111,
        ];
        let expects = vec![
            "5860",
            "9071",
            "5030",
            "7402",
            "0611",
            "9991",
        ];

        let chunk_bit_sz = 11;
        let mut nums = String::with_capacity(4);

        for (chunk, expect) in chunks.into_iter().zip(expects) {
            nums.clear();

            let mut b2n = BitsToNum10::new(chunk, chunk_bit_sz);
            while let Some(ch) = b2n.next_n10_char() {
                nums.push(ch);
            }
            assert_eq!(&nums, expect);

            let mut n2b = Num10ToBits::new(chunk_bit_sz);
            for num_ch in expect.chars() {
                n2b.next_n10_char(num_ch);
            }
            assert_eq!(n2b.try_take(), Some(chunk));
        }
    }

    #[test]
    fn test_num_s3_writer() {
        let nums = vec![257, 739, 25, 100, 10, 0, 1, 2, 9];

        let mut wr = S3NumsWriter::new(3, false);
        let mut zeroed_wr = S3NumsWriter::new(3, true);

        let str = String::with_capacity(10);
        let mut str = WriterFmt::new(str);

        for num in nums {
            str.clear();
            wr.write(num, &mut str).unwrap();
            assert_eq!(str.as_ref(), &format!("{num}"));
            
            str.clear();
            zeroed_wr.write(num, &mut str).unwrap();
            assert_eq!(str.as_ref(), &format!("{num:03}"));
        }
    }
    
    #[test]
    fn test_num_s3_writer_rev() {
        let nums = vec![257, 739, 25, 100, 10, 0, 1, 2, 9];

        let expect_wr = vec!["752", "937", "520", "1", "10", "0", "100", "200", "900"];
        let expect_zeroed = vec!["752", "937", "520", "001", "010", "000", "100", "200", "900"];

        let mut wr = S3RevNumsWriter::new(3, false);
        let mut zeroed_wr = S3RevNumsWriter::new(3, true);

        let str = String::with_capacity(10);
        let mut str = WriterFmt::new(str);

        for (i, num) in nums.into_iter().enumerate() {
            str.clear();
            wr.write(num, &mut str).unwrap();
            assert_eq!(str.as_ref(), &expect_wr[i]);
            
            str.clear();
            zeroed_wr.write(num, &mut str).unwrap();
            assert_eq!(str.as_ref(), &expect_zeroed[i]);
        }

        let mut wr = S3RevNumsWriter::new(4, false);
        str.clear();
        wr.write(20, &mut str).unwrap();
        assert_eq!(str.as_ref(), "200");

        str.clear();
        wr.write(120, &mut str).unwrap();
        assert_eq!(str.as_ref(), "210");
        
        str.clear();
        wr.write(1230, &mut str).unwrap();
        assert_eq!(str.as_ref(), "321");
    }

    #[test]
    fn test_bit_loss() {
        let bit_loss_proc = |bits: u32| {
            let bits = bits as f32;
            let ceil_bits = f32::ceil(bits / LOG2_10) * LOG2_10;
            let delta_bits = ceil_bits - bits;
            let proc = 100.00 * delta_bits / bits;
            return proc
        };

        let mut best_loss_proc = bit_loss_proc(1);
        let mut best_bits = vec![1];
        let mut best_n = vec![1];

        for bits in 2..=128 {
            let loss_proc = bit_loss_proc(bits);
            if loss_proc < best_loss_proc {
                best_loss_proc = loss_proc;
                let n = f32::ceil(bits as f32 / LOG2_10) as u32;
                if best_n.last() != Some(&n) {
                    best_n.push(n);
                }
                best_bits.push(bits);
                // println!("bits = {bits:2} <--> n = {n:2} (loss = {best_loss_proc:.3}%)");
            }
        }

        assert!(best_loss_proc < 0.1);
        assert_eq!(&best_bits, &[1, 2, 3, 13, 23, 33, 43, 53, 63, 73, 83, 93]);
        assert_eq!(&best_n, &[1, 4, 7, 10, 13, 16, 19, 22, 25, 28]);
    }
}
