use std::{ffi::OsString, path::Path, string::FromUtf8Error};

use image::{ImageError, ImageReader, RgbImage};
use thiserror::Error;
use clap::{Parser, Subcommand};

// TODO: hide without key : ord of avg of n bytes:  n = 4: [213, 215, 109, 217] -> 754 % 4 == 2 

const MAX_BIT_PER_CHAN: u8 = 4;
const MAX_WIN_SZ: u8 = 64;

#[derive(Error, Debug)]
#[must_use]
pub enum Error {
    #[error("Empty initial paths, use `--init` cli arg: `--init png_path_0 ... png_path_n`")]
    EmptyInit,
    #[error("Inconsistent lenght of modified paths, `--modt` arg should have same amount of paths as `--init`")]
    InconsistModLen,
    #[error("The dalta {0} is too big, should be no more than {MAX_BIT_PER_CHAN}")]
    TooBigDelta(u8),
    #[error("The msg is too big(no more than 4GB)")]
    TooBigMsg,
    #[error("Image save error: {0}")]
    ImageSave(Box<ImageError>),
    #[error("In case of reveal operation you should provide modified paths (with the same len)")]
    RevealWithoutModified,
    #[error("Inconsistent image sizes: {0}x{1} & {2}x{3}")]
    ImageInconsistentSize(u32, u32, u32, u32),
    #[error("Invalid revealed message: {0}")]
    InvalidMsg(Box<FromUtf8Error>),
    #[error("I/O error: {0}")]
    ErrorIO(std::io::Error),
    #[error("Save probelm.\nMost likely the prefix `file:`.\nFile path: \"{1}\".\nThe problem: {0}")]
    SaveProblem(std::io::Error, String),
    #[error("Unexpected byte({0}) of hide's type")]
    InvalidSimpleHideTypeByte(u8),
    #[error("Header was not readed (img too small)")]
    UnreadedHeader,
    #[error("Not enough size of initial images in total (need to hide {0} bytes more). To fix it add more images into `--init` arg.")]
    NotEnoughSizeOfInit(usize),
}
impl Error {
    pub fn test_too_big_msg(msg_len: usize) -> Result<()> {
        if msg_len > u32::MAX as usize {
            Err(Error::TooBigMsg)
        } else {
            Ok(())
        }
    }
}
impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::ErrorIO(err)
    }
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Parser, Debug)]
struct Cli {
    #[command(flatten)]
    info: Info,

    #[command(subcommand)]
    cmd: CliCmd,

    // TODO: reordering of pixels
}
#[derive(Debug, Subcommand)]
enum CliCmd {
    /// Hide a message into `.png`s by using initial & modified pictures
    SimpleHide {
        #[arg(long)]
        /// Message that is transmitted. 
        /// It's better if the message encrypted before steganography.
        msg: Msg,
    },
    /// Reveal a message from `.png`s by using initial & modified pictures
    SimpleReveal {
        #[arg(long)]
        /// If message is not a text, but is a file, then it will be saved to this path. 
        /// Otherwise into default: `file.bin`
        save: Option<String>,
    },
    /// Hide a message into `.png`'s by using only initial images
    OneHide {
        #[arg(long)]
        /// Message that is transmitted. 
        /// It's better if the message encrypted before steganography.
        msg: Msg,

        #[arg(long = "chunk-bits", default_value_t = 4)]
        /// Bits per chunk.
        /// Max value is 6.
        bits_per_chunk: u8,

        /// Size of a chunk.
        /// Max value is 64.
        #[arg(short, default_value_t = 8)]
        chunk_size: u8,

        // TODO: strategy : rand / the same / etc
    },
    /// Reveal a message from `.png`'s by using only modfied images
    OneReveal {
        #[arg(long)]
        /// If message is not a text, but is a file, then it will be saved to this path. 
        /// Otherwise into default: `file.bin`
        save: Option<String>,
    },
}

#[derive(Parser, Debug)]
pub struct Info {
    #[arg(long = "init", value_delimiter = ',')]
    /// Paths of initial .png
    initial_img: Vec<String>,
    
    #[arg(long = "mod", value_delimiter = ',')]
    /// Paths of modified .png
    modified_img: Option<Vec<String>>,

    #[arg(long = "bits", default_value_t = 1)]
    /// Bits per pixel channel. (Preferably 1 or 2). 
    /// Not allowed to be more than 4.
    bits_per_pixel_chan: u8,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Txt(String),
    File(String),
}
impl Msg {
    pub fn ty(&self) -> HideType {
        match self {
            Msg::Txt(_) => HideType::Txt,
            Msg::File(_) => HideType::File,
        }
    }

    pub fn into_bytes(self) -> Result<Vec<u8>> {
        match self {
            Msg::Txt(msg) => Ok(msg.into_bytes()),
            Msg::File(path) => {
                // TODO: can read it splitted/chunked, to make potential len infinity
                Ok(std::fs::read(path)?)
            }
        }
    }
}

impl From<OsString> for Msg {
    fn from(value: OsString) -> Self {
        let Some(str) = value.to_str() else {
            panic!("bad OS string (not a valid UTF-8): {value:?}")
        };
        
        const FILE_PREFIX: &str = "file:";
        if str.starts_with(FILE_PREFIX) {
            Self::File(str[FILE_PREFIX.len()..].to_string())
        } else {
            Self::Txt(str.to_string())
        }
    }
}

impl Cli {
    pub fn cli() -> Result<Self> {
        let mut cli_full = Cli::parse();
        let cli = &mut cli_full.info;
        
        if cli.initial_img.len() == 0 { return Err(Error::EmptyInit) }

        if let Some(x) = &cli.modified_img {
            if x.len() != cli.initial_img.len() {
                return Err(Error::InconsistModLen)
            }
        }

        cli.bits_per_pixel_chan = cli.bits_per_pixel_chan.max(1);
        if cli.bits_per_pixel_chan > MAX_BIT_PER_CHAN {
            return Err(Error::TooBigDelta(cli.bits_per_pixel_chan))
        }

        Ok(cli_full)
    }

    pub fn open_img(path: impl AsRef<Path>) -> RgbImage {
        ImageReader::open(path)
            .expect("expected file")
            .decode()
            .expect("expected valid img")
            .into_rgb8()
    }
}

#[repr(u8)]
#[derive(Clone, Copy)]
#[must_use]
pub enum HideType {
    Txt = 1,
    File = 2,
    ReservedPre = 254,
    Reserved = 255,
}
impl HideType {
    pub fn is_reserved(self) -> bool {
        match self {
            HideType::ReservedPre => true,
            HideType::Reserved => true,
            _ => false,
        }
    }

    pub fn try_from_u8(byte: u8) -> Option<Self> {
        Some(match byte {
            1 => Self::Txt,
            2 => Self::File,
            _ => return None,
        })
    }

    pub fn do_action(self, msg: Vec<u8>, save: Option<String>) -> Result<()> {
        match self {
            HideType::Txt => {
                let msg = String::from_utf8(msg)
                    .map_err(|err|Error::InvalidMsg(Box::new(err)))?;
                println!("msg: \"{msg}\"");
            }
            HideType::File => {
                let save_path = save.unwrap_or("file.bin".to_string());
                if let Err(err) = std::fs::write(&save_path, msg) {
                    return Err(Error::SaveProblem(err, save_path))
                }
                println!("Done!\nfile saved into \"{save_path}\"");
            }
            _ => {
                return Err(Error::InvalidSimpleHideTypeByte(self as u8));      
            }
        }
        Ok(())
    }
}

pub struct OneByteWriter {
    cur_byte: u8,
    cur_bit: u8,
}
impl OneByteWriter {
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

pub struct OneByteReader {
    cur_byte: u8,
    cur_bit: u8,
}
impl OneByteReader {
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
    pub fn take_byte(&self) -> u8 {
        self.cur_byte
    }
    #[inline(always)]
    pub fn next_le(&mut self, part_of_byte: u8, bits: u8) -> bool {
        // self.cur_byte |= part_of_byte << self.cur_bit;
        self.cur_byte |= u8::wrapping_shl(part_of_byte, self.cur_bit as u32);
        self.cur_bit += bits;
        self.is_done()
    }
}


pub struct ConstBitOneByteWriter {
    bw: OneByteWriter,
    bits: u8,
}
impl ConstBitOneByteWriter {
    fn new(first_byte: u8, bits: u8) -> Self {
        Self {
            bw: OneByteWriter::new(first_byte),
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
        self.bw = OneByteWriter::new(byte);
    }
}

pub struct ConstBitOneByteReader {
    br: OneByteReader,
    bits: u8,
}
impl ConstBitOneByteReader {
    fn new(bits: u8) -> Self {
        Self {
            br: OneByteReader::new(),
            bits,
        }
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
        let ret = self.next_le(part_of_byte).then(||self.br.take_byte());
        if ret.is_some() { self.reset(); }
        ret
    }
    #[inline(always)]
    pub fn reset(&mut self) {
        self.br = OneByteReader::new();
    }
}

struct SimpleByteWriter {
    bw: ConstBitOneByteWriter,
}
impl SimpleByteWriter {
    fn new(first_byte: u8, bits: u8) -> Self {
        Self {
            bw: ConstBitOneByteWriter::new(first_byte, bits),
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

struct IterByteWriter<I> {
    bw: ConstBitOneByteWriter,
    iter: I,
    is_done: bool,
}
impl<I: Iterator<Item = u8>> IterByteWriter<I> {
    fn new(mut iter: I, bits: u8) -> Self {
        let first_byte = iter.next(); 
        Self {
            bw: ConstBitOneByteWriter::new(first_byte.unwrap_or(0), bits),
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
}

struct SimpleByteReader {
    cur_bit: u8,
    cur_byte: u8,
    bits_per_pixel_chan: u8,
}
impl SimpleByteReader {
    fn new(bits_per_pixel_chan: u8) -> Self {
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

struct SimpleByteMsgWriter<Iter: Iterator<Item = u8>> {
    writer: SimpleByteWriter,
    header: Vec<u8>,
    msg_iter: Iter,

    len_written: usize,
    is_done: bool,
}
impl<Iter: Iterator<Item = u8>> SimpleByteMsgWriter<Iter> {
    pub fn new(msg_len: usize, msg_iter: Iter, bits_per_pixel_chan: u8, ty: HideType) -> Result<Self> {
        Error::test_too_big_msg(msg_len)?;
        
        let mut header = Vec::with_capacity(5);
        header.push(ty as u8);
        let msg_len_bytes = u32::to_le_bytes(msg_len as u32);
        header.extend(msg_len_bytes);
        let writer = SimpleByteWriter::new(header[0], bits_per_pixel_chan);

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


struct SimpleByteMsgReader {
    reader: SimpleByteReader,
    msg_len_bytes: [u8; 4],
    index_write: usize,
    msg_size: usize,
    msg: Option<Vec<u8>>,
    ty: Option<HideType>,
}
impl SimpleByteMsgReader {
    pub fn new(bits_per_pixel_chan: u8) -> Self {
        Self {
            reader: SimpleByteReader::new(bits_per_pixel_chan),
            msg_len_bytes: u32::to_le_bytes(0),
            index_write: 0,
            msg_size: 0,
            msg: None,
            ty: None,
        }
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
            match HideType::try_from_u8(byte) {
                Some(ty) => { self.ty = Some(ty); }
                _ => return Err(Error::InvalidSimpleHideTypeByte(byte)),
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

#[derive(Default)]
struct OneHideWriterFlags {
    pub continue_init: bool,
    pub is_done: bool,
}

struct TopBottomChunks<'a, 'b> {
    chunk_top: &'b mut Vec<&'a mut u8>,
    chunk_bottom: &'b mut Vec<&'a mut u8>,
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

struct OneHideBlockWriter<I> {
    iter_bw: IterByteWriter<I>,
    sum: u16,
    rem: u8,
    bits_per_chunk: u8,
    chunk_size: u8,
    max_chan_delta: u8,
    
    // TODO: strategy (cur strategy is pseudo random & filling small firstly)
    pseudo_rand_index: u8,
}
impl<I: Iterator<Item = u8>> OneHideBlockWriter<I> {
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
        self.iter_bw.iter.count()
    }

    pub fn write_bits<'a, ChanI>(&mut self, chunk: &mut TopBottomChunks<'a, '_>, mut chan_iter: ChanI) -> OneHideWriterFlags
    where ChanI: Iterator<Item = &'a mut u8>
    {
        let mut flags = OneHideWriterFlags::default();

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

impl Info {
    fn save_img(&self, index: usize, path: &str, img: RgbImage) -> Result<()> {
        let save_result = if let Some(out_paths) = &self.modified_img {
            img.save_with_format(&out_paths[index], image::ImageFormat::Png)
        } else {
            let mut path = path.strip_suffix(".png").unwrap_or(&path).to_string();
            path.push_str("_mod.png");
            img.save_with_format(&path, image::ImageFormat::Png)
        };
        save_result.map_err(|err|Error::ImageSave(Box::new(err)))
    }

    fn take_modified(&mut self) -> Result<Vec<String>> {
        if self.modified_img.is_none() {
            return Err(Error::RevealWithoutModified);
        }

        let modified_img = std::mem::take(&mut self.modified_img).unwrap();
        if modified_img.len() != self.initial_img.len() {
            return Err(Error::RevealWithoutModified);
        }
        Ok(modified_img)
    }

    pub fn simple_hide_bytes(self, msg: Vec<u8>, ty: HideType) -> Result<()> {
        let cli = self;

        let msg_len = msg.len();
        let msg_iter = msg.into_iter();
        let bits = cli.bits_per_pixel_chan;
        let mut msg_writer = SimpleByteMsgWriter::new(msg_len, msg_iter, bits, ty)?;
        
        // TODO: it can be paralleled (by images & by chunks of pixels in an image)
        for (index, path) in cli.initial_img.iter().enumerate() {
            let mut img = Cli::open_img(path);

            let chan_iter = img.pixels_mut().flat_map(|x|&mut x.0);
            msg_writer.write(chan_iter);

            cli.save_img(index, path, img)?;
            if msg_writer.is_done() { break }
        }
        if !msg_writer.is_done() {
            return Err(Error::NotEnoughSizeOfInit(msg_writer.bytes_left()));
        }
        Ok(())
    }

    pub fn one_hide_bytes(self, msg: Vec<u8>, bits_per_chunk: u8, chunk_size: u8, ty: HideType) -> Result<()> {
        let msg_len = msg.len();
        Error::test_too_big_msg(msg_len)?;

        let mut chunk_buf_top: Vec<&mut u8> = Vec::with_capacity(MAX_WIN_SZ as usize);
        let mut chunk_buf_bottom: Vec<&mut u8> = Vec::with_capacity(MAX_WIN_SZ as usize);

        let mut header = vec![ty as u8, bits_per_chunk, chunk_size];
        header.extend((msg_len as u32).to_le_bytes());

        const HEADER_CHUNK_SIZE: u8 = 8;
        const HEADER_BITS_PER_CHUNK: u8 = 4;
        let mut header_writer = OneHideBlockWriter::new(header, HEADER_BITS_PER_CHUNK, HEADER_CHUNK_SIZE);
        let mut msg_writer = OneHideBlockWriter::new(msg, bits_per_chunk, chunk_size);

        'init: for (index, path) in self.initial_img.iter().enumerate() {
            let mut img = Cli::open_img(path);
            let mut chan_iter = img.pixels_mut().flat_map(|x|&mut x.0);
 
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
                        self.save_img(index, path, img)?;
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
                        self.save_img(index, path, img)?;
                        continue 'init
                    }
                    if flags.is_done { break }
                }
            }

            chunk.clear();
            self.save_img(index, path, img)?;
            if msg_writer.is_done() { break }
        }

        if !msg_writer.is_done() {
            return Err(Error::NotEnoughSizeOfInit(msg_writer.bytes_left()));
        }
        Ok(())
    }
}

const HALF: u8 = u8::MAX / 2;

fn main() {
    let start = std::time::Instant::now();
    
    if let Err(err) = main_inner() {
        println!("Error: {err}")
    } else {
        println!("\nduration: {}ms", start.elapsed().as_millis());
    }
}
fn main_inner() -> Result<()> {
    let cli = Cli::cli()?;
    let (mut cli, cmd) = (cli.info, cli.cmd);

    match cmd {
        CliCmd::SimpleHide { msg } => {
            let ty = msg.ty();
            cli.simple_hide_bytes(msg.into_bytes()?, ty)?;
        }

        CliCmd::SimpleReveal { save } => {
            let modified_img = cli.take_modified()?;
            let mut msg_reader = SimpleByteMsgReader::new(cli.bits_per_pixel_chan);
            
            // TODO: it can be paralleled (by images & by chunks of pixels in an image)
            for (path_a, path_b) in cli.initial_img.iter().zip(modified_img.iter()) {
                let img_a = Cli::open_img(path_a);
                let img_b = Cli::open_img(path_b);
                if img_a.width() != img_b.width() || img_a.height() != img_b.height() {
                    return Err(Error::ImageInconsistentSize(
                        img_a.width(),
                        img_a.height(),
                        img_b.width(), 
                        img_b.height()
                    ))
                }

                let chan_iter_a = img_a.pixels().flat_map(|x|&x.0).cloned();
                let chan_iter_b = img_b.pixels().flat_map(|x|&x.0).cloned();
                let chan_pair_iter = chan_iter_a.zip(chan_iter_b);
                msg_reader.read(chan_pair_iter)?;

                if msg_reader.is_finished() { break }
            }

            let Some(ty) = msg_reader.ty else {
                return Err(Error::UnreadedHeader);
            };
            let Some(msg) = msg_reader.take_msg() else {
                return Err(Error::UnreadedHeader);
            };

            ty.do_action(msg, save)?;
        }

        CliCmd::OneHide { msg, bits_per_chunk, chunk_size } => {
            let ty = msg.ty();
            let msg_bytes = msg.into_bytes()?;
            cli.one_hide_bytes(msg_bytes, bits_per_chunk, chunk_size, ty)?;
        }

        CliCmd::OneReveal { save } => {
            let modified_img = cli.take_modified()?;

            const HEADER_SIZE: usize = 7;
            const HEADER_CHUNK_SIZE: u8 = 8;
            const HEADER_BITS_PER_CHUNK: u8 = 4;

            let mut chunk_size = HEADER_CHUNK_SIZE;
            let mut bits_per_chunk = HEADER_BITS_PER_CHUNK;
            let mut rem = 1u16 << bits_per_chunk;
            let mut sum;
            let mut header_reader = ConstBitOneByteReader::new(bits_per_chunk);
            let mut header = Vec::with_capacity(HEADER_SIZE);
            let mut ty = HideType::Reserved;
            let mut msg_len = 0;
            
            let mut msg_reader: Option<(ConstBitOneByteReader, Vec<u8>)> = None;

            // TODO: it can be paralleled (by images & by chunks of pixels in an image)
            'modi: for path in &modified_img {
                let img = Cli::open_img(path);
                let mut chan_iter = img.pixels().flat_map(|x|&x.0).cloned();

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
                    match HideType::try_from_u8(header[0]) {
                        Some(ty_x) => ty = ty_x,
                        _ => return Err(Error::InvalidSimpleHideTypeByte(header[0])),
                    };
                    bits_per_chunk = header[1];
                    rem = 1u16 << bits_per_chunk;
                    chunk_size = header[2];
                    let len_bytes: [u8; 4] = header[HEADER_SIZE - 4..HEADER_SIZE].try_into().unwrap();
                    msg_len = u32::from_le_bytes(len_bytes) as usize;
                    
                    let msg = Vec::with_capacity(msg_len);
                    msg_reader = Some((ConstBitOneByteReader::new(bits_per_chunk), msg));
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
            ty.do_action(msg, save)?;
        }
    }

    Ok(())
}

const PSEUDO_RAND_INDEXES: &[usize; u8::MAX as usize + 1] = &[
    111, 105, 117, 126, 121, 100, 123, 109, 76, 58, 105, 20, 99, 96, 20, 42, 61, 3, 114, 14, 
    28, 120, 75, 60, 71, 38, 20, 45, 24, 26, 108, 91, 25, 48, 88, 23, 107, 88, 43, 60, 
    20, 82, 66, 112, 79, 69, 18, 100, 116, 89, 90, 86, 18, 34, 84, 121, 117, 29, 81, 114, 
    72, 37, 7, 111, 39, 111, 68, 35, 115, 90, 81, 56, 40, 2, 46, 50, 108, 0, 55, 109, 
    18, 21, 0, 100, 23, 37, 22, 65, 19, 71, 51, 18, 30, 34, 45, 33, 109, 99, 122, 44, 
    104, 42, 68, 39, 61, 109, 4, 44, 44, 112, 34, 78, 51, 30, 58, 59, 51, 38, 119, 69, 
    108, 28, 18, 77, 13, 105, 87, 56, 99, 33, 35, 31, 44, 23, 80, 1, 94, 92, 89, 4, 
    29, 52, 15, 6, 36, 24, 47, 6, 56, 48, 127, 51, 85, 39, 29, 91, 3, 63, 3, 100, 
    35, 85, 69, 122, 102, 25, 65, 60, 19, 121, 77, 122, 44, 107, 82, 107, 3, 45, 71, 26, 
    127, 118, 7, 0, 121, 79, 98, 98, 49, 17, 36, 55, 71, 14, 14, 28, 53, 46, 59, 112, 
    43, 68, 5, 57, 73, 64, 127, 47, 72, 98, 65, 23, 119, 77, 50, 0, 81, 52, 11, 61, 
    76, 66, 83, 126, 89, 24, 24, 67, 82, 103, 115, 114, 73, 70, 56, 103, 122, 123, 54, 11, 
    20, 29, 37, 55, 13, 45, 100, 25, 86, 115, 34, 81, 101, 10, 79, 80, 
];
