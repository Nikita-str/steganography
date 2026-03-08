use std::borrow::Cow;

use crate::text::s3::{RngMinimal, S3Full, S3FullReader, S3WriterInfo};
use crate::text::s3_rw_ty::{S3TypeReader, S3TypeWriter};
use crate::text::str_reader::PeakableReadExt;
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

pub struct CsvHeaderReader { len: usize }
impl CsvHeaderReader {
    pub fn read_header<R: PeakableReadExt + ?Sized>(&self, r: &mut R, sep: &CsvSepInfo) -> Result<(), std::io::Error> {
        for _ in 0..(self.len - 1) {
            while r.read_char()? != sep.sep { }
        }

        let end = sep.end.as_bytes();
        let mut ind_sep = 0;
        loop {
            let byte = r.read_byte()?;
            if byte == end[ind_sep] {
                ind_sep += 1;
                if ind_sep == end.len() { break; }
            } else if ind_sep != 0 {
                if end.iter().find(|&&x| x == byte).is_some() {
                    // 01012 but we have 0101[0] -> 010
                    return Err(std::io::Error::other("CsvHeaderReader: separator with duplicated chars is not supported rn"))
                } else {
                    ind_sep = 0;
                }
            }
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
            let last = last + 1;
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

pub struct CsvLineReader<R> {
    cols: Vec<S3TypeReader<R>>,
    s3: Vec<u64>,
    /// How many bits can fit
    chunk_sz: Vec<u8>,
}

impl<R: std::io::Read> CsvLineReader<R> {
    pub fn new() -> Self {
        Self {
            cols: Vec::with_capacity(8),
            s3: vec![1],
            chunk_sz: vec![0],
        }
    }

    pub fn add_column(&mut self, ty: S3TypeReader<R>) {
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
            let last = last + 1;
            self.s3[last] = s3;
            // how many bits can fit:
            self.chunk_sz[last] = s3.ilog2() as u8; // + (!s3.is_power_of_two()) as u8;
        }
        self.cols.push(ty);
    }

    pub fn read_line<W>(&mut self, s3full: &mut S3FullReader<'_, R, W>, sep: &CsvSepInfo, fake: bool) -> Result<(), std::io::Error>
    where W: std::io::Write,
    {
        let mut chunk_ind = 0;
        let mut is_first = true;

        for s3r in &mut self.cols {
            if s3full.is_need_chunk() {
                if let Some(chunk_sz) = self.chunk_sz.get(chunk_ind).cloned() {
                    s3full.set_next_chunk(chunk_sz);
                    chunk_ind += 1;
                }
            }

            if !is_first {
                s3full.reader_mut().read_char_expect(sep.sep, false)?;
            }
            is_first = false;

            if s3full.read_s3(s3r, fake)? {
                return Ok(())
            }
        }
        
        if !sep.end.is_empty() {
            s3full.reader_mut().read_str_expect(sep.end.as_ref(), true)?;
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

pub struct CsvReader<R> {
    header: CsvHeaderReader,
    line: CsvLineReader<R>,
    sep: CsvSepInfo,
}
impl<R: std::io::Read> CsvReader<R> {
    pub fn new_std() -> Self {
        Self::new("\n".into(), ',')
    }

    pub fn new(end: Cow<'static, str>, sep: char) -> Self {
        Self {
            header: CsvHeaderReader { len: 0 },
            line: CsvLineReader::new(),
            sep: CsvSepInfo { end, sep },
        }
    }

    pub fn add_column(&mut self, ty: S3TypeReader<R>) {
        self.line.add_column(ty);
        self.header.len += 1;
    }

    pub fn read_all<W>(&mut self, s3full: &mut S3FullReader<'_, R, W>) -> Result<(), std::io::Error>
    where W: std::io::Write,
    {
        self.header.read_header(s3full.reader_mut(), &self.sep)?;

        loop {
            self.line.read_line(s3full, &self.sep, false)?;

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
    use image::EncodableLayout;
    use rand::rng;
    use crate::text::price::{FracVariation, PricePostfixInfo};
    use crate::text::s3_rw_ty::{S3CtorsRW, S3Signature, S3Type};
    use crate::text::str_reader::StrReadWraper;
    use crate::text::str_writer::WriterFmt;
    use crate::text::time::TimeFormat;
    use super::*;

    #[allow(unused)]
    struct Rng1;
    impl RngMinimal for Rng1 {
        fn r8(&mut self) -> u8 {
            0xFF
        }
    
        fn r64(&mut self) -> u64 {
            0xFFFF_FFFF_FFFF_FFFF
        }
    
        fn r8_range(&mut self, range: std::ops::RangeInclusive<u8>) -> u8 {
            *range.end()
        }
    
        fn r64_range_excl(&mut self, range: std::ops::Range<u64>) -> u64 {
            range.end - 1
        }
    }

    #[test]
    fn test_csv() {
        let mut rng = rng();
        let mut str = String::with_capacity(1000);

        let mut tests = vec![
            "Давай".to_string().into_bytes(),
            vec![0b1111_1111; 42],
            vec![0b1111_1101; 42],
            vec![0b1010_0101; 42],
            "Давай проверим этот текст, что ли?".to_string().into_bytes(),
        ];

        for len in [3, 17, 25, 67, 129, 255, 997, 1023, 9999, 1024 * 25] {
            let mut v = Vec::with_capacity(len);
            (0..len).for_each(|_|v.push(rng.r8()));
            tests.push(v);
        }

        let mut sigs = vec![];
        let mut sig1 = S3Signature::new();
        sig1.add_s3_type(S3Type::new_id(21, 2, 1));
        sig1.add_s3_type(S3Type::new_price_float(2, 3, PricePostfixInfo::new_empty(), FracVariation::HighNum));
        sig1.add_s3_type(S3Type::new_time(TimeFormat::HMS));
        let columns: &[_] = &["prod_id", "price", "time"];
        sigs.push((sig1, columns));
        
        let mut sig2 = S3Signature::new();
        sig2.add_s3_type(S3Type::new_id(1, 1, 0));
        sig2.add_s3_type(S3Type::new_time(TimeFormat::HMS));
        let columns: &[_] = &["prod_id", "time"];
        sigs.push((sig2, columns));
        
        let mut sig3 = S3Signature::new();
        sig3.add_s3_type(S3Type::new_id(13, 2, 1));
        sig3.add_s3_type(S3Type::new_time(TimeFormat::HMSMill));
        sig3.add_s3_type(S3Type::new_time(TimeFormat::HMSMill));
        sig3.add_s3_type(S3Type::new_price_int(3, 2, PricePostfixInfo::new_0(1)));
        sig3.add_s3_type(S3Type::new_int_num(3, false));
        let columns: &[_] = &["user_id", "time_a", "time_b", "value", "amount"];
        sigs.push((sig3, columns));
        
        for test in &tests {
            for (sig, columns) in &sigs {
                let mut read = test.as_bytes();
                
                str.clear();
                let mut str_w = WriterFmt::new(str);

                let s3_dyn_ctors = S3CtorsRW::new();

                let mut csv = CsvWriter::new_std();
                columns.into_iter().zip(sig.iter_writers(&s3_dyn_ctors)).for_each(|(column, ty)|{
                    csv.add_column_str(column, ty);
                });
                
                // let mut rng = Rng1;
                let mut s3full = S3Full::new(&mut read, &mut str_w, &mut rng).unwrap();
                
                csv.write_all(&mut s3full).unwrap();
                
                let mut csv_r = CsvReader::new_std();
                sig.iter_readers(&s3_dyn_ctors).for_each(|ty|{
                    csv_r.add_column(ty);
                });
                
                let mut r = StrReadWraper::new_std(str_w.as_bytes());
                let mut w = Vec::with_capacity(1024);
                let mut s3full = S3FullReader::new(&mut r, &mut w);
                csv_r.read_all(&mut s3full).unwrap();
                assert_eq!(test, &w[..test.len()]);
                assert!(w.len() <= test.len() + 16);
                
                drop(csv_r);
                drop(s3_dyn_ctors);
                str = str_w.take_inner();
            }
        }
    }
}
