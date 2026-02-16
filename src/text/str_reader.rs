
pub type Result<T> = std::io::Result<T>;

pub trait PeakableReadExt {
    fn is_eof(&mut self) -> Result<bool>;
    fn read_byte(&mut self) -> Result<u8>;
    fn try_read_byte(&mut self) -> Result<Option<u8>>;

    fn peak_byte(&mut self, n: u8) -> Result<Option<u8>>;
    fn peak_next_byte(&mut self) -> Result<Option<u8>> {
        self.peak_byte(0)
    }

    fn test_byte(&mut self, n: u8, expect: u8) -> Result<bool> {
        self.peak_byte(n).map(|x|x == Some(expect))
    }
    fn test_next_bytes(&mut self, expects: &[u8]) -> Result<bool> {
        for (n, x) in expects.iter().enumerate() {
            let peak = self.peak_byte(n as u8)?;
            if peak != Some(*x) {
                return Ok(false)
            }
        }
        Ok(true)
    }
    fn test_next_char(&mut self, expect_char: char) -> Result<bool> {
        let len = expect_char.len_utf8();
        if len == 1 {
            Ok(self.peak_next_byte()? == Some(expect_char as u8))
        } else {
            let mut bytes = [0; 4];
            expect_char.encode_utf8(&mut bytes);
            self.test_next_bytes(&bytes[0..len])
        }
    }

    fn try_read_char(&mut self) -> std::io::Result<Option<char>> {
        if self.is_eof()? {
            Ok(None)
        } else {
            Ok(Some(self.read_char()?))
        }
    }

    fn read_char(&mut self) -> std::io::Result<char> {
        let b1 = self.read_byte()?;

        #[inline(always)]
        fn convert(bytes: &[u8]) -> std::io::Result<char> {
            let s = str::from_utf8(bytes).map_err(|_|std::io::Error::other("Invalid UTF-8 char seq"))?;
            s.chars().next().ok_or_else(||std::io::Error::other("Empty string?!")) 
        }

        match b1.leading_ones() {
            0 => Ok(b1 as char),
            2 => {
                let b2 = self.read_byte()?;
                convert(&[b1, b2])
            }
            3 => {
                let b2 = self.read_byte()?;
                let b3 = self.read_byte()?;
                convert(&[b1, b2, b3])
            }
            4 => {
                let b2 = self.read_byte()?;
                let b3 = self.read_byte()?;
                let b4 = self.read_byte()?;
                convert(&[b1, b2, b3, b4])
            }
            _ => Err(std::io::Error::other("Invalid UTF-8 char seq")),
        }
    }
    
    fn read_str(&mut self, str: &mut String, char_len: u8) -> std::io::Result<()> {
        for _ in 0..char_len {
            str.push(self.read_char()?);
        }
        Ok(())
    }
    
    fn read_str_until_char(&mut self, str: &mut String, unitl_char: char) -> std::io::Result<()> {
        loop {
            if self.test_next_char(unitl_char)? {
                break
            }
            if self.is_eof()? {
                break 
            }
            str.push(self.read_char()?);
        }
        Ok(())
    }
}

pub struct ReadWraper<R> {
    r: R,
    buf: Vec<u8>,
    i_start: usize,
    i_end: usize,
    is_buf_empty: bool,
    is_r_eof: bool,
}
impl<R> ReadWraper<R> {
    const KB: usize = 1024;
    const BUF_SZ: usize = Self::KB * 4;

    pub fn new_std(r: R) -> Self {
        Self::with_capacity(r, Self::BUF_SZ)
    }

    pub fn with_capacity(r: R, capacity: usize) -> Self {
        Self {
            r,
            buf: vec![0; capacity],
            i_start: 0,
            i_end: 0,
            is_buf_empty: true, 
            is_r_eof: false,
        }
    }

    #[inline(always)]
    fn is_eof(&mut self) -> bool {
        self.is_buf_empty && self.is_r_eof
    }

    #[inline(always)]
    fn buf_capacity(&self) -> usize {
        self.buf.len()
    }

    #[inline(always)]
    fn is_buf_empty(&self) -> bool {
        self.is_buf_empty
    }

    fn buf_len(&self) -> usize {
        if self.is_buf_empty() {
            0
        } else if self.i_end <= self.i_start {
            let sep_point = self.buf_capacity();
            self.i_end + sep_point - self.i_start
        } else {
            self.i_end - self.i_start
        }
    }
}

impl<R: std::io::Read> ReadWraper<R> {
    fn make_continuous(&mut self) {
        if self.i_start == 0 {
            return
        }
        todo!("rn `make_continuous` is not implimented, sorry! (You need it only to peak more than capactiy... do you really need it?)")
    }

    pub fn peak(&mut self, n: usize) -> Result<Option<u8>> {
        let n = n + 1;
        if self.buf_len() < n {
            self.fill_buf()?;
        }
        if !self.is_r_eof && self.buf_len() < n {
            self.make_continuous();
            self.buf.reserve(n - self.buf_len());

            // SAFETY: we controls our bounds by `i_start` & `i_end` 
            //         so this action only allows to fill the buf
            //         but does not allow to read something that was not recieved from reader(`self.r`)
            unsafe { self.buf.set_len(n); }

            self.fill_buf()?;
        }

        if self.buf_len() < n {
            return Ok(None)
        }
        let n = n - 1;

        let i = self.i_start + n;
        if i < self.buf_capacity() {
            Ok(Some(self.buf[i]))
        } else {
            let delta = self.buf_capacity() - self.i_start;
            Ok(Some(self.buf[n - delta]))
        }
    }

    fn fill_buf(&mut self) -> Result<()> {
        if self.is_r_eof {
            return Ok(())
        }

        let capacity = self.buf_capacity();
        let mut need_read = capacity - self.buf_len();
        let mut readed_once = false;
        
        let mut from;
        let mut to;
        let mut need_change_from_to;
        if self.i_end == capacity {
            from = 0;
            to = need_read;
            need_change_from_to = false;
        } else if self.i_start <= self.i_end {
            from = self.i_end;
            to = capacity;
            need_change_from_to = self.i_start != 0;
        } else {
            from = self.i_end;
            to = self.i_start;
            need_change_from_to = false;
        }

        loop {
            if need_read == 0 {
                break
            }

            let readed = self.r.read(&mut self.buf[from..to])?;
            let is_eof = readed == 0;
            if is_eof {
                self.is_r_eof = true;
                break
            }
            readed_once |= !is_eof;
            need_read -= readed;

            from += readed;
            self.i_end += readed;
            if from == to {
                self.is_buf_empty = false;
                if need_change_from_to {
                    need_change_from_to = false;
                    from = 0;
                    to = need_read;
                }
            }
        }

        if self.i_end != capacity {
            self.i_end %= capacity;
        }
        self.is_buf_empty &= !readed_once;

        Ok(())
    }

    fn upd_bounds(&mut self) {
        let is_empty = self.i_start == self.i_end;
        if is_empty {
            self.is_buf_empty = true;
            self.i_start = 0;
            self.i_end = 0;
        }
        if self.i_start == self.buf_capacity() {
            self.i_start = 0;
        }
    }

    fn read_byte_unchecked(&mut self) -> u8 {
        let x = self.buf[self.i_start];

        self.i_start += 1;
        self.upd_bounds();

        x
    }
}

impl<R: std::io::Read> PeakableReadExt for ReadWraper<R> {
    fn is_eof(&mut self) -> Result<bool> {
        Ok(self.is_eof())
    }

    fn read_byte(&mut self) -> Result<u8> {
        debug_assert!(!self.is_eof());
        if self.is_buf_empty() {
            self.fill_buf()?;
        }

        Ok(self.read_byte_unchecked())
    }

    fn try_read_byte(&mut self) -> Result<Option<u8>> {
        if self.is_buf_empty() {
            self.fill_buf()?;
        }
        if self.is_eof() {
            return Ok(None)
        }
        Ok(Some(self.read_byte_unchecked()))
    }

    fn peak_byte(&mut self, n: u8) -> Result<Option<u8>> {
        self.peak(n as usize)
    }
}


impl<R: std::io::Read> std::io::Read for ReadWraper<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut readed = 0;

        loop {
            if buf.len() == readed {
                break
            }

            if self.is_buf_empty() {
                self.fill_buf()?;
            }
            if self.is_eof() {
                break
            }
            
            let from = self.i_start;
            let len;
            if self.i_start < self.i_end {
                len = (buf.len() - readed).min(self.i_end - self.i_start);
            } else {
                len = (buf.len() - readed).min(self.buf_capacity() - self.i_start);
            }

            let sub_buf = &mut buf[readed..readed + len];
            sub_buf.copy_from_slice(&self.buf[from..from + len]);

            debug_assert!(len != 0);
            self.i_start += len;
            readed += len;
            
            if readed > 20 { panic!() }
            self.upd_bounds();
        }
            
        Ok(readed)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;
    use super::*;

    #[test]
    fn test_read() {
        let v = vec![11u8, 21u8, 31, 41, 51, 61, 71, 81, 91, 12, 13];
        let mut r = ReadWraper::with_capacity(v.as_slice(), 4);
        assert!(!r.is_eof());
        assert_eq!(r.peak_byte(1).unwrap(), Some(21));
        assert_eq!(r.peak_byte(0).unwrap(), Some(11));
        assert_eq!(r.read_byte().unwrap(), 11);
        assert_eq!(r.read_byte().unwrap(), 21);
        assert_eq!(r.peak_byte(3).unwrap(), Some(61));
        assert_eq!(r.peak_byte(0).unwrap(), Some(31));
        assert_eq!(r.peak_byte(1).unwrap(), Some(41));
        assert_eq!(r.peak_byte(2).unwrap(), Some(51));
        assert_eq!(r.read_byte().unwrap(), 31);
        assert_eq!(r.read_byte().unwrap(), 41);
        assert_eq!(r.peak_byte(3).unwrap(), Some(81));
        assert_eq!(r.peak_byte(0).unwrap(), Some(51));
        assert_eq!(r.read_byte().unwrap(), 51);
        assert_eq!(r.read_byte().unwrap(), 61);
        assert_eq!(r.peak_byte(3).unwrap(), Some(12));
        assert_eq!(r.read_byte().unwrap(), 71);
        assert_eq!(r.try_read_byte().unwrap(), Some(81));
        assert_eq!(r.read_byte().unwrap(), 91);
        assert_eq!(r.peak_byte(3).unwrap(), None);
        assert!(!r.is_eof());
        assert_eq!(r.peak_byte(0).unwrap(), Some(12));
        assert_eq!(r.peak_byte(1).unwrap(), Some(13));
        assert_eq!(r.read_byte().unwrap(), 12);
        assert!(!r.is_eof());
        assert_eq!(r.try_read_byte().unwrap(), Some(13));
        assert!(r.is_eof());
        assert_eq!(r.try_read_byte().unwrap(), None);
        
        for i in 0..7 {
            let mut r = ReadWraper::with_capacity(v.as_slice(), 4);
            let mut vv = vec![];
            for _ in 0..=i {
                vv.push(r.read_byte().unwrap());
            }
            r.read_to_end(&mut vv).unwrap();
            assert_eq!(vv.as_slice(), v.as_slice());
        }
          
        for i in 0..7 {
            let mut r = ReadWraper::with_capacity(v.as_slice(), 4);
            let mut vv = vec![];
            for _ in 0..=i {
                vv.push(r.read_byte().unwrap());
                let _ = r.peak_byte(3).unwrap();
            }
            r.read_to_end(&mut vv).unwrap();
            assert_eq!(vv.as_slice(), v.as_slice());
        }

        //TODO: rand test (for both: std::io::Read on Wrap & for PeakableReadExt)
    }
}
