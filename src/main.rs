use std::{ffi::OsString, path::Path, string::FromUtf8Error};

use image::{ImageError, ImageReader, RgbImage};
use thiserror::Error;
use clap::{Parser, Subcommand};

// TODO: hide without key : ord of avg of n bytes:  n = 4: [213, 215, 109, 217] -> 754 % 4 == 2 

const MAX_BIT_PER_CHAN: u8 = 4;

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
    #[error("Unexpected byte({0}) of simple hide's type")]
    InvalidSimpleHideTypeByte(u8),
    #[error("Header was not readed (img too small)")]
    UnreadedHeader,
    #[error("Not enough size of initial images in total (need to hide {0} bytes more). To fix it add more images into `--init` arg.")]
    NotEnoughSizeOfInit(usize),
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
    /// Hide a message into `.png`s 
    SimpleHide {
        #[arg(long)]
        /// Message that is transmitted. 
        /// It's better if the message encrypted before steganography.
        msg: Msg,
    },
    /// Reveal a message from `.png`s
    SimpleReveal {
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
enum Msg {
    Txt(String),
    File(String),
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
pub enum SimpleHideType {
    Txt = 1,
    File = 2,
    ReservedPre = 254,
    Reserved = 255,
}
impl SimpleHideType {
    pub fn try_from_u8(byte: u8) -> Option<Self> {
        Some(match byte {
            1 => Self::Txt,
            2 => Self::File,
            _ => return None,
        })
    }
}

struct SimpleByteWriter {
    cur_bit: u8,
    cur_byte: u8,
    bits_per_pixel_chan: u8,
    mask: u8,
}
impl SimpleByteWriter {
    fn new(first_byte: u8, bits_per_pixel_chan: u8) -> Self {
        Self {
            cur_bit: 0,
            cur_byte: first_byte,
            bits_per_pixel_chan,
            mask: (1 << (bits_per_pixel_chan)) as u8 - 1,
        }
    }

    #[inline]
    pub fn update_byte(&mut self, byte: &mut u8) {
        let delta = self.cur_byte & self.mask;
                    
        if *byte < HALF {
            *byte += delta;
        } else {
            *byte -= delta;
        }

        self.cur_byte >>= self.bits_per_pixel_chan;
        self.cur_bit += self.bits_per_pixel_chan;
    }
    
    #[inline]
    pub fn need_next(&self) -> bool {
        self.cur_bit >= 8
    }

    #[inline]
    pub fn set_next_byte(&mut self, byte: u8) {
        self.cur_bit = 0;
        self.cur_byte = byte;
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
    finished: bool,
}
impl<Iter: Iterator<Item = u8>> SimpleByteMsgWriter<Iter> {
    pub fn new(msg_len: usize, msg_iter: Iter, bits_per_pixel_chan: u8, ty: SimpleHideType) -> Result<Self> {        
        if msg_len > u32::MAX as usize {
            return Err(Error::TooBigMsg)
        }
        
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
            finished: false,
        })
    }

    #[inline(always)]
    pub fn bytes_left(self) -> usize {
        self.msg_iter.count()
    }

    #[inline(always)]
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    #[inline(always)]
    pub fn need_write_len(&self) -> bool {
        self.len_written < self.header.len()
    }

    #[inline(always)]
    pub fn need_write_msg(&self) -> bool {
        !self.need_write_len() && !self.finished
    }
    
    /// # Return
    /// * bool = ControlFlow::Break
    #[inline]
    fn set_next_byte_from_iter(&mut self) -> bool {
        if let Some(byte) = self.msg_iter.next() {
            self.writer.set_next_byte(byte);
        } else {
            self.finished = true;
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
                self.writer.set_next_byte(byte);
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
    ty: Option<SimpleHideType>,
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
            match SimpleHideType::try_from_u8(byte) {
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

    pub fn simple_hide_bytes(self, msg: Vec<u8>, ty: SimpleHideType) -> Result<()> {
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
            if msg_writer.is_finished() { break }
        }
        if !msg_writer.is_finished() {
            return Err(Error::NotEnoughSizeOfInit(msg_writer.bytes_left()));
        }
        Ok(())
    }
}

const HALF: u8 = u8::MAX / 2;

fn main() {
    if let Err(err) = main_inner() {
        println!("Error: {err}")
    }
}
fn main_inner() -> Result<()> {
    let cli = Cli::cli()?;
    let (cli, cmd) = (cli.info, cli.cmd);

    match cmd {
        CliCmd::SimpleHide { msg: Msg::Txt(msg) } => {
            cli.simple_hide_bytes(msg.into_bytes(), SimpleHideType::Txt)?;
        }
        CliCmd::SimpleHide { msg: Msg::File(path) } => {
            // TODO: can read it splitted/chunked, to make potential len infinity
            let msg = std::fs::read(path)?;
            cli.simple_hide_bytes(msg, SimpleHideType::File)?;
        }
        
        CliCmd::SimpleReveal { save } => {
            if cli.modified_img.is_none() {
                return Err(Error::RevealWithoutModified);
            }

            let modified_img = cli.modified_img.unwrap();
            if modified_img.len() != cli.initial_img.len() {
                return Err(Error::RevealWithoutModified);
            }

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

            match ty {
                SimpleHideType::Txt => {
                    let msg = String::from_utf8(msg)
                        .map_err(|err|Error::InvalidMsg(Box::new(err)))?;
                    println!("msg: \"{msg}\"");
                }
                SimpleHideType::File => {
                    let save_path = save.unwrap_or("file.bin".to_string());
                    if let Err(err) = std::fs::write(&save_path, msg) {
                        return Err(Error::SaveProblem(err, save_path))
                    }
                    println!("Done!\nfile saved into \"{save_path}\"");
                }
                _ => {
                    return Err(Error::InvalidSimpleHideTypeByte(ty as u8));      
                }
            }
        }
    }

    Ok(())
}
