use crate::writer::{ConstBytesWriter, IterByteWriter};
use crate::prelude::*;
use crate::PSEUDO_RAND_INDEXES;

pub struct DeltaBytesWriter {
    bw: ConstBytesWriter,
}
impl DeltaBytesWriter {
    pub fn new(first_byte: u8, bits: u8) -> Self {
        Self {
            bw: ConstBytesWriter::new(first_byte, bits),
        }
    }
    #[inline]
    pub fn update_byte(&mut self, byte: &mut u8) {
        let delta = self.bw.next();

        if *byte < HALF {
            *byte += delta;
        } else {
            *byte -= delta;
        }
    }
    #[inline]
    pub fn need_next(&self) -> bool {
        self.bw.is_done()
    }
    #[inline]
    pub fn set_new_byte(&mut self, byte: u8) {
        self.bw.set_new_byte(byte);
    }
}

pub struct DeltaByteMsgWriter<Iter: Iterator<Item = u8>> {
    writer: DeltaBytesWriter,
    header: Vec<u8>,
    msg_iter: Iter,

    len_written: usize,
    is_done: bool,
}
impl<Iter: Iterator<Item = u8>> DeltaByteMsgWriter<Iter> {
    pub fn new(msg_len: usize, msg_iter: Iter, bits_per_pixel_chan: u8, ty: MsgType) -> Result<Self> {
        Error::test_too_big_msg(msg_len)?;
        
        let mut header = Vec::with_capacity(5);
        header.push(ty as u8);
        let msg_len_bytes = u32::to_le_bytes(msg_len as u32);
        header.extend(msg_len_bytes);
        let writer = DeltaBytesWriter::new(header[0], bits_per_pixel_chan);

        Ok(Self{
            writer,
            header,
            msg_iter,
            len_written: 0,
            is_done: false,
        })
    }

    #[inline(always)]
    pub fn bytes_left(self) -> usize {
        self.msg_iter.count()
    }

    #[inline(always)]
    pub fn is_done(&self) -> bool {
        self.is_done
    }

    #[inline(always)]
    pub fn need_write_len(&self) -> bool {
        self.len_written < self.header.len()
    }

    #[inline(always)]
    pub fn need_write_msg(&self) -> bool {
        !self.need_write_len() && !self.is_done
    }
    
    /// # Return
    /// * bool = ControlFlow::Break
    #[inline]
    fn set_next_byte_from_iter(&mut self) -> bool {
        if let Some(byte) = self.msg_iter.next() {
            self.writer.set_new_byte(byte);
        } else {
            self.is_done = true;
            return true
        }
        false
    }

    /// # Return
    /// * bool = ControlFlow::Break
    pub fn write_len(&mut self, chan_byte: &mut u8) -> bool {
        self.writer.update_byte(chan_byte);
        if self.writer.need_next() {
            self.len_written += 1;
            if self.need_write_len() {
                let byte = self.header[self.len_written];
                self.writer.set_new_byte(byte);
            } else {
                self.set_next_byte_from_iter();
                return true;
            }
        }
        false
    }    
    
    /// # Return
    /// * bool = ControlFlow::Break
    pub fn write_msg(&mut self, chan_byte: &mut u8) -> bool {
        self.writer.update_byte(chan_byte);
        if self.writer.need_next() { 
            return self.set_next_byte_from_iter()
        }
        false
    }    
    
    pub fn write<'a>(&mut self, chan_iter: impl IntoIterator<Item = &'a mut u8>) {
        let mut chan_iter = chan_iter.into_iter();

        // write len of msg
        if self.need_write_len() {
            loop {
                let Some(chan_byte) = chan_iter.next() else { break };
                if self.write_len(chan_byte) { break }
            }
        }

        // write msg itself
        if self.need_write_msg() {
            for chan_byte in chan_iter {
                if self.write_msg(chan_byte) { break }
            }
        }
    }
}


#[derive(Default)]
pub struct AvgSumHideWriterFlags {
    pub continue_init: bool,
    pub is_done: bool,
}

pub struct TopBottomChunks<'a, 'b> {
    pub chunk_top: &'b mut Vec<&'a mut u8>,
    pub chunk_bottom: &'b mut Vec<&'a mut u8>,
}
impl<'a, 'b> TopBottomChunks<'a, 'b> {
    pub fn clear(&mut self) {
        self.chunk_top.clear();
        self.chunk_bottom.clear();
    }
    pub fn push(&mut self, x: &'a mut u8) {
        if *x > HALF {
            self.chunk_top.push(x)
        } else {
            self.chunk_bottom.push(x)
        }
    }
    pub fn len(&self, is_top: bool) -> usize {
        if is_top {
            self.chunk_top.len()
        } else {
            self.chunk_bottom.len()
        }
    }
    pub fn swap_remove(&mut self, index: usize, is_top: bool) -> &'a mut u8 {
        if is_top {
            self.chunk_top.swap_remove(index)
        } else {
            self.chunk_bottom.swap_remove(index)
        }
    }
    pub fn is_top(&self) -> bool {
        self.chunk_top.len() >= self.chunk_bottom.len()
    }
}


pub struct AvgSumHideBlockWriter<I> {
    iter_bw: IterByteWriter<I>,
    sum: u16,
    rem: u8,
    bits_per_chunk: u8,
    chunk_size: u8,
    max_chan_delta: u8,
    
    // TODO: strategy (cur strategy is pseudo random & filling small firstly)
    pseudo_rand_index: u8,
}
impl<I: Iterator<Item = u8>> AvgSumHideBlockWriter<I> {
    pub fn new<II: IntoIterator<IntoIter = I>>(into_iter: II, bits_per_chunk: u8, chunk_size: u8) -> Self {
        Self {
            iter_bw: IterByteWriter::new(into_iter.into_iter(), bits_per_chunk),
            sum: 0,
            rem: 1 << bits_per_chunk,
            bits_per_chunk,
            chunk_size,
            max_chan_delta: ((1 << bits_per_chunk) - 1) / (chunk_size >> 1) + 1,
            pseudo_rand_index: 0,
        }
    }

    #[inline(always)]
    pub fn is_done(&self) -> bool {
        self.iter_bw.is_done()
    }
    
    #[inline(always)]
    pub fn bytes_left(self) -> usize {
        self.iter_bw.take_iter().count()
    }

    pub fn write_bits<'a, ChanI>(&mut self, chunk: &mut TopBottomChunks<'a, '_>, mut chan_iter: ChanI) -> AvgSumHideWriterFlags
    where ChanI: Iterator<Item = &'a mut u8>
    {
        let mut flags = AvgSumHideWriterFlags::default();

        flags.is_done = self.iter_bw.write_bits(|part_of_byte|{
            chunk.clear();
            self.sum = 0;

            // fill the chunk (or break)
            for _ in 0..self.chunk_size {
                if let Some(x) = chan_iter.next() {
                    self.sum += *x as u16;
                    chunk.push(x);
                } else {
                    flags.continue_init = true;
                    return;
                }
            }
            
            // TODO: strategy (cur strategy is pseudo random & filling small firstly)
            let is_top = chunk.is_top();
            let sum_rem = self.sum % (self.rem as u16);
            let part_of_byte = part_of_byte as u16;
            let mut need_write = (self.rem as u16 + part_of_byte - sum_rem) % self.rem as u16;
            if is_top && need_write != 0 { need_write = (1 << self.bits_per_chunk) - need_write; }

            while need_write != 0 {
                // calc min value that can have a pixel_chan in the rest of chunk
                // let can_write_min = need_write.saturating_sub(((chunk.len(is_top) - 1) * self.max_chan_delta as usize) as u16);
                
                // calc max value that can have a pixel_chan in the rest of chunk
                let can_write_min = (self.max_chan_delta as u16).min(need_write);
                need_write -= can_write_min;
                
                // update chunk value
                let index = PSEUDO_RAND_INDEXES[self.pseudo_rand_index as usize] % chunk.len(is_top);
                let value = chunk.swap_remove(index, is_top);
                if is_top {
                    // `-=` because value more than HALF
                    *value -= can_write_min as u8;
                } else {
                    // `+=` because value less than HALF
                    *value += can_write_min as u8;
                }
                
                self.pseudo_rand_index = self.pseudo_rand_index.wrapping_add(1);
            }
        });
        chunk.clear();
        flags
    }
}