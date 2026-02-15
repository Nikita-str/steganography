use std::borrow::Cow;

use crate::text::s3::{RngMinimal, S3Full, S3TypeWriter, S3WriterInfo};
use crate::text::str_writer::WriteExt;

pub struct CsvHeaderWriter {
    names: Vec<Cow<'static, str>>,
}

impl CsvHeaderWriter {
    pub fn new() -> Self {
        Self {
            names: Vec::with_capacity(8),
        }
    }

    pub fn add_name_str(&mut self, str: &'static str) {
        self.names.push(Cow::Borrowed(str));
    }

    pub fn add_name_string(&mut self, str: String) {
        self.names.push(Cow::Owned(str));
    }

    pub fn write_header<W: WriteExt + ?Sized>(&self, w: &mut W, sep: &CsvSepInfo) -> Result<(), std::io::Error> {
        let mut is_first = true;
        for name in &self.names {
            if !is_first {
                w.write_char(sep.sep)?;
            }
            is_first = false;
            w.write_str(name)?;
        }

        if !sep.end.is_empty() {
            w.write_str(sep.end.as_ref())?;
        }
        Ok(())
    }
}

pub struct CsvLineWriter<W, Rng> {
    cols: Vec<S3TypeWriter<W, Rng>>,
    s3: Vec<u64>,
    /// How many bits can fit
    chunk_sz: Vec<u8>,
}

impl<W: WriteExt, Rng: RngMinimal> CsvLineWriter<W, Rng> {
    pub fn new() -> Self {
        Self {
            cols: Vec::with_capacity(8),
            s3: vec![1],
            chunk_sz: vec![0],
        }
    }

    pub fn add_column(&mut self, ty: S3TypeWriter<W, Rng>) {
        let s3 = ty.s3_once();
        let last = self.s3.len() - 1;
        let prev_x = self.s3[last];
        if let Some(x) = prev_x.checked_mul(s3) {
            self.s3[last] = x;
            // how many bits can fit:
            self.chunk_sz[last] = x.ilog2() as u8; // + (!x.is_power_of_two()) as u8;
        } else {
            self.s3.push(1);
            self.chunk_sz.push(0);
            self.s3[last] = s3;
            // how many bits can fit:
            self.chunk_sz[last] = s3.ilog2() as u8; // + (!s3.is_power_of_two()) as u8;
        }
        self.cols.push(ty);
    }

    pub fn write_line<R>(&mut self, s3full: &mut S3Full<'_, R, W, Rng>, sep: &CsvSepInfo, fake: bool) -> Result<(), std::io::Error>
    where R: std::io::Read,
    {
        let mut is_eof_stream = false;
        let mut chunk_ind = 0;
        let mut is_first = true;

        for s3w in &mut self.cols {
            if s3full.is_need_chunk() {
                if let Some(chunk_sz) = self.chunk_sz.get(chunk_ind).cloned() {
                    s3full.set_next_chunk(chunk_sz);
                    chunk_ind += 1;
                }
            }

            if !is_first {
                s3full.writer_mut().write_char(sep.sep)?;
            }
            is_first = false;

            is_eof_stream = s3full.write_s3(s3w, fake || is_eof_stream)?;
        }
        
        if !sep.end.is_empty() {
            s3full.writer_mut().write_str(sep.end.as_ref())?;
        }
        Ok(())
    }
}

pub struct CsvSepInfo {
    pub end: Cow<'static, str>,
    pub sep: char,
}
//TODO: CsvSepInfo fns

pub struct CsvWriter<W, R> {
    header: CsvHeaderWriter,
    line: CsvLineWriter<W, R>,
    sep: CsvSepInfo,
}
impl<W: WriteExt, Rng: RngMinimal> CsvWriter<W, Rng> {
    pub fn new_std() -> Self {
        Self::new("\n".into(), ',')
    }

    pub fn new(end: Cow<'static, str>, sep: char) -> Self {
        Self {
            header: CsvHeaderWriter::new(),
            line: CsvLineWriter::new(),
            sep: CsvSepInfo { end, sep },
        }
    }

    pub fn add_column_str(&mut self, str: &'static str, ty: S3TypeWriter<W, Rng>) {
        self.header.add_name_str(str);
        self.line.add_column(ty);
    }
    
    pub fn add_column_string(&mut self, str: String, ty: S3TypeWriter<W, Rng>) {
        self.header.add_name_string(str);
        self.line.add_column(ty);
    }

    pub fn write_all<R>(&mut self, s3full: &mut S3Full<'_, R, W, Rng>) -> Result<(), std::io::Error>
    where R: std::io::Read,
    {
        self.header.write_header(s3full.writer_mut(), &self.sep)?;

        loop {
            self.line.write_line(s3full, &self.sep, false)?;

            // we need writer atleast one line to make non-empty CSV, so break only after first iter
            if s3full.is_eof_stream() {
                break
            }
        }
        Ok(())
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use rand::rng;
    use crate::text::price::{FracVariation, PricePostfixInfo, S3FloatPriceWriter, S3IntPriceWriter};
    use crate::text::s3::S3WriterRandWrap as WrapR;
    use crate::text::str_writer::WriterFmt;
    use crate::text::id::IdWriter;
    use crate::text::time::{S3TimeWriter, TimeFormat};
    use super::*;

    #[test]
    fn test_csv() {
        let mut rng = rng();
        let str = String::with_capacity(1000);
        let mut str = WriterFmt::new(str);

        let mut read = "Давай проверим этот текст, что ли?".as_bytes();
        println!("bit len = {} | {read:?}", read.len() * 8);

        let mut csv = CsvWriter::new_std();
        csv.add_column_str("prod_id",S3TypeWriter::Id(IdWriter::new(21, 2, 1)));

        let int_part = S3IntPriceWriter::new(2, 3, PricePostfixInfo::new_empty());
        let float_price = S3FloatPriceWriter::new(int_part, FracVariation::HighNum);
        csv.add_column_str("price", S3TypeWriter::FloatPrice(float_price));

        csv.add_column_str("time", S3TypeWriter::Time(WrapR(S3TimeWriter::new(TimeFormat::HMS))));

        println!("{:?}", csv.line.s3);
        println!("{:?}", csv.line.chunk_sz);

        let mut s3full = S3Full::new(&mut read, &mut str, &mut rng).unwrap();

        csv.write_all(&mut s3full).unwrap();

        println!("{}", str.as_str());
        println!("lines: {}", str.lines().count());
    }
}

