use std::{path::Path, string::FromUtf8Error};

use image::{ImageError, ImageReader, Rgb, RgbImage};
use thiserror::Error;
use clap::{Parser, Subcommand};

// TODO: hide without key : ord of avg of n bytes:  n = 4: [213, 215, 109, 217] -> 754 % 4 == 2 

const MAX_BIT_PER_CHAN: u8 = 4;

#[derive(Error, Debug)]
enum Error {
    #[error("Empty initial paths, use `--in` cli arg: `--in png_path_0 ... png_path_n`")]
    EmptyInit,
    #[error("Inconsistent lenght of modified paths, `--out` arg should have same amount of paths as `--in`")]
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
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Parser, Debug)]
struct Cli {
    #[arg(long = "init")]
    /// Paths of initial .png
    initial_img: Vec<String>,
    
    #[arg(long = "mod")]
    /// Paths of modified .png
    modified_img: Option<Vec<String>>,

    #[arg(long = "bits", default_value_t = 1)]
    /// Bits per pixel channel. (Preferably 1 or 2). 
    /// Not allowed to be more than 4.
    bits_per_pixel_chan: u8,

    #[arg(short = 'x')]
    /// If the flag setted 
    decode: bool,

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
        msg: String,
    },
    /// Reveal a message from `.png`s
    SimpleReveal { },
}

impl Cli {
    pub fn cli() -> Result<Self> {
        let mut cli = Cli::parse();
        
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

        Ok(cli)
    }

    pub fn open_img(path: impl AsRef<Path>) -> RgbImage {
        ImageReader::open(path)
            .expect("expected file")
            .decode()
            .expect("expected valid img")
            .into_rgb8()
    }
}

struct ImgWriter<'iter> {
    pixels: &'iter mut Rgb<u8>,
    bits: u8,
    written_bits: u8,
}
// impl<'iter> ImgWriter<'iter> {
//     pub fn new(img: &mut RgbaImage, max_delta: u8, written_bits: u8) -> Self {
//         Self {
//             pixels: img.pixels_mut(),
//             max_delta,
//             written_bits,
//         }
//     }
// }

const HALF: u8 = u8::MAX / 2;

fn main() {
    if let Err(err) = main_inner() {
        println!("Error: {err}")
    }
}
fn main_inner() -> Result<()> {
    let cli = Cli::cli()?;

    let mask = (1 << (cli.bits_per_pixel_chan)) as u8 - 1;

    match &cli.cmd {
        CliCmd::SimpleHide { msg } => {
            let msg_len = msg.len();
            let mut len_written = false;
            
            if msg_len > u32::MAX as usize {
                return Err(Error::TooBigMsg)
            }
            let msg_len_bytes = u32::to_le_bytes(msg_len as u32);
            let mut msg_len_bytes_iter = msg_len_bytes.iter();
            let mut msg_iter = msg.as_bytes().iter();

            let mut finished = false;
            let mut cur_bit = 0;
            let mut cur_byte = *msg_len_bytes_iter.next().unwrap();
            
            // TODO: it can be paralleled (by images & by chunks of pixels in an image)
            for (index, path) in cli.initial_img.iter().enumerate() {
                let mut img = Cli::open_img(path);
                let mut pix_chan_iter = img.pixels_mut().flat_map(|x|&mut x.0);
                
                if !len_written {
                    loop {
                        let Some(pixel) = pix_chan_iter.next() else { break };

                        let delta = cur_byte & mask;
                    
                        if *pixel < HALF {
                            *pixel += delta;
                        } else {
                            *pixel -= delta;
                        }

                        cur_byte >>= cli.bits_per_pixel_chan;
                        cur_bit += cli.bits_per_pixel_chan;
                        if cur_bit >= 8 {
                            cur_bit = 0;
                            if let Some(byte) = msg_len_bytes_iter.next() {
                                cur_byte = *byte;
                            } else {
                                len_written = true;
                                if let Some(byte) = msg_iter.next() {
                                    cur_byte = *byte;
                                } else {
                                    finished = true;
                                }
                                break;
                            }
                        }
                    }
                }

                if !finished && len_written {
                    for pixel in pix_chan_iter {
                        let delta = cur_byte & mask;
                        
                        if *pixel < HALF {
                            *pixel += delta;
                        } else {
                            *pixel -= delta;
                        }

                        cur_byte >>= cli.bits_per_pixel_chan;
                        cur_bit += cli.bits_per_pixel_chan;
                        if cur_bit >= 8 {
                            cur_bit = 0;
                            if let Some(byte) = msg_iter.next() {
                                cur_byte = *byte;
                            } else {
                                finished = true;
                                break;
                            }
                        }
                    }
                }

                let save_result = if let Some(out_paths) = &cli.modified_img {
                    img.save_with_format(&out_paths[index], image::ImageFormat::Png)
                } else {
                    let mut path = path.strip_suffix(".png").unwrap_or(&path).to_string();
                    path.push_str("_mod.png");
                    img.save_with_format(&path, image::ImageFormat::Png)
                };
                if let Err(err) = save_result {
                    Error::ImageSave(Box::new(err));
                }

                if finished { break }
            }
        }
        
        CliCmd::SimpleReveal {  } => {
            if cli.modified_img.is_none() {
                return Err(Error::RevealWithoutModified);
            }

            let modified_img = cli.modified_img.unwrap();
            if modified_img.len() != cli.initial_img.len() {
                return Err(Error::RevealWithoutModified);
            }

            let mut msg_len_bytes = u32::to_le_bytes(0);
            let mut index_write = 0usize;
            let mut msg_size = 0usize;
            let mut msg: Option<Vec<u8>> = None;
            let mut cur_bit = 0;
            let mut cur_byte = 0;

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
                let chan_iter_a = img_a.pixels().flat_map(|x|&x.0);
                let chan_iter_b = img_b.pixels().flat_map(|x|&x.0);
                let mut chan_pair_iter = chan_iter_a.zip(chan_iter_b);
                
                if msg.is_none() {
                    loop {
                        let Some((pixel_a, pixel_b)) = chan_pair_iter.next() else { break };
                        let delta = if pixel_a >= pixel_b {
                            *pixel_a - *pixel_b
                        } else {
                            *pixel_b - *pixel_a
                        };
    
                        cur_byte = (delta << cur_bit) | cur_byte;
                        cur_bit += cli.bits_per_pixel_chan;
                        if cur_bit >= 8 {
                            msg_len_bytes[index_write] = cur_byte;
                            cur_bit = 0;
                            cur_byte = 0;
                            index_write += 1;
                            if index_write == msg_len_bytes.len() {
                                msg_size = u32::from_le_bytes(msg_len_bytes) as usize;
                                msg = Some(Vec::<u8>::with_capacity(msg_size));
                                index_write = 0;
                                break;
                            }
                        }
                    }
                } 

                if let Some(msg) = &mut msg {
                    for (pixel_a, pixel_b) in chan_pair_iter {
                        let delta = if pixel_a >= pixel_b {
                            *pixel_a - *pixel_b
                        } else {
                            *pixel_b - *pixel_a
                        };
                        
                        cur_byte = (delta << cur_bit) | cur_byte;
                        cur_bit += cli.bits_per_pixel_chan;
                        if cur_bit >= 8 {
                            msg.push(cur_byte);
                            cur_bit = 0;
                            cur_byte = 0;
                            index_write += 1;
                            if index_write >= msg_size { break }
                        }
                    }
                }                
                if msg.is_some() && index_write >= msg_size { break }
            }

            if let Some(msg) = msg {
                let msg = String::from_utf8(msg)
                    .map_err(|err|Error::InvalidMsg(Box::new(err)))?;
                println!("msg: \"{msg}\"");
            }
        }
    }

    Ok(())
}
