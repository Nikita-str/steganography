use std::path::Path;
use clap::{Parser, Subcommand};
use image::{ImageReader, RgbImage};

use steganography::prelude::*;
use steganography::png::prelude::*;
use steganography::png::writer::TopBottomChunks;


#[derive(Parser, Debug)]
pub struct Cli {
    #[command(flatten)]
    pub info: Info,

    #[command(subcommand)]
    pub cmd: CliCmd,

    // TODO: reordering of pixels
}
#[derive(Debug, Subcommand)]
pub enum CliCmd {
    /// Hide a message into `.png`s by using delta of initial & modified(will be created) pictures
    DeltaHide {
        #[arg(long)]
        /// Message that is transmitted. 
        /// It's better if the message encrypted before steganography.
        msg: Msg,
    },
    /// Reveal a message from `.png`s by using delta of initial & modified pictures
    DeltaReveal {
        #[arg(long)]
        /// If message is not a text, but is a file, then it will be saved to this path. 
        /// Otherwise into default: `file.bin`
        save: Option<String>,
    },
    /// Hide a message into `.png`'s by using avg sum of initial images (modified will be created)
    AvgSumHide {
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
    /// Reveal a message from `.png`'s by using avg sum of modfied images
    AvgSumReveal {
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
    pub initial_img: Vec<String>,
    
    #[arg(long = "mod", value_delimiter = ',')]
    /// Paths of modified .png
    pub modified_img: Option<Vec<String>>,

    #[arg(long = "bits", default_value_t = 1)]
    /// Bits per pixel channel. (Preferably 1 or 2). 
    /// Not allowed to be more than 4.
    pub bits_per_pixel_chan: u8,
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


impl Info {
    pub fn save_img(&self, index: usize, path: &str, img: RgbImage) -> Result<()> {
        let save_result = if let Some(out_paths) = &self.modified_img {
            img.save_with_format(&out_paths[index], image::ImageFormat::Png)
        } else {
            let mut path = path.strip_suffix(".png").unwrap_or(&path).to_string();
            path.push_str("_mod.png");
            img.save_with_format(&path, image::ImageFormat::Png)
        };
        save_result.map_err(|err|Error::ImageSave(Box::new(err)))
    }

    pub fn take_modified(&mut self) -> Result<Vec<String>> {
        if self.modified_img.is_none() {
            return Err(Error::RevealWithoutModified);
        }

        let modified_img = std::mem::take(&mut self.modified_img).unwrap();
        if modified_img.len() != self.initial_img.len() {
            return Err(Error::RevealWithoutModified);
        }
        Ok(modified_img)
    }

    pub fn delta_hide_bytes(self, msg: Vec<u8>, ty: MsgType) -> Result<()> {
        let cli = self;

        let msg_len = msg.len();
        let msg_iter = msg.into_iter();
        let bits = cli.bits_per_pixel_chan;
        let mut msg_writer = DeltaByteMsgWriter::new(msg_len, msg_iter, bits, ty)?;
        
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

    pub fn avg_sum_hide_bytes(self, msg: Vec<u8>, bits_per_chunk: u8, chunk_size: u8, ty: MsgType) -> Result<()> {
        let msg_len = msg.len();
        Error::test_too_big_msg(msg_len)?;

        let mut chunk_buf_top: Vec<&mut u8> = Vec::with_capacity(MAX_WIN_SZ as usize);
        let mut chunk_buf_bottom: Vec<&mut u8> = Vec::with_capacity(MAX_WIN_SZ as usize);

        let mut header = vec![ty as u8, bits_per_chunk, chunk_size];
        header.extend((msg_len as u32).to_le_bytes());

        const HEADER_CHUNK_SIZE: u8 = 8;
        const HEADER_BITS_PER_CHUNK: u8 = 4;
        let mut header_writer = AvgSumHideBlockWriter::new(header, HEADER_BITS_PER_CHUNK, HEADER_CHUNK_SIZE);
        let mut msg_writer = AvgSumHideBlockWriter::new(msg, bits_per_chunk, chunk_size);

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