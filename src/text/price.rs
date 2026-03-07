use crate::text::s3::{RngMinimal, S3Reader, S3Writer, S3WriterInfo, S3WriterRand};
use crate::text::str_reader::{PeakableReadExt, StrReadWraper};
use crate::text::str_writer::WriteExt;
use crate::text::num::{S3NumsReader, S3RevNumsWriter};

#[derive(Clone, Copy)]
pub enum PostfixSymb {
    /// `9`: `$12.99` or `$1299` or `Baaaaka`
    Nine,
    /// `0`: `$12.00` or `$12.50` or `$1200` 
    Zero,
}
impl PostfixSymb {
    #[inline]
    pub fn to_n1(self) -> u8 {
        match self {
            PostfixSymb::Nine => 9,
            PostfixSymb::Zero => 0,
        }
    }
}

#[derive(Clone, Copy)]
pub struct PricePostfixInfo {
    postfix_len: u8,
    postfix_symb: PostfixSymb,
}
impl PricePostfixInfo {
    /// Empty postfix.
    pub fn new_empty() -> Self {
        Self {
            postfix_len: 0,
            postfix_symb: PostfixSymb::Zero,
        }
    }

    /// Postfix that generates `len` zeroes at the end.
    /// 
    /// Exmaple: `new_0(2)` --> `99`
    pub fn new_0(len: u8) -> Self {
        Self {
            postfix_len: len,
            postfix_symb: PostfixSymb::Zero,
        }
    }

    /// Postfix that generates `len` nines at the end.
    /// 
    /// Exmaple: `new_9(3)` --> `00`
    pub fn new_9(len: u8) -> Self {
        Self {
            postfix_len: len,
            postfix_symb: PostfixSymb::Nine,
        }
    }

    pub fn is_zero_ty(&self) -> bool {
        matches!(self.postfix_symb, PostfixSymb::Zero)
    }

    pub fn remove_postfix_from_bstr<'s>(&self, str: &'s [u8]) -> &'s [u8] {
        &str[0..str.len().saturating_sub(self.postfix_len as usize)]
    }

    pub fn wrtie<W: WriteExt + ?Sized>(&self, w: &mut W) -> Result<(), std::io::Error> {
        for _ in 0..self.postfix_len {
            w.write_n1z(self.postfix_symb.to_n1())?;
        }
        Ok(())
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Clone, Copy)]
pub struct S3IntPriceWriter {
    rev_num_writer: S3RevNumsWriter,
    min_prefix: u8,
    prefix_range: u8,
    postfix: PricePostfixInfo,
}

impl S3IntPriceWriter {
    /// # Panics
    /// * if `int_len == 0`
    pub fn new(int_len: u8, prefix_range: u8, postfix: PricePostfixInfo) -> Self {
        assert!(int_len != 0);
        
        let mut rev_num_writer = S3RevNumsWriter::new(int_len, false);
        rev_num_writer.set_allow_empty(true);

        Self {
            min_prefix: 0,
            rev_num_writer,
            prefix_range,
            postfix,
        }
    }
    
    pub fn create_reader(&self) -> S3IntPriceReader {
        S3IntPriceReader {
            num_reader: S3NumsReader::new(self.rev_num_writer.len(), true, false),
            postfix: self.postfix,
        }
    }

    pub fn set_min_prefix(&mut self, min_prefix: u8) {
        assert!(min_prefix <= self.prefix_range);
        self.min_prefix = min_prefix;
    } 
}

impl S3WriterInfo for S3IntPriceWriter {
    fn bits_once(&self) -> u8 {
        self.rev_num_writer.bits_once()
    }

    fn s3_once(&self) -> u64 {
        self.rev_num_writer.s3_once()
    }
}

impl<W: WriteExt, Rng: RngMinimal> S3WriterRand<W, Rng> for S3IntPriceWriter {
    type Error = std::io::Error;

    /// # Panics
    /// * if `self.s3_once() <= x`
    fn write(&mut self, x: u64, w: &mut W, rng: &mut Rng) -> Result<(), Self::Error> {
        debug_assert!(x < self.s3_once());

        let mut prefix = rng.r8_range(0..=self.prefix_range);
        if prefix < self.min_prefix { prefix = self.min_prefix };
        w.write_n3e(prefix as u16)?;

        let is_zero = prefix == 0;
        self.rev_num_writer.set_zeroed(!is_zero);
        self.rev_num_writer.write(x, w)?;

        // we must write only 0 zero in the next case:
        if prefix == 0 && x == 0 && self.postfix.is_zero_ty() {
            return w.write_char('0')
        }

        self.postfix.wrtie(w)
    }
}

#[derive(Clone, Copy)]
pub struct S3IntPriceReader {
    num_reader: S3NumsReader,
    postfix: PricePostfixInfo,
}
impl S3IntPriceReader {
    /// # Panics
    /// * if `int_len == 0`
    pub fn new(int_len: u8, postfix: PricePostfixInfo) -> Self {
        assert!(int_len != 0);
        
        let num_reader = S3NumsReader::new(int_len, true, false);

        Self {
            num_reader,
            postfix,
        }
    }
}

impl S3WriterInfo for S3IntPriceReader {
    fn bits_once(&self) -> u8 {
        self.num_reader.bits_once()
    }

    fn s3_once(&self) -> u64 {
        self.num_reader.s3_once()
    }
}

impl<R: std::io::Read> S3Reader<StrReadWraper<R>> for S3IntPriceReader {
    type Error = std::io::Error;
    
    fn read(&mut self, r: &mut StrReadWraper<R>) -> Result<u64, Self::Error> {
        let price_str = r.read_nums(true)?;
        if price_str.is_empty() {
            return Err(std::io::Error::other("S3IntPriceReader: empty price?!"));
        }
        if price_str == "0" {
            return Ok(0)
        }
        let price_str = price_str.as_bytes();
        let price_str = self.postfix.remove_postfix_from_bstr(price_str);
        if price_str.is_empty() {
            return Ok(0);
        }

        let real_part_from = price_str.len().saturating_sub(self.num_reader.len() as usize);
        let mut real_price_str = &price_str[real_part_from..];

        self.num_reader.read(&mut real_price_str)
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Clone, Copy)]
pub enum FracVariation {
    /// 0 bits; ending = `.00` 
    Zeros,
    /// 0 bits; ending = `.99` 
    Nines,

    /// 1 bit; ending = `.00` | `.50` 
    Fifty,
    /// 1 bit; ending = `.00` | `.99` 
    ZeroOrNinty,

    /// 3.32 bits; ending = `.[x]0` 
    HighNum,
    /// 4.32 bits; ending = `.[x][0|5]` 
    Step5,
}

impl FracVariation {
    #[inline]
    pub fn s3_once(self) -> u64 {
        match self {
            FracVariation::Zeros => 1,
            FracVariation::Nines => 1,
            FracVariation::Fifty => 2,
            FracVariation::ZeroOrNinty => 2,
            FracVariation::HighNum => 10,
            FracVariation::Step5 => 20,
        }
    }
    
    pub fn wrtie<W: WriteExt + ?Sized>(&self, frac: u8, w: &mut W) -> Result<(), std::io::Error> {
        match self {
            FracVariation::Zeros => w.write_n2z(0),
            FracVariation::Nines => w.write_n2z(99),
            FracVariation::Fifty => w.write_n2z(frac * 50),
            FracVariation::ZeroOrNinty => w.write_n2z(frac * 99),
            FracVariation::HighNum => w.write_n2z(frac * 10),
            FracVariation::Step5 => w.write_n2z(frac * 5),
        }
    }
        
    pub fn read<R: PeakableReadExt + ?Sized>(&self, r: &mut R) -> Result<Option<u8>, std::io::Error> {
        let frac = r.read_n2z()?;
        let val = match self {
            FracVariation::Zeros => None,
            FracVariation::Nines => None,
            FracVariation::Fifty => Some(frac / 50),
            FracVariation::ZeroOrNinty => Some(frac / 99),
            FracVariation::HighNum => Some(frac / 10),
            FracVariation::Step5 => Some(frac / 5),
        };
        Ok(val)
    }
}

#[derive(Clone, Copy)]
pub struct S3FloatPriceWriter {
    int_part: S3IntPriceWriter,
    frac_variation: FracVariation,
}

impl S3FloatPriceWriter {
    pub fn new(int_part: S3IntPriceWriter, frac_variation: FracVariation) -> Self {
        Self {
            int_part,
            frac_variation,
        }
    }

    pub fn create_reader(&self) -> S3FloatPriceReader {
        S3FloatPriceReader {
            int_part: self.int_part.create_reader(),
            frac_variation: self.frac_variation,
        }
    }
}

impl S3WriterInfo for S3FloatPriceWriter {
    fn bits_once(&self) -> u8 {
        (self.s3_once().ilog2() + 1) as u8
    }

    fn s3_once(&self) -> u64 {
        self.int_part.s3_once() * self.frac_variation.s3_once()
    }
}

impl<W: WriteExt, Rng: RngMinimal> S3WriterRand<W, Rng> for S3FloatPriceWriter {
    type Error = std::io::Error;

    /// # Panics
    /// * if `self.s3_once() <= x`
    fn write(&mut self, x: u64, w: &mut W, rng: &mut Rng) -> Result<(), Self::Error> {
        debug_assert!(x < self.s3_once());

        let float_var = self.frac_variation.s3_once();
        let frac = (x % float_var) as u8;
        let int = x / float_var;

        self.int_part.write(int, w, rng)?;
        w.write_char('.')?;
        self.frac_variation.wrtie(frac, w)
    }
}


#[derive(Clone, Copy)]
pub struct S3FloatPriceReader {
    int_part: S3IntPriceReader,
    frac_variation: FracVariation,
}

impl S3FloatPriceReader {
    pub fn new(int_part: S3IntPriceReader, frac_variation: FracVariation) -> Self {
        Self {
            int_part,
            frac_variation,
        }
    }
}

impl S3WriterInfo for S3FloatPriceReader {
    fn bits_once(&self) -> u8 {
        (self.s3_once().ilog2() + 1) as u8
    }

    fn s3_once(&self) -> u64 {
        self.int_part.s3_once() * self.frac_variation.s3_once()
    }
}

impl<R: std::io::Read> S3Reader<StrReadWraper<R>> for S3FloatPriceReader {
    type Error = std::io::Error;

    /// # Panics
    /// * if `self.s3_once() <= x`
    fn read(&mut self, r: &mut StrReadWraper<R>) -> Result<u64, Self::Error> {
        let float_var = self.frac_variation.s3_once();
        let int = self.int_part.read(r)?;
        if r.wrap_mut().read_char()? != '.' {
            return Err(std::io::Error::other("S3FloatPriceReader: expect a dot"))
        }
        let frac = self.frac_variation.read(r.wrap_mut())?.unwrap_or(0);

        Ok(int * float_var + frac as u64)
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use crate::text::str_writer::WriterFmt;
    use super::*;

    struct Rng {
        range_res: Vec<u8>,
        u8_res: Vec<u8>,
    }
    impl Rng {
        pub fn new(mut range_res: Vec<u8>, mut u8_res: Vec<u8>) -> Self {
            range_res.reverse();
            u8_res.reverse();
            Self {
                range_res,
                u8_res,
            }
        }
    }
    impl RngMinimal for Rng {
        fn r8(&mut self) -> u8 {
            self.u8_res.pop().unwrap()
        }
    
        fn r64(&mut self) -> u64 {
            unimplemented!()
        }
    
        fn r8_range(&mut self, _: std::ops::RangeInclusive<u8>) -> u8 {
            self.range_res.pop().unwrap()
        }

        fn r64_range_excl(&mut self, _: std::ops::Range<u64>) -> u64 {
            unimplemented!()
        }
    }

    #[test]
    fn test_int_price() {        
        let str = String::with_capacity(10);
        let mut str = WriterFmt::new(str);

        // `00`:
        let mut wr = S3IntPriceWriter::new(2, 12, PricePostfixInfo::new_0(2));
        let mut rng = Rng::new(vec![1, 12, 0, 3, 0], vec![]);
        
        let tests = [85, 20, 20, 0, 0];
        let expects = ["15800", "120200", "200", "30000", "0"];

        for (test, expect) in tests.into_iter().zip(expects) {
            str.clear();
            wr.write(test, &mut str, &mut rng).unwrap();
            assert_eq!(str.as_ref(), expect);
        }

        // `99`:
        let mut wr = S3IntPriceWriter::new(2, 12, PricePostfixInfo::new_9(2));
        let mut rng = Rng::new(vec![1, 12, 0, 3, 0], vec![]);
        
        let tests = [85, 20, 20, 0, 0];
        let expects = ["15899", "120299", "299", "30099", "99"];

        for (test, expect) in tests.into_iter().zip(expects) {
            str.clear();
            wr.write(test, &mut str, &mut rng).unwrap();
            assert_eq!(str.as_ref(), expect);
        }
    }
    

    #[test]
    fn test_float_price() {        
        let str = String::with_capacity(10);
        let mut str = WriterFmt::new(str);

        let mut int_price = S3IntPriceWriter::new(2, 3, PricePostfixInfo::new_empty());
        int_price.set_min_prefix(1);

        let mut w = S3FloatPriceWriter::new(int_price, FracVariation::HighNum);
        let mut r = w.create_reader();
        let mut rng = Rng::new(vec![1, 2, 0, 3, 0, 3, 3], vec![]);
        
        let tests = [851, 204, 200, 0, 0, 14, 104];
        let expects = ["158.10", "202.40", "102.00", "300.00", "100.00", "310.40", "301.40"];

        for (test, expect) in tests.into_iter().zip(expects) {
            str.clear();
            w.write(test, &mut str, &mut rng).unwrap();
            assert_eq!(str.as_ref(), expect);

            let mut reader = StrReadWraper::new_std(str.as_bytes());
            let val = r.read(&mut reader).unwrap();
            assert_eq!(val, test);
        }
        
        //

        let mut int_price = S3IntPriceWriter::new(2, 3, PricePostfixInfo::new_empty());
        int_price.set_min_prefix(1);

        let mut w = S3FloatPriceWriter::new(int_price, FracVariation::Step5);
        let mut r = w.create_reader();
        let mut rng = Rng::new(vec![1, 2, 0, 3, 0, 0, 2], vec![]);
        
        let tests = [85 * 20 + 11, 20 * 20 + 9, 20 * 20 + 18, 10, 5, 0, 3 * 20 + 10];
        let expects = ["158.55", "202.45", "102.90", "300.50", "100.25", "100.00", "230.50"];

        for (test, expect) in tests.into_iter().zip(expects) {
            str.clear();
            w.write(test, &mut str, &mut rng).unwrap();
            assert_eq!(str.as_ref(), expect);

            let mut reader = StrReadWraper::new_std(str.as_bytes());
            let val = r.read(&mut reader).unwrap();
            assert_eq!(val, test);
        }
        
        //

        let int_price = S3IntPriceWriter::new(2, 3, PricePostfixInfo::new_empty());

        let mut w = S3FloatPriceWriter::new(int_price, FracVariation::Step5);
        let mut r = w.create_reader();
        let mut rng = Rng::new(vec![1, 2, 0, 0, 0, 0, 0], vec![]);
        
        let tests = [85 * 20 + 11, 20 * 20 + 9, 20 * 20 + 18, 10, 5, 0, 3 * 20 + 10];
        let expects = ["158.55", "202.45", "2.90", "0.50", "0.25", "0.00", "30.50"];

        for (test, expect) in tests.into_iter().zip(expects) {
            str.clear();
            w.write(test, &mut str, &mut rng).unwrap();
            assert_eq!(str.as_ref(), expect);
            
            let mut reader = StrReadWraper::new_std(str.as_bytes());
            let val = r.read(&mut reader).unwrap();
            assert_eq!(val, test);
        }
    }
}