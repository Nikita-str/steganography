use std::{borrow::Cow, collections::HashMap};

use crate::text::s3::{RngMinimal, S3WriterInfo, S3WriterRand};
use crate::text::str_writer::WriteExt;


enum TxtEnum {
    Small(Vec<Cow<'static, str>>),
    Big(HashMap<u32, Cow<'static, str>>),
}
impl TxtEnum {
    fn take_small(self) -> Vec<Cow<'static, str>> {
        match self {
            TxtEnum::Small(small) => small,
            TxtEnum::Big(_) => panic!("Self is not `::Small`"),
        }
    }

    fn transmute_small_into_big(&mut self) {
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

    /// # Panics 
    /// * if `self.variation_amount() <= 1`
    pub fn seal(mut self) -> TxtVariationSealed {
        if self.s3 == 0 || self.s3 == 1 {
            self.set_s3(self.variation_amount());
        }

        assert!(self.variation_amount() > 1);

        TxtVariationSealed::new(self)
    }
}

pub struct TxtVariationSealed {
    inner: TxtVariation,
    ratio: u8,
}

impl TxtVariationSealed {
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

impl S3WriterInfo for TxtVariationSealed {
    fn bits_once(&self) -> u8 {
        (self.s3_once().ilog2() + 1) as u8
    }

    fn s3_once(&self) -> u64 {
        self.inner.s3 as u64
    }
}

impl<W: WriteExt, Rng: RngMinimal> S3WriterRand<W, Rng> for TxtVariationSealed {
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

#[cfg(test)]
mod tests {
    use rand::rng;
    use crate::text::str_writer::WriterFmt;
    use super::*;

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
            let mut var = var.seal();
            
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