
type Result<T> = std::io::Result<T>;

pub trait WriteExt {
    fn write_char(&mut self, ch: char) -> Result<()>;
    fn write_str(&mut self, s: &str) -> Result<()>;
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