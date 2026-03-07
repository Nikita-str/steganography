
type Result<T> = std::io::Result<T>;

pub trait WriteExt {
    fn write_char(&mut self, ch: char) -> Result<()>;
    fn write_str(&mut self, s: &str) -> Result<()>;

    /// Writes `n1` that in `0..=9`
    fn write_n1z(&mut self, n1: u8) -> std::io::Result<()> {
        debug_assert!(n1 <= 9);

        let a = n1 as u8 + b'0';
        self.write_char(a as char)?;

        Ok(())
    }

    /// Writes `n2` that in `0..=99`
    fn write_n2z(&mut self, n2: u8) -> std::io::Result<()> {
        debug_assert!(n2 <= 99);

        let a = (n2 / 10) as u8 + b'0';
        let b = (n2 % 10) as u8 + b'0';
        self.write_char(a as char)?;
        self.write_char(b as char)?;

        Ok(())
    }

    /// Writes [z]eroed `n3` that in `0..=999`
    /// 
    /// Examples:
    /// * `n3 = 157` --> `"157"`
    /// * `n3 = 17` --> `"017"`
    /// * `n3 = 3` --> `"003"`
    /// * `n3 = 0` --> `"000"`
    fn write_n3z(&mut self, n3: u16) -> std::io::Result<()> {
        debug_assert!(n3 <= 999);

        let a = ((n3 / 10) / 10) as u8 + b'0';
        let b = ((n3 / 10) % 10) as u8 + b'0';
        let c = (n3 % 10) as u8 + b'0';
        self.write_char(a as char)?;
        self.write_char(b as char)?;
        self.write_char(c as char)?;

        Ok(())
    }

    /// Writes `n3` that in `0..=999`
    /// 
    /// Examples:
    /// * `n3 = 157` --> `"157"`
    /// * `n3 = 17` --> `"17"`
    /// * `n3 = 3` --> `"3"`
    /// * `n3 = 0` --> `"0"`
    fn write_n3(&mut self, n3: u16) -> std::io::Result<()> {
        self.write_n3e(n3)?;
        if n3 == 0 {
            self.write_char('0')?;
        }
        Ok(())
    }
    
    /// Writes `n3` allowed [e]mpty that in `0..=999`
    /// 
    /// Examples:
    /// * `n3 = 157` --> `"157"`
    /// * `n3 = 17` --> `"17"`
    /// * `n3 = 3` --> `"3"`
    /// * `n3 = 0` --> `""`
    fn write_n3e(&mut self, n3: u16) -> std::io::Result<()> {
        debug_assert!(n3 <= 999);

        let a = ((n3 / 10) / 10) as u8;
        let b = ((n3 / 10) % 10) as u8;
        let c = (n3 % 10) as u8;

        if a != 0 {
            self.write_char((a + b'0') as char)?;
        }
        if a != 0 || b != 0 {
            self.write_char((b + b'0') as char)?;
        }
        if a != 0 || b != 0 || c != 0 {
            self.write_char((c + b'0') as char)?;
        }

        Ok(())
    }
}

impl<W: std::io::Write> WriteExt for W {
    fn write_char(&mut self, ch: char) -> Result<()> {
        let len = ch.len_utf8();
        let mut bytes = [0u8; 4];
        ch.encode_utf8(&mut bytes);
        self.write_all(&bytes[..len])
    }

    fn write_str(&mut self, s: &str) -> Result<()> {
        for ch in s.chars() {
            self.write_char(ch)?;
        }
        Ok(())
    }
}

pub struct WriterFmt<W: std::fmt::Write> {
    w: W,
}
impl<W: std::fmt::Write> WriterFmt<W> {
    pub fn new(w: W) -> Self {
        Self { w }
    }

    pub fn take_inner(self) -> W {
        self.w
    }
}

impl<W: std::fmt::Write> AsRef<W> for WriterFmt<W> {
    fn as_ref(&self) -> &W {
        &self.w
    }
}

impl<W: std::fmt::Write> AsMut<W> for WriterFmt<W> {
    fn as_mut(&mut self) -> &mut W {
        &mut self.w
    }
}

impl<W: std::fmt::Write> std::ops::Deref for WriterFmt<W> {
    type Target = W;
    fn deref(&self) -> &Self::Target {
        &self.w
    }
}

impl<W: std::fmt::Write> std::ops::DerefMut for WriterFmt<W> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.w
    }
}

impl<W: std::fmt::Write> WriteExt for WriterFmt<W> {
    fn write_char(&mut self, ch: char) -> Result<()> {
        self.w.write_char(ch).map_err(|x|std::io::Error::other(x))
    }

    fn write_str(&mut self, s: &str) -> Result<()> {
        self.w.write_str(s).map_err(|x|std::io::Error::other(x))
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Write;
    use super::*;

    #[test]
    fn test_write_char() {
        let mut str = String::new();
        let mut vec = Vec::<u8>::new();

        let test_chars = "x3+Ab~`ыЙяёøઌ🌚🌚🌝🥰😎"; 

        for ch in test_chars.chars() {
            str.write_char(ch).unwrap();
            vec.write_char(ch).unwrap();
        }

        assert_eq!(test_chars, str);
        assert_eq!(str.as_bytes(), &vec);
        
        vec.clear();
        assert!(vec.is_empty());

        vec.write_str(test_chars).unwrap();
        assert_eq!(str.as_bytes(), &vec);
        assert!(!vec.is_empty());
    }
}