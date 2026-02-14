use crate::text::s3::{S3Writer, S3WriterInfo};
use crate::text::str_writer::WriteExt;

/// Writes `n2` that in `0..=99`
fn write_n2<W: WriteExt>(w: &mut W, n2: u16) -> std::io::Result<()> {
    debug_assert!(n2 <= 99);

    let a = (n2 / 10) as u8 + b'0';
    let b = (n2 % 10) as u8 + b'0';
    w.write_char(a as char)?;
    w.write_char(b as char)?;

    Ok(())
}

/// Writes `n3` that in `0..=999`
fn write_n3<W: WriteExt>(w: &mut W, n3: u16) -> std::io::Result<()> {
    debug_assert!(n3 <= 999);

    let a = ((n3 / 10) / 10) as u8 + b'0';
    let b = ((n3 / 10) % 10) as u8 + b'0';
    let c = (n3 % 10) as u8 + b'0';
    w.write_char(a as char)?;
    w.write_char(b as char)?;
    w.write_char(c as char)?;

    Ok(())
}

/// Reads `n2` that in `0..=99`
fn read_n2(r: &str) -> u32 {
    debug_assert!(r.len() == 2);

    let a = r.as_bytes()[0] - b'0';
    let b = r.as_bytes()[1] - b'0';

    (a * 10 + b) as u32
}

/// Reads `n3` that in `0..=999`
fn read_n3(r: &str) -> u32 {
    debug_assert!(r.len() == 3);

    let a = (r.as_bytes()[0] - b'0') as u32;
    let b = (r.as_bytes()[1] - b'0') as u32;
    let c = (r.as_bytes()[2] - b'0') as u32;

    a * 100 + b * 10 + c
}

#[derive(Clone, Copy)]
pub enum TimeFormat {
    /// `23:59`
    HM,
    /// `23:59:59`
    HMS,
    /// `23:59:59.999`
    HMSMill,
}
impl TimeFormat {
    pub const fn bit_size(self) -> u8 {
        let floor_bit = match self {
            TimeFormat::HM => u64::ilog2(self.variants()) as u8,
            TimeFormat::HMS => u64::ilog2(self.variants()) as u8,
            TimeFormat::HMSMill => u64::ilog2(self.variants()) as u8,
        };
        floor_bit + 1
    }

    pub const fn variants(self) -> u64 {
        match self {
            TimeFormat::HM => 24 * 60,
            TimeFormat::HMS => 24 * 60 * 60,
            TimeFormat::HMSMill => 24 * 60 * 60 * 1000,
        }
    }
    
    pub const fn char_len(self) -> usize {
        match self {
            TimeFormat::HM => "23:59".len(),
            TimeFormat::HMS => "23:59:59".len(),
            TimeFormat::HMSMill => "23:59:59.1000".len(),
        }
    }
}

pub struct TimeToBits {
    bits: u64,
    mul: u64,
    rest_n: u8,
    fmt: TimeFormat,
}

impl TimeToBits {
    pub const fn char_len(&self) -> usize {
        self.fmt.char_len()
    }

    pub fn mask(n: u8, fmt: TimeFormat) -> u64 {
        if n == 0 { return 0 }

        let bits = f64::floor(f64::log2(fmt.variants() as f64) * n as f64) as u8;
        (!0u64) >> (64 - bits)
    }

    pub const fn new(n: u8, fmt: TimeFormat) -> Self {
        match fmt {
            TimeFormat::HM => debug_assert!(n <= 6),
            TimeFormat::HMS => debug_assert!(n <= 3),
            TimeFormat::HMSMill => debug_assert!(n <= 2),
        }

        Self {
            bits: 0,
            mul: 1,
            rest_n: n,
            fmt,
        }
    }

    pub const fn is_done(&self) -> bool {
        self.rest_n == 0
    }
    
    pub fn try_take(&self) -> Option<u64> {
        self.is_done().then_some(self.bits)
    }

    /// # Panic
    /// * if `self.is_done()` || `s.len() != self.char_len()`
    pub fn next(&mut self, time_s: &str) -> Option<u64> {
        if self.is_done() {
            panic!("Num10ToBits: you lost a time ({time_s})")
        }

        self.rest_n -= 1;
        let mut x = 0;
        match self.fmt {
            TimeFormat::HM => {
                x = read_n2(&time_s[0..=1]) * 60 + read_n2(&time_s[3..=4]);
            }
            TimeFormat::HMS => {
                x += read_n2(&time_s[0..=1]) * (60 * 60);
                x += read_n2(&time_s[3..=4]) * 60;
                x += read_n2(&time_s[6..=7]);
            }
            TimeFormat::HMSMill => {
                x += read_n2(&time_s[0..=1]) * (60 * 60 * 1000);
                x += read_n2(&time_s[3..=4]) * (60 * 1000);
                x += read_n2(&time_s[6..=7]) * 1000;
                x += read_n3(&time_s[9..=11]);
            }
        }

        self.bits = (x as u64 * self.mul) + self.bits;
        self.mul = self.mul.wrapping_mul(self.fmt.variants());

        self.try_take()
    }
}

pub struct BitsToTime {
    bits: u64,
    rest_n: u8,
    fmt: TimeFormat,
}

impl BitsToTime {
    pub const fn new(chunk: u64, n: u8, fmt: TimeFormat) -> Self {
        match fmt {
            TimeFormat::HM => debug_assert!(n <= 6),
            TimeFormat::HMS => debug_assert!(n <= 3),
            TimeFormat::HMSMill => debug_assert!(n <= 2),
        }

        Self {
            bits: chunk,
            rest_n: n,
            fmt,
        }
    }

    pub const fn new_hm(chunk: u64, n: u8) -> Self {
        debug_assert!(n <= 6);

        Self {
            bits: chunk,
            rest_n: n,
            fmt: TimeFormat::HM,
        }
    }

    pub const fn new_hms(chunk: u64, n: u8) -> Self {
        debug_assert!(n <= 3);

        Self {
            bits: chunk,
            rest_n: n,
            fmt: TimeFormat::HMS,
        }
    }

    pub const fn new_hms_ms(chunk: u64, n: u8) -> Self {
        debug_assert!(n <= 2);

        Self {
            bits: chunk,
            rest_n: n,
            fmt: TimeFormat::HMSMill,
        }
    }

    pub const fn is_done(&self) -> bool {
        self.rest_n == 0
    }

    /// # Return
    /// * `Some(_)` => something was writed
    /// * `None` => `self.is_done()` (do nothing)
    pub fn write<W: WriteExt>(&mut self, w: &mut W) -> std::io::Result<Option<()>> {
        if let Some(bits) = self.next_u32() {
            S3TimeWriter::new(self.fmt).write(bits as u64, w)?;
            Ok(Some(()))
        } else {
            Ok(None)
        }
    }

    fn next_u32(&mut self) -> Option<u32> {
        (!self.is_done()).then(||{
            self.rest_n -= 1;
            let ret = (self.bits % self.fmt.variants()) as u32;
            self.bits /= self.fmt.variants();
            ret
        })
    }
}

#[derive(Clone, Copy)]
pub struct S3TimeWriter { fmt: TimeFormat }
impl S3TimeWriter {
    #[inline(always)]
    pub fn new(fmt: TimeFormat) -> Self {
        Self { fmt }
    }   
}

impl S3WriterInfo for S3TimeWriter {
    fn bits_once(&self) -> u8 {
        self.fmt.bit_size()
    }

    fn s3_once(&self) -> u64 {
        self.fmt.variants()
    }
}

impl<W: WriteExt> S3Writer<W> for S3TimeWriter {
    type Error = std::io::Error;

    /// # Panics
    /// * if `self.s3_once() <= x`
    fn write(&mut self, x: u64, w: &mut W) -> Result<(), Self::Error> {
        debug_assert!(x < self.s3_once());

        match self.fmt {
            TimeFormat::HM => {
                let m = x % 60;
                let h = x / 60;

                write_n2(w, h as u16)?;
                w.write_char(':')?;
                write_n2(w, m as u16)?;
            }
            TimeFormat::HMS => {
                let s = x % 60;
                let x = x / 60;

                let m = x % 60;
                let h = x / 60;

                write_n2(w, h as u16)?;
                w.write_char(':')?;
                write_n2(w, m as u16)?;
                w.write_char(':')?;
                write_n2(w, s as u16)?;
            }
            TimeFormat::HMSMill => {
                let ms = x % 1000;
                let x = x / 1000;

                let s = x % 60;
                let x = x / 60;
                
                let m = x % 60;
                let h = x / 60;

                write_n2(w, h as u16)?;
                w.write_char(':')?;
                write_n2(w, m as u16)?;
                w.write_char(':')?;
                write_n2(w, s as u16)?;
                w.write_char('.')?;
                write_n3(w, ms as u16)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rand::{RngCore, rng};
    use crate::text::str_writer::WriterFmt;

    use super::*;

    #[test]
    fn test_num_conv() {
        let chunks = vec![
            0b_10_1010_1101,
            0b_01_1010_1101,
            0b_01_0011_0001 + 23,
            0b_11_1111_1111,
            0b_00_1000_1000,
            0b_00_0011_1100,
        ];
        let expects = vec![
            "11:25",
            "07:09",
            "05:28",
            "17:03",
            "02:16",
            "01:00",
        ];

        let time = String::with_capacity(5);
        let mut time = WriterFmt::new(time);

        for (chunk, expect) in chunks.into_iter().zip(expects) {
            time.clear();

            let mut b2t = BitsToTime::new_hm(chunk, 1);
            assert!(!b2t.is_done());
            b2t.write(&mut time).unwrap().unwrap();
            assert!(b2t.is_done());
            assert_eq!(time.as_ref(), &expect);


            let mut t2b = TimeToBits::new(1, TimeFormat::HM);
            assert_eq!(t2b.next(&time), Some(chunk));
        }

        let mut rng = rng();
        let mask_hm = TimeToBits::mask(5, TimeFormat::HM);
        let mask_hms = TimeToBits::mask(2, TimeFormat::HMS);
        let mask_hmsmill = TimeToBits::mask(1, TimeFormat::HMSMill);

        for _ in 0..32 {
            let chunk = rng.next_u64();

            // HM:
            time.clear();
            let chunk_hm = chunk & mask_hm;
            let mut b2t = BitsToTime::new_hm(chunk_hm, 5);
            while let Some(_) = b2t.write(&mut time).unwrap() {
                time.push(' ');
            }
            let mut t2b = TimeToBits::new(5, TimeFormat::HM);
            for time_s in time.split(' ').filter(|x|!x.is_empty()) {
                t2b.next(time_s);
            }
            assert!(t2b.is_done());
            assert_eq!(t2b.try_take().unwrap(), chunk_hm);
            
            // HMS:
            time.clear();
            let chunk_hms = chunk & mask_hms;
            let mut b2t = BitsToTime::new_hms(chunk_hms, 2);
            while let Some(_) = b2t.write(&mut time).unwrap() {
                time.push(' ');
            }
            let mut t2b = TimeToBits::new(2, TimeFormat::HMS);
            for time_s in time.split(' ').filter(|x|!x.is_empty()) {
                t2b.next(time_s);
            }
            assert!(t2b.is_done());
            assert_eq!(t2b.try_take().unwrap(), chunk_hms);
            
            // HMS_MS:
            time.clear();
            let chunk_hmsmill = chunk & mask_hmsmill;
            let mut b2t = BitsToTime::new_hms_ms(chunk_hmsmill, 1);
            while let Some(_) = b2t.write(&mut time).unwrap() {
                time.push(' ');
            }
            let mut t2b = TimeToBits::new(1, TimeFormat::HMSMill);
            for time_s in time.split(' ').filter(|x|!x.is_empty()) {
                t2b.next(time_s);
            }
            assert!(t2b.is_done());
            assert_eq!(t2b.try_take().unwrap(), chunk_hmsmill);
        }
    }

    
    #[test]
    fn test_bit_loss() {
        let bit_loss_proc = |time_n: u32| {
            let time_n = time_n as f32;
            let bit_per_time = f32::log2(24.0 * 60.0);
            let real_bits = f32::floor(bit_per_time * time_n);
            let delta_bits = bit_per_time * time_n - real_bits;
            let proc = 100.00 * delta_bits / real_bits;
            println!("n = {time_n:1} (loss = {proc:.3}%)");
            return proc
        };

        let mut best_proc = bit_loss_proc(1);
        let mut best_n = 1;

        for time_n in 1..=6 {
            let proc = bit_loss_proc(time_n);
            if proc < best_proc {
                best_proc = proc;
                best_n = time_n;
            }
        }

        assert!(best_proc < 1.);
        assert_eq!(best_n, 5);
    }
}
