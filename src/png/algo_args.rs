use crate::prelude::*;
use crate::png::prelude::*;
use crate::png::writer::TopBottomChunks;
use crate::reader::ConstBytesReader;

pub struct DeltaHideArgs {
    pub msg: Vec<u8>,

    pub initial_img: Vec<String>,
    pub modified_img: ImgPaths,
    
    /// Bits per pixel channel. (Preferably 1 or 2). 
    /// Not allowed to be more than 4.
    pub bits: u8,
    pub ty: MsgType,
}

impl DeltaHideArgs {
    pub fn hide(self) -> Result<()> {
        let msg_len = self.msg.len();
        let msg_iter = self.msg.into_iter();
        let mut msg_writer = DeltaByteMsgWriter::new(msg_len, msg_iter, self.bits, self.ty)?;
        
        // TODO: it can be paralleled (by images & by chunks of pixels in an image)
        for (index, path) in self.initial_img.iter().enumerate() {
            let mut img = Img::open_img(path);

            let chan_iter = img.img.pixels_mut().flat_map(|x|&mut x.0);
            msg_writer.write(chan_iter);

            img.save_img(&self.modified_img, index)?;
            if msg_writer.is_done() { break }
        }
        if !msg_writer.is_done() {
            return Err(Error::NotEnoughSizeOfInit(msg_writer.bytes_left()));
        }
        Ok(())
    }
}

pub struct AvgSumHideArgs {
    pub msg: Vec<u8>,
    pub ty: MsgType,

    pub initial_img: Vec<String>,
    pub modified_img: ImgPaths,
    
    /// Bits per chunk.
    /// Max value is 6.
    pub bits_per_chunk: u8,

    /// Size of a chunk.
    /// Max value is 64.
    pub chunk_size: u8,
}
impl AvgSumHideArgs {
    const SUM_HEADER_CHUNK_SZ: u8 = 8;
    const SUM_HEADER_BITS_PER_CHUNK: u8 = 4;

    pub fn header_writer(&self) -> Result<AvgSumHideBlockWriter<impl Iterator<Item = u8> + 'static>> {
        let msg_len = self.msg.len();
        Error::test_too_big_msg(msg_len)?;
        let mut header = vec![self.ty as u8, self.bits_per_chunk, self.chunk_size];
        header.extend((msg_len as u32).to_le_bytes());

        Ok(AvgSumHideBlockWriter::new(
            header, 
            Self::SUM_HEADER_BITS_PER_CHUNK, 
            Self::SUM_HEADER_CHUNK_SZ,
        ))
    }

    pub fn hide(self) -> Result<()> {
        let mut header_writer = self.header_writer()?;
        let mut msg_writer = AvgSumHideBlockWriter::new(self.msg, self.bits_per_chunk, self.chunk_size);

        let mut chunk_buf_top: Vec<&mut u8> = Vec::with_capacity(MAX_WIN_SZ as usize);
        let mut chunk_buf_bottom: Vec<&mut u8> = Vec::with_capacity(MAX_WIN_SZ as usize);

        'init: for (index, path) in self.initial_img.iter().enumerate() {
            let mut img = Img::open_img(path);

            let mut chan_iter = img.img.pixels_mut().flat_map(|x|&mut x.0);
 
            // here we change lifetime of references in chunk
            // [Safety]:
            //     Chunk is always clear; 
            //     On each iteration we address only ref of current iteration.
            //     No one ref pointed to the same point
            let mut chunks = TopBottomChunks {
                chunk_top: &mut chunk_buf_top,
                chunk_bottom: &mut chunk_buf_bottom,
            };
            let chunk: &mut TopBottomChunks = unsafe { std::mem::transmute(&mut chunks) };

            // let mut chunk_buf_top: Vec<&mut u8> = Vec::with_capacity(MAX_WIN_SZ as usize);
            // let mut chunk_buf_bottom: Vec<&mut u8> = Vec::with_capacity(MAX_WIN_SZ as usize);

            if !header_writer.is_done() {
                chunk.clear();
                loop {
                    let flags = header_writer.write_bits(chunk, &mut chan_iter);
                    if flags.continue_init {
                        chunk.clear();
                        img.save_img(&self.modified_img, index)?;
                        continue 'init
                    }
                    if flags.is_done { break }
                }
            }

            if !msg_writer.is_done() {
                chunk.clear();
                loop {
                    let flags = msg_writer.write_bits(chunk, &mut chan_iter);
                    if flags.continue_init {
                        chunk.clear();
                        img.save_img(&self.modified_img, index)?;
                        continue 'init
                    }
                    if flags.is_done { break }
                }
            }

            chunk.clear();
            img.save_img(&self.modified_img, index)?;
            if msg_writer.is_done() { break }
        }

        if !msg_writer.is_done() {
            return Err(Error::NotEnoughSizeOfInit(msg_writer.bytes_left()));
        }
        Ok(())
    }
}


pub struct DeltaRevealArgs {
    pub initial_img: Vec<String>,
    pub modified_img: Vec<String>,
    pub save_path: Option<String>,
    pub bits: u8,
}
impl DeltaRevealArgs {
    pub fn reveal(&self) -> Result<(Vec<u8>, MsgType)> {
        let mut msg_reader = DeltaByteMsgReader::new(self.bits);
        
        // TODO: it can be paralleled (by images & by chunks of pixels in an image)
        for (path_a, path_b) in self.initial_img.iter().zip(self.modified_img.iter()) {
            let img_a = Img::open_img(path_a);
            let img_b = Img::open_img(path_b);
            if img_a.width() != img_b.width() || img_a.height() != img_b.height() {
                return Err(Error::ImageInconsistentSize(
                    img_a.width(),
                    img_a.height(),
                    img_b.width(), 
                    img_b.height()
                ))
            }

            let chan_iter_a = img_a.img.pixels().flat_map(|x|&x.0).cloned();
            let chan_iter_b = img_b.img.pixels().flat_map(|x|&x.0).cloned();
            let chan_pair_iter = chan_iter_a.zip(chan_iter_b);
            msg_reader.read(chan_pair_iter)?;

            if msg_reader.is_finished() { break }
        }

        let Some(ty) = msg_reader.ty() else {
            return Err(Error::UnreadedHeader);
        };
        let Some(msg) = msg_reader.take_msg() else {
            return Err(Error::UnreadedHeader);
        };
        Ok((msg, ty))
    }
}

pub struct AvgSumRevealArgs {
    pub modified_img: Vec<String>,
    pub save_path: Option<String>,
}
impl AvgSumRevealArgs {
    pub fn reveal(&self) -> Result<(Vec<u8>, MsgType)> {
        const HEADER_SIZE: usize = 7;
        const HEADER_CHUNK_SIZE: u8 = 8;
        const HEADER_BITS_PER_CHUNK: u8 = 4;

        let mut chunk_size = HEADER_CHUNK_SIZE;
        let mut bits_per_chunk = HEADER_BITS_PER_CHUNK;
        let mut rem = 1u16 << bits_per_chunk;
        let mut sum;
        let mut header_reader = ConstBytesReader::new(bits_per_chunk);
        let mut header = Vec::with_capacity(HEADER_SIZE);
        let mut ty = MsgType::Reserved;
        let mut msg_len = 0;
        
        let mut msg_reader: Option<(ConstBytesReader, Vec<u8>)> = None;

        // TODO: it can be paralleled (by images & by chunks of pixels in an image)
        'modi: for path in &self.modified_img {
            let img = Img::open_img(path);
            let mut chan_iter = img.img.pixels().flat_map(|x|&x.0).cloned();

            while header.len() != HEADER_SIZE {
                sum = 0;
                for _ in 0..chunk_size {
                    if let Some(byte) = chan_iter.next() {
                        sum += byte as u16;
                    } else {
                        continue 'modi
                    }
                }

                let part_of_byte = sum % rem;
                if let Some(byte) = header_reader.try_take_next_le_byte(part_of_byte as u8) {
                    header.push(byte)
                }
            }

            if header.len() == HEADER_SIZE {
                match MsgType::try_from_u8(header[0]) {
                    Some(ty_x) => ty = ty_x,
                    _ => return Err(Error::InvalidMsgTypeByte(header[0])),
                };
                bits_per_chunk = header[1];
                rem = 1u16 << bits_per_chunk;
                chunk_size = header[2];
                let len_bytes: [u8; 4] = header[HEADER_SIZE - 4..HEADER_SIZE].try_into().unwrap();
                msg_len = u32::from_le_bytes(len_bytes) as usize;
                
                let msg = Vec::with_capacity(msg_len);
                msg_reader = Some((ConstBytesReader::new(bits_per_chunk), msg));
            }
            
            if let Some((msg_reader, msg)) = &mut msg_reader {
                while msg.len() != msg_len {
                    sum = 0;
                    for _ in 0..chunk_size {
                        if let Some(byte) = chan_iter.next() {
                            sum += byte as u16;
                        } else {
                            continue 'modi
                        }
                    }

                    let part_of_byte = sum % rem;
                    if let Some(byte) = msg_reader.try_take_next_le_byte(part_of_byte as u8) {
                        msg.push(byte)
                    }
                }

                break 'modi
            }
        }

        if ty.is_reserved() {
            return Err(Error::UnreadedHeader);
        }
        let Some((_, msg)) = msg_reader else {
            return Err(Error::UnreadedHeader);
        };
        Ok((msg, ty))
    }
}