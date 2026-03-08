use std::{borrow::Cow, collections::HashMap};

use crate::text::s3::{RngMinimal, S3Reader, S3WriterInfo, S3WriterRand};
use crate::text::str_reader::StrReadWraper;
use crate::text::str_writer::WriteExt;

#[derive(Clone)]
enum TxtEnum {
    Small(Vec<Cow<'static, str>>),
    Big(HashMap<u32, Cow<'static, str>>),
}
impl TxtEnum {
    fn is_small(&self) -> bool {
        matches!(self, Self::Small(_))
    }

    fn take_small(self) -> Vec<Cow<'static, str>> {
        match self {
            TxtEnum::Small(small) => small,
            TxtEnum::Big(_) => panic!("Self is not `::Small`"),
        }
    }

    fn take_big(self) -> HashMap<u32, Cow<'static, str>> {
        match self {
            TxtEnum::Big(big) => big,
            TxtEnum::Small(_) => panic!("Self is not `::Big`"),
        }
    }

    fn transmute_small_into_big(&mut self) {
        if !self.is_small() { return }

        let mut small = TxtEnum::Small(Vec::new());
        std::mem::swap(self, &mut small);
        
        let mut i = 0;
        let big = small.take_small().into_iter().map(|x|{
            i += 1;
            (i - 1, x)
        }).collect();
        
        *self = TxtEnum::Big(big);
    }

    fn find_str(&self, val: u32) -> Option<&str> {
        let x = match self {
            TxtEnum::Small(small) => small.get(val as usize),
            TxtEnum::Big(big) => big.get(&val),
        };
        x.map(|x|x.as_ref())
    }
}

#[derive(Clone)]
pub struct TxtVariation {
    inner: TxtEnum,
    s3: u32,
}

impl TxtVariation {
    pub fn new(capacity: usize) -> Self {
        Self {
            s3: 0,
            inner: if capacity <= 8 {
                TxtEnum::Small(Vec::with_capacity(8))
            } else {
                TxtEnum::Big(HashMap::with_capacity(capacity))
            },
        }
    }

    pub fn variation_amount(&self) -> u32 {
        match &self.inner {
            TxtEnum::Small(small) => small.len() as u32,
            TxtEnum::Big(big) => big.len() as u32,
        }
    }

    /// # Panics
    /// * if `self.variation_amount() < s3`
    pub fn set_s3(&mut self, s3: u32) {
        assert!(s3 <= self.variation_amount());
        self.s3 = s3;
    }

    pub fn add_string_iter(&mut self, iter: impl IntoIterator<Item = String>) {
        iter.into_iter().for_each(|s|self.add_string(s));
    }

    pub fn add_string(&mut self, s: String) {
        self.add_cow(Cow::Owned(s));
    }

    pub fn add_str_iter(&mut self, iter: impl IntoIterator<Item = &'static str>) {
        iter.into_iter().for_each(|s|self.add_str(s));
    }

    pub fn add_str(&mut self, s: &'static str) {
        self.add_cow(Cow::Borrowed(s));
    }
    
    pub fn add_cow(&mut self, cow: Cow<'static, str>) {
        if let TxtEnum::Small(small) = &self.inner {
            if small.len() >= 8 {
                self.inner.transmute_small_into_big();
            }
        }

        match &mut self.inner {
            TxtEnum::Small(small) => {
                small.push(cow);
            },
            TxtEnum::Big(big) => {
                let val = big.len() as u32;
                big.insert(val, cow);
            }
        }
    }


    fn seal_prepare(&mut self) {
        if self.s3 == 0 || self.s3 == 1 {
            self.set_s3(self.variation_amount());
        }

        assert!(self.variation_amount() > 1);
    }

    /// # Panics 
    /// * if `self.variation_amount() <= 1`
    pub fn seal_w(mut self) -> TxtVariationWriter {
        self.seal_prepare();
        TxtVariationWriter::new(self)
    }
    
    /// # Panics 
    /// * if `self.variation_amount() <= 1`
    pub fn seal_r(mut self, valid_char: IsValidChar, default: Option<u32>) -> TxtVariationReader {
        self.seal_prepare();
        TxtVariationReader::new(self, valid_char, default)
    }
}

pub struct TxtVariationWriter {
    inner: TxtVariation,
    ratio: u8,
}

impl TxtVariationWriter {
    pub fn new(inner: TxtVariation) -> Self {
        let s3 = inner.s3;
        let real_s3 = inner.variation_amount();

        Self {
            inner,
            ratio: if real_s3 == s3 {
                1
            } else {
                let ration_int = real_s3 / s3;
                let rest = (real_s3 % s3) > 0;
                let ratio = ration_int + (rest as u32);
                if ration_int > 255 {
                    panic!("TxtVariationSealed: To high ratio");
                }
                ratio as u8
            }
        }
    }
    pub fn into_inner(self) -> TxtVariation {
        self.inner
    }
}

impl S3WriterInfo for TxtVariationWriter {
    fn bits_once(&self) -> u8 {
        let s3 = self.s3_once();
        let is_pow_2 = (s3 & (s3 - 1)) == 0;
        self.s3_once().ilog2() as u8 + (!is_pow_2) as u8
    }

    fn s3_once(&self) -> u64 {
        self.inner.s3 as u64
    }
}

impl<W: WriteExt, Rng: RngMinimal> S3WriterRand<W, Rng> for TxtVariationWriter {
    type Error = std::io::Error;

    /// # Panics
    /// * if `self.s3_once() <= x`
    fn write(&mut self, mut x: u64, w: &mut W, rng: &mut Rng) -> Result<(), Self::Error> {
        debug_assert!(x < self.s3_once());

        if self.ratio != 1 {
            let k = rng.r8_range(0..=(self.ratio - 1)) as u64;
            let mut x2 = x + self.inner.s3 as u64 * k;

            if x2 >= self.inner.variation_amount() as u64 {
                x2 -= self.inner.s3 as u64;
            }
            debug_assert!(x2 < self.inner.variation_amount() as u64);
            
            x = x2
        }
        
        let Some(key) = self.inner.inner.find_str(x as u32) else {
            return Err(std::io::Error::other(format!("No key for {x} value?!")))
        };

        w.write_str(key)
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub trait IsValidCharSeq {
    fn clone_boxed(&self) -> Box<dyn IsValidCharSeq>;

    fn reset(&mut self);
    fn is_valid(&mut self, c: char) -> bool;
}

#[derive(Clone)]
pub struct IsValidCharEng {
    lower: bool,
    capital: bool,
}
impl IsValidCharEng {
    pub fn new_lower() -> Self {
        Self {
            lower: true,
            capital: false,
        }
    }
    pub fn new_capital() -> Self {
        Self {
            lower: false,
            capital: true,
        }
    }
    pub fn new_any() -> Self {
        Self {
            lower: true,
            capital: true,
        }
    }
}

impl IsValidCharSeq for IsValidCharEng {
    fn reset(&mut self) { }

    fn is_valid(&mut self, c: char) -> bool {
        match c {
            'a'..='z' if self.lower => true,
            'A'..='Z' if self.capital => true,
            _ => false,
        }
    }
    
    fn clone_boxed(&self) -> Box<dyn IsValidCharSeq> {
        Box::new(self.clone())
    }
}

#[derive(Clone)]
pub struct IsValidCharEngNum {
    num: IsValidCharEng,
    first: bool,
}
impl IsValidCharEngNum {
    pub fn new(num: IsValidCharEng) -> Self {
        Self { num, first: true }
    }
}
impl IsValidCharSeq for IsValidCharEngNum {     
    fn reset(&mut self) { self.first = true; }

    fn is_valid(&mut self, c: char) -> bool {
        match c {
            '0'..='9' if !self.first => true,
            _ => {
                self.first = false;
                self.num.is_valid(c)
            }
        }
    }

    fn clone_boxed(&self) -> Box<dyn IsValidCharSeq> {
        Box::new(self.clone())
    }
}

pub enum IsValidChar {
    Eng(IsValidCharEng),
    EngThenNum(IsValidCharEngNum),
    Dyn(Box<dyn IsValidCharSeq>)
}
impl Clone for IsValidChar {
    fn clone(&self) -> Self {
        match self {
            Self::Eng(arg0) => Self::Eng(arg0.clone()),
            Self::EngThenNum(arg0) => Self::EngThenNum(arg0.clone()),
            Self::Dyn(arg0) => Self::Dyn(arg0.clone_boxed()),
        }
    }
}
impl IsValidCharSeq for IsValidChar {
    fn reset(&mut self) {
        match self {
            IsValidChar::Eng(x) => x.reset(),
            IsValidChar::EngThenNum(x) => x.reset(),
            IsValidChar::Dyn(x) => x.reset(),
        }
    }

    fn is_valid(&mut self, c: char) -> bool {
        match self {
            IsValidChar::Eng(x) => x.is_valid(c),
            IsValidChar::EngThenNum(x) => x.is_valid(c),
            IsValidChar::Dyn(x) => x.is_valid(c),
        }
    }
    
    fn clone_boxed(&self) -> Box<dyn IsValidCharSeq> {
        unimplemented!()
    }

    
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

struct InnerReader {
    map: HashMap<Cow<'static, str>, u32>,
    valid_chars: IsValidChar, //MAYBE: use prefix tree
}

pub struct TxtVariationReader {
    inner: Box<InnerReader>,
    s3: u32,
    unkn_value: Option<u32>,
}
impl TxtVariationReader {
    pub fn new(mut var: TxtVariation, valid_char: IsValidChar, default: Option<u32>) -> Self {
        var.inner.transmute_small_into_big();
        let map = var.inner.take_big().into_iter().map(|(k,v)|(v, k)).collect();

        if let Some(x) = default {
            assert!(x < var.s3, "must be: {x} < {}", var.s3);
        }

        Self {
            inner: Box::new(InnerReader {
                map,
                valid_chars: valid_char,
            }),
            s3: var.s3,
            unkn_value: default,
        }
    }
}

impl S3WriterInfo for TxtVariationReader {
    fn bits_once(&self) -> u8 {
        let s3 = self.s3_once();
        let is_pow_2 = (s3 & (s3 - 1)) == 0;
        self.s3_once().ilog2() as u8 + (!is_pow_2) as u8
    }

    fn s3_once(&self) -> u64 {
        self.s3 as u64
    }
}

impl<R: std::io::Read> S3Reader<StrReadWraper<R>> for TxtVariationReader {
    type Error = std::io::Error;

    fn read(&mut self, r: &mut StrReadWraper<R>) -> Result<u64, Self::Error> {
        let valid_chars = &mut self.inner.valid_chars;
        valid_chars.reset();
        let mut s = &*r.read_while(|c|valid_chars.is_valid(c), true)?;
        self.read(&mut s)
    }
}

impl S3Reader<&str> for TxtVariationReader {
    type Error = std::io::Error;

    fn read(&mut self, s: &mut &str) -> Result<u64, Self::Error> {
        match self.inner.map.get(*s) {
            Some(&x) => Ok(x as u64),
            None => match self.unkn_value {
                Some(x) => Ok(x as u64),
                None => panic!("TxtVariationReader: {s:?} unknown variation!"),
            }
        }
    }
}


// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use rand::rng;
    use crate::text::str_writer::WriterFmt;
    use super::*;

    #[test]
    pub fn test_txt_rw() {        
        let v = vec![
            "Abobus", "Autobus", "Bus", "Busy", 
            "Dinosaurs", "Agrosaurus", "Triceratops", "TRex",
            "Diplodocus", "Brontosaurus", "Stegosaurus", "Streptococcus",
        ];

        let mut var = TxtVariation::new(12);
        var.add_str_iter(v.iter().map(|x|*x));

        let mut w = var.clone().seal_w();
        let mut r = var.seal_r(IsValidChar::Eng(IsValidCharEng::new_any()), Some(9));

        for x in 0..=11 {
            let mut rng = rng();
            let mut str = WriterFmt::new(String::new());
            w.write(x, &mut str, &mut rng).unwrap();

            let mut str = StrReadWraper::new_std(str.as_bytes());
            let y = r.read(&mut str).unwrap();
            assert_eq!(x, y);
        }
    }

    #[test]
    pub fn test_txt_bit_once() {
        let mut txt_var = TxtVariation::new(10);
        txt_var.add_str_iter(["A", "B", "C"].into_iter());
        let x = txt_var.seal_w();
        assert_eq!(2, x.bits_once());
        
        let mut txt_var = TxtVariation::new(10);
        txt_var.add_str_iter(["A", "B", "C", "D"].into_iter());
        let x = txt_var.seal_w();
        assert_eq!(2, x.bits_once());
        
        let mut txt_var = TxtVariation::new(10);
        txt_var.add_str_iter(["A", "B", "C", "D", "E"].into_iter());
        let x = txt_var.seal_w();
        assert_eq!(3, x.bits_once());
    }

    #[test]
    pub fn test_txt_enum() {
        let v = vec![
            "Abobus", "Autobus", "Bus", "Busy", 
            "Dinosaurs", "Agrosaurus", "Triceratops", "T-Rexus",
            "Diplodocus", "Brontosaurus", "Stegosaurus", "Streptococcus",
        ];
        let mut expect = Vec::with_capacity(2);

        let mut rng = rng();
        let str = String::with_capacity(10);
        let mut str = WriterFmt::new(str);

        for s3 in [7, 12, 3, 5] {
            let mut var = TxtVariation::new(12);
            var.add_str_iter(v.iter().map(|x|*x));
            var.set_s3(s3);
            let mut var = var.seal_w();
            
            for x in 0..100u32 {
                str.clear();
                
                let x = x % s3;
                var.write(x as u64, &mut str, &mut rng).unwrap();
                
                let s = str.as_ref();
                let i = x as usize;
                expect.clear();
                for j in 0.. {
                    let i = i + j * s3 as usize;
                    if i < v.len() {
                        expect.push(v[i]);
                    } else {
                        break
                    }
                }

                assert!(expect.contains(&s.as_str()), "{x} --> {s:?}   expect in: {expect:?}");
            }
        }

    }
}