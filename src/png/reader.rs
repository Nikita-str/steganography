use crate::prelude::*;

pub struct DeltaByteReader {
    cur_bit: u8,
    cur_byte: u8,
    bits_per_pixel_chan: u8,
}
impl DeltaByteReader {
    pub fn new(bits_per_pixel_chan: u8) -> Self {
        Self {
            cur_bit: 0,
            cur_byte: 0,
            bits_per_pixel_chan,
        }
    }

    #[inline]
    pub fn update_byte(&mut self, pixel_a: u8, pixel_b: u8) {
        let delta = if pixel_a >= pixel_b {
            pixel_a - pixel_b
        } else {
            pixel_b - pixel_a
        };

        self.cur_byte = (delta << self.cur_bit) | self.cur_byte;
        self.cur_bit += self.bits_per_pixel_chan;
    }
    
    #[inline]
    pub fn is_next_done(&self) -> bool {
        self.cur_bit >= 8
    }

    #[inline]
    pub fn take_next(&mut self) -> u8 {
        let ret = self.cur_byte;
        self.cur_bit = 0;
        self.cur_byte = 0;
        ret
    }
    
    #[inline]
    pub fn take_if_next_done(&mut self) -> Option<u8> {
        self.is_next_done().then(||self.take_next())
    }
}

pub struct DeltaByteMsgReader {
    reader: DeltaByteReader,
    msg_len_bytes: [u8; 4],
    index_write: usize,
    msg_size: usize,
    msg: Option<Vec<u8>>,
    ty: Option<MsgType>,
}
impl DeltaByteMsgReader {
    pub fn new(bits_per_pixel_chan: u8) -> Self {
        Self {
            reader: DeltaByteReader::new(bits_per_pixel_chan),
            msg_len_bytes: u32::to_le_bytes(0),
            index_write: 0,
            msg_size: 0,
            msg: None,
            ty: None,
        }
    }
    #[inline(always)]
    pub fn ty(&self) -> Option<MsgType> {
        self.ty
    }
    #[inline(always)]
    pub fn need_read_ty(&self) -> bool {
        self.ty.is_none()
    }

    #[inline(always)]
    pub fn need_read_len(&self) -> bool {
        self.msg.is_none()
    }

    #[inline(always)]
    pub fn need_read_msg(&self) -> bool {
        self.msg.is_some() && self.index_write < self.msg_size
    }

    #[inline(always)]
    pub fn is_finished(&self) -> bool {
        self.msg.is_some() && self.index_write >= self.msg_size
    }
    
    #[inline(always)]
    pub fn take_msg(self) -> Option<Vec<u8>> {
        self.msg
    }

    pub fn read_ty(&mut self, pixel_a: u8, pixel_b: u8) -> Result<bool> {
        self.reader.update_byte(pixel_a, pixel_b);
        if let Some(byte) = self.reader.take_if_next_done() {
            match MsgType::try_from_u8(byte) {
                Some(ty) => { self.ty = Some(ty); }
                _ => return Err(Error::InvalidMsgTypeByte(byte)),
            }
            return Ok(true)
        }
        return Ok(false)
    }

    pub fn read_len(&mut self, pixel_a: u8, pixel_b: u8) -> bool {
        self.reader.update_byte(pixel_a, pixel_b);

        if let Some(byte) = self.reader.take_if_next_done() {
            self.msg_len_bytes[self.index_write] = byte;
            self.index_write += 1;
            if self.index_write == self.msg_len_bytes.len() {
                self.msg_size = u32::from_le_bytes(self.msg_len_bytes) as usize;
                self.msg = Some(Vec::<u8>::with_capacity(self.msg_size));
                self.index_write = 0;
                return true
            }
        }
        false
    }
    
    #[allow(unused)]
    pub fn read_msg(&mut self, pixel_a: u8, pixel_b: u8) -> bool {
        self.reader.update_byte(pixel_a, pixel_b);

        if let Some(byte) = self.reader.take_if_next_done() {
            self.msg.as_mut().unwrap().push(byte);
            self.index_write += 1;
            if self.index_write >= self.msg_size { return true }
        }

        false
    }

    pub fn read(&mut self, chan_pair_iter: impl IntoIterator<Item = (u8, u8)>) -> Result<()> {
        let mut chan_pair_iter = chan_pair_iter.into_iter();

        if self.need_read_ty() {
            loop {
                let Some((pixel_a, pixel_b)) = chan_pair_iter.next() else { break };
                if self.read_ty(pixel_a, pixel_b)? { break }
            }
        } 

        if self.need_read_len() {
            loop {
                let Some((pixel_a, pixel_b)) = chan_pair_iter.next() else { break };
                if self.read_len(pixel_a, pixel_b) { break }
            }
        } 

        if self.need_read_msg() {
            let msg = self.msg.as_mut().unwrap();
            for (pixel_a, pixel_b) in chan_pair_iter {
                // just `if self.read_msg(pixel_a, pixel_b) { break }`
                // but with unwrapped `msg`

                self.reader.update_byte(pixel_a, pixel_b);

                if let Some(byte) = self.reader.take_if_next_done() {
                    msg.push(byte);
                    self.index_write += 1;
                    if self.index_write >= self.msg_size { break }
                }
            }
        }

        Ok(())
    }
}
