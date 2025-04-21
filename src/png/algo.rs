use image::{Rgb, RgbImage};
use rand::RngCore;

use crate::prelude::*;
use crate::PSEUDO_RAND_INDEXES;

use crate::writer::{IterByteWriter, HiderWriter};
use crate::reader::{ConstBufReader, ConstBytesReader};

use crate::png::reader::*;
use crate::png::writer::*;
use crate::png::prelude::*;
use crate::png::writer::TopBottomChunks;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// [+] General help objects

macro_rules! paths_pair {
    ($x: ident) => {
        PathsPair { 
            initial_img: &$x.initial_img, 
            modified_img: &$x.modified_img
        }
    };
}
struct PathsPair<'a> {
    initial_img: &'a Vec<String>,
    modified_img: &'a ImgPaths,
}

fn hider_loop<'a, H, F>(writer: &mut H, paths: PathsPair<'a>, mut img_f: F) -> Result<()>
where
    H: HiderWriter,
    F: FnMut(&mut H, &mut RgbImage) -> Result<()>
{
    for (index, path) in paths.initial_img.iter().enumerate() {
        let mut img = Img::open_img(path);
        img_f(writer, &mut img.img)?;
        img.save_img(paths.modified_img, index)?;
        if writer.is_done() { break }
    }

    if !writer.is_done() {
        return Err(Error::NotEnoughSizeOfInit(writer.bytes_left()));
    }
    Ok(())
}
// [-] General help objects
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// [+] Hider(s)

// ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━
// [=][+] Hider: Delta

pub struct DeltaHider {
    pub msg: Vec<u8>,

    pub initial_img: Vec<String>,
    pub modified_img: ImgPaths,
    
    /// Bits per pixel channel. (Preferably 1 or 2). 
    /// Not allowed to be more than 4.
    pub bits: u8,
    pub ty: MsgType,
}

impl DeltaHider {
    pub fn hide(self) -> Result<()> {
        let paths = paths_pair!(self);
        let msg_len = self.msg.len();
        let msg_iter = self.msg.into_iter();
        let mut msg_writer = DeltaByteMsgWriter::new(msg_len, msg_iter, self.bits, self.ty)?;
        
        // TODO: it can be paralleled (by images & by chunks of pixels in an image)
        hider_loop(&mut msg_writer, paths, |msg_writer, img| {
            let chan_iter = img.pixels_mut().flat_map(|x|&mut x.0);
            msg_writer.write(chan_iter);
            Ok(())
        })
    }
}

// [=][-] Hider: Delta
// ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━
// ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━
// [=][+] Hider: Avg Sum

pub struct AvgSumHider {
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

impl AvgSumHider {
    const HEADER_CHUNK_SIZE: u8 = 8;
    const HEADER_BITS_PER_CHUNK: u8 = 4;

    pub fn header_writer(&self) -> Result<AvgSumHideBlockWriter<impl Iterator<Item = u8> + 'static>> {
        let msg_len = self.msg.len();
        Error::test_too_big_msg(msg_len)?;
        let mut header = vec![self.ty as u8, self.bits_per_chunk, self.chunk_size];
        header.extend((msg_len as u32).to_le_bytes());

        Ok(AvgSumHideBlockWriter::new(
            header, 
            Self::HEADER_BITS_PER_CHUNK, 
            Self::HEADER_CHUNK_SIZE,
        ))
    }

    pub fn hide(self) -> Result<()> {
        let paths = paths_pair!(self);
        let mut header_writer = self.header_writer()?;
        let mut msg_writer = AvgSumHideBlockWriter::new(self.msg, self.bits_per_chunk, self.chunk_size);

        hider_loop(&mut msg_writer, paths, |msg_writer, img|{
            let mut chan_iter = img.pixels_mut().flat_map(|x|&mut x.0);
 
            let mut chunk_buf_top: Vec<&mut u8> = Vec::with_capacity(MAX_WIN_SZ as usize);
            let mut chunk_buf_bottom: Vec<&mut u8> = Vec::with_capacity(MAX_WIN_SZ as usize);
            
            let chunks = &mut TopBottomChunks {
                chunk_top: &mut chunk_buf_top,
                chunk_bottom: &mut chunk_buf_bottom,
            };

            if !header_writer.is_done() {
                loop {
                    let flags = header_writer.write_bits(chunks, &mut chan_iter);
                    if flags.continue_init { return Ok(()) }
                    if flags.is_done { break }
                }
            }

            if !msg_writer.is_done() {
                loop {
                    let flags = msg_writer.write_bits(chunks, &mut chan_iter);
                    if flags.continue_init { return Ok(()) }
                    if flags.is_done { break }
                }
            }
            Ok(())
        })
    }
}

// [=][-] Hider: Avg Sum
// ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━
// ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━
// [=][+] Hider: Less Sign

pub struct LessSignHider<ByteIter> {
    pub msg: ByteIter,
    pub ty: MsgType,

    pub initial_img: Vec<String>,
    pub modified_img: ImgPaths,
    
    /// Bits per chunk.
    /// Max value is 4.
    pub bits: u8,

    /// Gray output mode
    pub gray: bool,
}

impl<ByteIter: IntoIterator<Item = u8>> LessSignHider<ByteIter> {
    pub fn transmute_msg(self) -> LessSignHider<ByteIter::IntoIter> {
        LessSignHider {
            msg: self.msg.into_iter(),
            ty: self.ty,
            initial_img: self.initial_img,
            modified_img: self.modified_img,
            bits: self.bits,
            gray: self.gray,
        }
    }
}
impl<Any> LessSignHider<Any> {
    const HEADER_SIZE: usize = 4 + 4 + 2;
}
impl<ByteIter: ExactSizeIterator<Item = u8>> LessSignHider<ByteIter> {
    fn header_writer(&self) -> Result<LessSignHiderWriter<impl Iterator<Item = u8> + 'static>> {        
        let mut header = vec![];
        
        let mut rng = rand::rng();
        let mut rand_bits = rng.next_u32(); // TODO: need to possibility of adding noise
        if self.gray {
            rand_bits &= !1u32;
        } else {
            rand_bits |= 1u32;
        }
        header.extend((rand_bits as u32).to_le_bytes());

        let msg_len = self.msg.len();
        Error::test_too_big_msg(msg_len)?;
        header.extend((msg_len as u32).to_le_bytes());

        assert!(self.bits <= 4);
        let bits_and_gray = ((self.gray as u8) << 7) | self.bits;
        header.extend([self.ty as u8, bits_and_gray]);

        assert_eq!(header.len(), Self::HEADER_SIZE);
        Ok(LessSignHiderWriter::new(header.into_iter(), 1))
    }

    pub fn hide(self) -> Result<()> {
        let paths = paths_pair!(self);
        let mut header_writer = self.header_writer()?;
        let mut msg_writer = LessSignHiderWriter::new(self.msg, self.bits);

        if !self.gray {
            hider_loop(&mut msg_writer, paths, |msg_writer, img|{
                let mut chan_iter = img.pixels_mut().flat_map(|x|&mut x.0);
                header_writer.write_while_can(&mut chan_iter);
                msg_writer.write_while_can(&mut chan_iter);
                msg_writer.imitate(&mut chan_iter);
                Ok(())
            })
        } else {
            hider_loop(&mut msg_writer, paths, |msg_writer, img|{
                let mut pixel_iter = img.pixels_mut();
                header_writer.write_while_can_gray(&mut pixel_iter);
                msg_writer.write_while_can_gray(&mut pixel_iter);
                msg_writer.imitate_gray(&mut pixel_iter);
                Ok(())
            })
        }
    }
}

struct LessSignHiderWriter<I> {
    iter_bw: IterByteWriter<I>,
    mask: u8,
}
impl<I: Iterator<Item = u8>> LessSignHiderWriter<I> {
    pub fn new(iter: I, bits: u8) -> Self {
        Self {
            iter_bw: IterByteWriter::new(iter, bits),
            mask: !((1u8 << bits) - 1),
        }
    }

    /// # Return
    /// * `true` if all data is written
    pub fn write_while_can<'a>(&mut self, mut chan_iter: impl Iterator<Item = &'a mut u8>) -> bool {
        while !self.iter_bw.is_done() {
            let Some(byte) = chan_iter.next() else {
                return false
            };

            self.iter_bw.write_bits(|part_of_byte|{
                *byte &= self.mask;  
                *byte |= part_of_byte;
                true
            });
        }
        true
    }

    /// # Return
    /// * `true` if all data is written
    pub fn imitate<'a>(&self, mut chan_iter: impl Iterator<Item = &'a mut u8>) {
        let bits = 1 + !self.mask;
        for index in 0.. {
            let Some(byte) = chan_iter.next() else {
                return
            };

            let part_of_byte = PSEUDO_RAND_INDEXES[index % 256] as u8 % bits;
            *byte &= self.mask;  
            *byte |= part_of_byte;
        }
    }

    fn pixel_gray_hide(rgb: &mut Rgb<u8>, mask: u8, part_of_byte: u8) {
        let r = rgb.0[0] as u16;
        let g = rgb.0[1] as u16;
        let b = rgb.0[2] as u16;

        // grayscale coefs: 299  587  114  (total: 1000)
        //          approx: 300  575  125  (total: 1000)
        //          div 25:  12   23    5  (total:   40)
        let gray = 12 * r + 23 * g + 5 * b;
        let mut gray = (gray / 40) as u8;
        
        gray &= mask;  
        gray |= part_of_byte;

        rgb.0 = [gray, gray, gray];
    }

    /// # Return
    /// * `true` if all data is written
    pub fn write_while_can_gray<'a>(&mut self, mut pixel_iter: impl Iterator<Item = &'a mut Rgb<u8>>) -> bool {
        while !self.iter_bw.is_done() {
            let Some(rgb) = pixel_iter.next() else {
                return false
            };

            self.iter_bw.write_bits(|part_of_byte|{
                Self::pixel_gray_hide(rgb, self.mask, part_of_byte);
                true
            });
        }
        true
    }

    pub fn imitate_gray<'a>(&self, mut pixel_iter: impl Iterator<Item = &'a mut Rgb<u8>>) {
        let bits = 1 + !self.mask;
        for index in 0.. {
            let Some(rgb) = pixel_iter.next() else {
                return
            };

            let part_of_byte = PSEUDO_RAND_INDEXES[index % 256] as u8 % bits;
            Self::pixel_gray_hide(rgb, self.mask, part_of_byte);
        }
    }
}
impl<I: Iterator<Item = u8>> HiderWriter for LessSignHiderWriter<I> {
    fn is_done(&self) -> bool {
        self.iter_bw.is_done()
    }
    fn bytes_left(&mut self) -> usize {
        self.iter_bw.bytes_left()
    }
}

// [=][-] Hider: Less Sign
// ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━

// [-] Hider(s)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// [+] Revealer(s)

// ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━
// [=][+] Revealer: Delta

pub struct DeltaRevealer {
    pub initial_img: Vec<String>,
    pub modified_img: Vec<String>,
    pub save_path: Option<String>,
    pub bits: u8,
}
impl DeltaRevealer {
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

// [=][-] Revealer: Delta
// ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━
// ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━
// [=][+] Revealer: Avg Sum

pub struct AvgSumRevealer {
    pub modified_img: Vec<String>,
    pub save_path: Option<String>,
}
impl AvgSumRevealer {
    const HEADER_SIZE: usize = 7;

    fn msg_reader_ctor(header: &Vec<u8>) -> Result<(Option<AvgSumChunkReader>, MsgType)> {
        let ty = match MsgType::try_from_u8(header[0]) {
            Some(ty_x) => ty_x,
            _ => return Err(Error::InvalidMsgTypeByte(header[0])),
        };

        let bits_per_chunk = header[1];
        let chunk_size = header[2];
        
        let len_bytes: [u8; 4] = header[Self::HEADER_SIZE - 4..Self::HEADER_SIZE].try_into().unwrap();
        let msg_len = u32::from_le_bytes(len_bytes) as usize;

        let reader = AvgSumChunkReader::new(msg_len, chunk_size, bits_per_chunk);
        Ok((Some(reader), ty))
    }

    pub fn reveal(&self) -> Result<(Vec<u8>, MsgType)> {
        let chunk_size = AvgSumHider::HEADER_CHUNK_SIZE;
        let bits_per_chunk = AvgSumHider::HEADER_BITS_PER_CHUNK;
        let mut header_reader = AvgSumChunkReader::new(Self::HEADER_SIZE, chunk_size, bits_per_chunk);
        let mut ty = MsgType::Reserved;
        
        let mut msg_reader: Option<AvgSumChunkReader> = None;

        // TODO: it can be paralleled (by images & by chunks of pixels in an image)
        'modi: for path in &self.modified_img {
            let img = Img::open_img(path);
            let mut chan_iter = img.img.pixels().flat_map(|x|&x.0).cloned();

            if header_reader.read_while_can(&mut chan_iter) {
                continue 'modi
            } else if msg_reader.is_none() {
                assert_eq!(header_reader.buf.len(), Self::HEADER_SIZE);
                let header = &header_reader.buf;
                (msg_reader, ty) = Self::msg_reader_ctor(header)?;
            }
            
            if let Some(msg_reader) = &mut msg_reader { 
                if msg_reader.read_while_can(&mut chan_iter) {
                    continue 'modi
                }

                break 'modi
            }
        }

        if ty.is_reserved() {
            return Err(Error::UnreadedHeader);
        }
        let Some(msg_reader) = msg_reader else {
            return Err(Error::UnreadedHeader);
        };
        if msg_reader.buf.len() != msg_reader.expected_size {
            return Err(Error::UnfullResult(msg_reader.expected_size - msg_reader.buf.len()));
        }
        Ok((msg_reader.buf, ty))
    }
}

struct AvgSumChunkReader {
    reader: ConstBytesReader,
    buf: Vec<u8>,
    expected_size: usize,
    chunk_size: u8,
    rem: u16,
}
impl AvgSumChunkReader {
    fn new(expected_size: usize, chunk_size: u8, bits_per_chunk: u8) -> Self {
        let rem = 1u16 << bits_per_chunk;
        let reader = ConstBytesReader::new(bits_per_chunk);
        let buf = Vec::with_capacity(expected_size);
        Self {
            reader,
            buf,
            expected_size,
            chunk_size,
            rem,
        }
    }
    /// # Result
    /// is iter ended
    fn read_while_can(&mut self, mut chan_iter: impl Iterator<Item = u8>) -> bool {
        while self.buf.len() != self.expected_size {
            let mut sum = 0;
            for _ in 0..self.chunk_size {
                if let Some(byte) = chan_iter.next() {
                    sum += byte as u16;
                } else {
                    return true
                }
            }

            let part_of_byte = sum % self.rem;
            if let Some(byte) = self.reader.try_take_next_le_byte(part_of_byte as u8) {
                self.buf.push(byte)
            }
        }
        false
    }
}

// [=][-] Revealer: Avg Sum
// ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━
// ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━
// [=][+] Revealer: Less Sign

pub struct LessSignRevealer {
    pub modified_img: Vec<String>,
    pub save_path: Option<String>,
}
impl LessSignRevealer {
    fn is_gray_calc(first_pixel: &Rgb<u8>) -> Result<bool> {
        let is_gray = (first_pixel.0[0] & 1) == 0;
        if is_gray {
            let grayscale1 = first_pixel.0[0] == first_pixel.0[1];
            let grayscale2 = first_pixel.0[0] == first_pixel.0[2];
            if !grayscale1 || !grayscale2 {
                return Err(Error::Other("The header says that picture is grayscaled but it doesn't".into()))
            }
        }
        Ok(is_gray)
    }

    fn msg_reader_ctor(header: &Vec<u8>, expected_gray: bool) -> Result<(Option<ConstBufReader>, MsgType)> {
        let len_bytes: [u8; 4] = header[4..8].try_into().unwrap();
        let msg_len = u32::from_le_bytes(len_bytes) as usize;

        let ty = header[9 - 1];
        let bits_and_gray = header[10 - 1];

        let ty = match MsgType::try_from_u8(ty) {
            Some(ty_x) => ty_x,
            _ => return Err(Error::InvalidMsgTypeByte(ty)),
        };
     
        let bits = bits_and_gray & 0b1111;
        let gray = (bits_and_gray & (1 << 7)) != 0;
        if gray != expected_gray {
            return Err(Error::Other("Inconsistent header".into()))
        }

        let reader = ConstBufReader::new(msg_len, bits);
        Ok((Some(reader), ty))   
    }

    pub fn reveal(&self) -> Result<(Vec<u8>, MsgType)> {
        let mut gray: Option<bool> = None;
        let mut header_reader = ConstBufReader::new(LessSignHider::<()>::HEADER_SIZE, 1);
        let mut msg_reader = None;
        let mut ty = MsgType::Reserved;

        // TODO: it can be paralleled (by images & by chunks of pixels in an image)
        'modi: for path in &self.modified_img {
            let img = Img::open_img(path);
            let mut iter = img.img.pixels().peekable();

            // find out is the picture a grayscaled
            if gray.is_none() {
                let Some(first_pixel) = iter.peek() else {
                    continue 'modi // can .png pic have zero pixels?
                };
                gray = Some(Self::is_gray_calc(first_pixel)?);
            }

            let is_gray = gray.unwrap();
            let mut iter: Box<dyn Iterator<Item = u8>> = if is_gray {
                Box::new(img.img.pixels().map(|x|x.0[0]))
            } else {
                Box::new(img.img.pixels().flat_map(|x|&x.0).cloned())
            };

            if !header_reader.is_done() {
                let mask = header_reader.mask();
                header_reader.read_while_can(&mut iter, |iter| {
                    let byte = iter.next()?;
                    Some(byte & mask)
                });

                if !header_reader.is_done() {
                    continue 'modi
                }

                (msg_reader, ty) = Self::msg_reader_ctor(header_reader.buf_ref(), is_gray)?;
            }

            if let Some(msg_reader) = &mut msg_reader {
                let mask = msg_reader.mask();
                msg_reader.read_while_can(&mut iter, |iter| {
                    let byte = iter.next()?;
                    Some(byte & mask)
                });

                if msg_reader.is_done() { break 'modi }
            }
        }

        if ty.is_reserved() {
            return Err(Error::UnreadedHeader);
        }
        let Some(msg_reader) = msg_reader else {
            return Err(Error::UnreadedHeader);
        };
        if !msg_reader.is_done() {
            return Err(Error::UnfullResult(msg_reader.left_to_read()));
        }
        Ok((msg_reader.take_buf(), ty))
    }
}

// [=][-] Revealer: Less Sign
// ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━  ━━

// [-] Revealer(s)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━