
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