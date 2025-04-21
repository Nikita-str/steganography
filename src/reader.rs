
/// Allow to read single byte in a splitted by bits way.\
/// You can read next chunk of bits in LE order by [`Self::next_le`].\
/// To take the byte after it readed use [`Self::take_byte`].
pub struct SingleByteReader {
    cur_byte: u8,
    cur_bit: u8,
}
impl SingleByteReader {
    #[inline(always)]
    pub fn new() -> Self {
        Self {
            cur_byte: 0,
            cur_bit: 0,
        }
    }
    #[inline(always)]
    pub fn is_done(&self) -> bool {
        self.cur_bit >= 8
    }
    #[inline(always)]
    pub fn byte(&self) -> u8 {
        self.cur_byte
    }
    #[inline(always)]
    pub fn take_byte(&mut self) -> u8 {
        let ret = self.cur_byte;
        *self = Self::new();
        ret
    }
    #[inline(always)]
    pub fn next_le(&mut self, part_of_byte: u8, bits: u8) -> bool {
        self.cur_byte |= u8::wrapping_shl(part_of_byte, self.cur_bit as u32);
        self.cur_bit += bits;
        self.is_done()
    }
}

/// Allow to read bytes in a splitted way with const bit len per byte.\
/// You can read next chunk of bits in LE order by [`Self::next_le`].\
/// To take a byte after it readed use [`Self::try_take_next_le_byte`].
pub struct ConstBytesReader {
    br: SingleByteReader,
    bits: u8,
}
impl ConstBytesReader {
    pub fn new(bits: u8) -> Self {
        Self {
            br: SingleByteReader::new(),
            bits,
        }
    }
    #[inline(always)]
    pub fn is_not_started(&self) -> bool {
        self.br.cur_bit == 0
    }
    #[inline(always)]
    pub fn is_done(&self) -> bool {
        self.br.is_done()
    }
    #[inline(always)]
    pub fn next_le(&mut self, part_of_byte: u8) -> bool {
        self.br.next_le(part_of_byte, self.bits)
    }
    #[inline(always)]
    pub fn try_take_next_le_byte(&mut self, part_of_byte: u8) -> Option<u8> {
        self.next_le(part_of_byte).then(||self.br.take_byte())
    }
    #[inline(always)]
    pub fn reset(&mut self) {
        self.br = SingleByteReader::new();
    }
}

/// Reads bits(parts of bytes) until it fills the buffer. 
pub struct ConstBufReader {
    reader: ConstBytesReader,
    buf: Vec<u8>,
    expected_size: usize,
    mask: u8,
}
impl ConstBufReader {
    pub fn new(expected_size: usize, bits: u8) -> Self {
        assert!((1..=8).contains(&bits));
        let reader = ConstBytesReader::new(bits);
        let buf = Vec::with_capacity(expected_size);
        Self {
            reader,
            buf,
            expected_size,
            mask: (1u8 << bits) - 1,
        }
    }

    /// # Result
    /// is buffer full?
    pub fn read_while_can<I, F>(&mut self, chan_iter: &mut I, mut map_iter_to_bits: F) -> bool
    where
        I:  Iterator<Item = u8>,
        F: FnMut(&mut I) -> Option<u8>,
    {
        while !self.is_done() {
            let Some(part_of_byte) = map_iter_to_bits(chan_iter) else {
                return false
            };
            if let Some(byte) = self.reader.try_take_next_le_byte(part_of_byte) {
                self.buf.push(byte)
            }
        }
        true
    }

    #[inline(always)]
    pub fn left_to_read(&self) -> usize {
        self.expected_size - self.buf.len()
    }
    #[inline(always)]
    pub fn bits(&self) -> u8 {
        self.reader.bits
    }
    #[inline(always)]
    pub fn mask(&self) -> u8 {
        self.mask
    }
    #[inline(always)]
    pub fn is_done(&self) -> bool {
        self.buf.len() == self.expected_size
    }
    #[inline(always)]
    pub fn buf_ref(&self) -> &Vec<u8> {
        &self.buf
    }
    #[inline(always)]
    pub fn take_buf(self) -> Vec<u8> {
        self.buf
    }
}