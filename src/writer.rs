
/// Allow to write single byte in a splitted by bits way.\
/// You can take next chunk of bits by [`Self::next`]
pub struct SingleByteWriter {
    cur_byte: u8,
    cur_bit: u8,
}
impl SingleByteWriter {
    #[inline(always)]
    pub fn new(byte: u8) -> Self {
        Self {
            cur_byte: byte,
            cur_bit: 0,
        }
    }
    #[inline(always)]
    pub fn is_done(&self) -> bool {
        self.cur_bit >= 8
    }
    #[inline(always)]
    pub fn next(&mut self, bits: u8) -> u8 {
        let mask = (1u8 << bits) - 1;
        let ret = self.cur_byte & mask;
        self.cur_byte >>= bits;
        self.cur_bit += bits;
        ret
    }
    #[inline]
    #[allow(unused)]
    pub fn try_next(&mut self, bits: u8) -> Option<u8> {
        (!self.is_done()).then(||self.next(bits))
    }
}

/// Allow to write bytes in a splitted  way with const bit len per byte.\
/// You can take next chunk of bits by [`Self::next`]
pub struct ConstBytesWriter {
    bw: SingleByteWriter,
    bits: u8,
}
impl ConstBytesWriter {
    /// # panic
    /// * if `bits` > 8
    pub fn new(first_byte: u8, bits: u8) -> Self {
        assert!(bits <= 8);
        Self {
            bw: SingleByteWriter::new(first_byte),
            bits,
        }
    }
    #[inline(always)]
    pub fn is_done(&self) -> bool {
        self.bw.is_done()
    }
    #[inline(always)]
    pub fn next(&mut self) -> u8 {
        self.bw.next(self.bits)
    }
    #[inline(always)]
    pub fn set_new_byte(&mut self, byte: u8) {
        self.bw = SingleByteWriter::new(byte);
    }
}

pub struct IterByteWriter<I> {
    bw: ConstBytesWriter,
    iter: I,
    is_done: bool,
}
impl<I: Iterator<Item = u8>> IterByteWriter<I> {
    /// # panic
    /// * if `bits` > 8
    pub fn new(mut iter: I, bits: u8) -> Self {
        let first_byte = iter.next(); 
        Self {
            bw: ConstBytesWriter::new(first_byte.unwrap_or(0), bits),
            iter,
            is_done: first_byte.is_none(),
        }
    }
    #[inline]
    pub fn is_done(&self) -> bool {
        self.is_done
    }

    pub fn write_bits<F>(&mut self, mut f_write: F) -> bool
    where F: FnMut(u8)
    {
        let next_bits = self.bw.next();
        f_write(next_bits);

        if self.bw.is_done() {
            if let Some(byte) = self.iter.next() {
                self.bw.set_new_byte(byte);
            } else {
                self.is_done = true;
                return true;
            }
        }

        false
    }

    pub fn take_iter(self) -> I {
        self.iter
    }
}
