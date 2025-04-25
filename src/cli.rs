use std::borrow::Cow;

use clap::{Parser, Subcommand};

use steganography::prelude::*;
use steganography::png::algo as algo;
use steganography::png::prelude::*;
use steganography::text::RepeatConstTypo;

// TODO: text in code comment
// TODO: .webp
// TODO: alpha channel : ignore?

// TODO: chacha8rng for noise 

#[derive(Parser, Debug)]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: CliCmd,
}

#[derive(Parser, Debug)]
pub enum CliCmd {
    #[command(name = "txt")]
    TxtCmd(TxtArgs),

    #[command(name = "pic")]
    PicCmd (PicArgs),
}


#[derive(Parser, Debug)]
pub struct TxtArgs {
    #[arg(long = "init")]
    /// Initial text. Can be text, or txt file (add prefix `file:` then).
    init: Msg,
    
    #[command(subcommand)]
    pub cmd: TxtCmd,
}

#[derive(Debug, Subcommand)]
pub enum TxtCmd {
    #[command(name = "txt-repeat-hide", aliases = &["txt-rh", "txt-r+"])]
    RepeatHide {
        #[arg(long = "msg")]
        /// Message that will be hidden.
        msg: Msg,
        
        #[arg(long = "mod")]
        /// Where save hidden message.
        modified: Option<String>,

        #[arg(long = "freq")]
        /// Frequency of a repeat typo.
        bit_freq: usize,

        #[arg(long = "typo-a", default_value_t = ' ')]
        /// Typo for cases when we cannot just repeat.
        typo_a: char,

        #[arg(long = "typo-b", default_value_t = '.')]
        /// Second typo for cases when we cannot just repeat.
        typo_b: char,
    },

    #[command(name = "txt-repeat-reveal", aliases = &["txt-rr", "txt-r="])]
    RepeatReveal {
        #[arg(long = "mod")]
        /// Modified text.
        modified: Msg,

        #[arg(long = "freq")]
        /// Frequency of a repeat typo.
        bit_freq: usize,
        
        #[command(flatten)]
        save: SaveArg,
    },
}

#[derive(Parser, Debug)]
pub struct PicArgs {
    #[command(flatten)]
    pub info: Info,

    #[command(subcommand)]
    pub cmd: PicCmd,

    // TODO: reordering of pixels
}

#[derive(Debug, Subcommand)]
pub enum PicCmd {
    #[command(name = "png-delta-hide", aliases = &["png-dh", "png-d+"])]
    /// Hide a message into `.png`s by using delta of initial & modified(will be created) pictures
    DeltaHide {
        #[arg(long)]
        /// Message that is transmitted (if it is a file, add preffix: `file:`). 
        /// It's better if the message encrypted before steganography.
        msg: Msg,
            
        #[command(flatten)]
        arg: DeltaArg,
    },
    
    #[command(name = "png-delta-reveal", aliases = &["png-dr", "png-d="])]
    /// Reveal a message from `.png`s by using delta of initial & modified pictures
    DeltaReveal {
        #[command(flatten)]
        save: SaveArg,
        
        #[command(flatten)]
        arg: DeltaArg,
    },
    
    #[command(name = "png-avg-sum-hide", aliases = &["png-sumh", "png-sum+"])]
    /// Hide a message into `.png`'s by using avg sum of initial images (modified will be created)
    AvgSumHide {
        #[arg(long)]
        /// Message that is transmitted (if it is a file, add preffix: `file:`). 
        /// It's better if the message encrypted before steganography.
        msg: Msg,

        #[arg(long = "chunk-bits", default_value_t = 4)]
        /// Bits per chunk.
        /// Max value is 6.
        bits_per_chunk: u8,

        #[arg(short, long, default_value_t = 8)]
        /// Size of a chunk.
        /// Max value is 64.
        chunk_size: u8,

        // TODO: strategy : rand / the same / etc
    },

    #[command(name = "png-avg-sum-reveal", aliases = &["png-sumr", "png-sum="])]
    /// Reveal a message from `.png`'s by using avg sum of modfied images
    AvgSumReveal {
        #[command(flatten)]
        save: SaveArg,
    },
    
    #[command(name = "png-less-hide", aliases = &["png-lh", "png-l+"])]
    /// Hide a message into `.png`'s by using less significant bits of pixel channels in initial images (modified will be created)
    LessSignificantHide {
        #[arg(long)]
        /// Message that is transmitted (if it is a file, add preffix: `file:`). 
        /// It's better if the message encrypted before steganography.
        msg: Msg,

        #[arg(long = "bits", default_value_t = 4)]
        /// Hiden bits per channel / pixel(in grayscale mode).
        /// Max value is 4.
        bits: u8,

        #[arg(long)]
        /// Make image grayscaled (density is 3 times less).
        gray: bool,
    },
    
    #[command(name = "png-less-reveal", aliases = &["png-lr", "png-l="])]
    /// Reveal a message into `.png`'s by using less significant bits of pixel channels
    LessSignificantReveal {
        #[command(flatten)]
        save: SaveArg,
    },
}

#[derive(Parser, Debug)]
pub struct DeltaArg {
    #[arg(long = "bits", default_value_t = 1)]
    /// Bits per pixel channel. (Preferably 1 or 2). 
    /// Not allowed to be more than 4.
    pub bits_per_pixel_chan: u8,
}

#[derive(Parser, Debug)]
pub struct SaveArg {
    #[arg(long)]
    /// If message is not a text, but is a file, then it will be saved to this path. 
    /// Otherwise into default: `file.bin`
    save: Option<String>,
}
impl SaveArg {
    pub fn clone_inner(&self) -> Option<String> {
        self.save.clone()
    }
}

#[derive(Parser, Debug)]
pub struct Info {
    #[arg(long = "init", value_delimiter = ',')]
    /// Paths of initial .png
    pub initial_img: Option<Vec<String>>,
    
    #[arg(long = "mod", value_delimiter = ',')]
    /// Paths of modified .png
    pub modified_img: Option<Vec<String>>,
}
impl Cli {
    fn check_init_img(initial_img: Option<Vec<String>>) -> Result<Vec<String>> {
        let Some(initial_img) = initial_img else {
            return Err(Error::EmptyInit)
        };
        if initial_img.is_empty() {
            return Err(Error::EmptyInit)
        };
        Ok(initial_img)
    }

    fn check_mod_img(modified_img: Option<Vec<String>>) -> Result<Vec<String>> {
        let Some(modified_img) = modified_img else {
            return Err(Error::EmptyModified)
        };
        Ok(modified_img)
    }

    fn check_consist_len(initial_img: &Vec<String>, modified_img: &Option<Vec<String>>) -> Result<()> {
        if let Some(modified_img) = modified_img {
            if modified_img.len() != initial_img.len() {
                return Err(Error::InconsistModLen)
            }
        }
        Ok(())
    }
}

impl TryFrom<PicArgs> for algo::DeltaHider {
    type Error = steganography::Error;

    fn try_from(args: PicArgs) -> Result<Self> {
        if let PicCmd::DeltaHide { msg, arg }  = args.cmd {
            let (msg, ty) = msg.into_pair()?;
            let initial_img = Cli::check_init_img(args.info.initial_img)?;
            Cli::check_consist_len(&initial_img, &args.info.modified_img)?;

            let bits = arg.bits_per_pixel_chan.max(1);
            if arg.bits_per_pixel_chan > MAX_BIT_PER_CHAN {
                return Err(Error::TooBigDelta(arg.bits_per_pixel_chan))
            }

            Ok(Self {
                msg,
                initial_img,
                modified_img: ImgPaths::new_any(args.info.modified_img),
                bits,
                ty,
            })
        } else {
            Err(Error::Other("Cannot convert into args: command is not a delta hide.".into()))
        }
    }
}

impl TryFrom<PicArgs> for algo::AvgSumHider {
    type Error = steganography::Error;

    fn try_from(args: PicArgs) -> Result<Self> {
        if let PicCmd::AvgSumHide { msg, bits_per_chunk, chunk_size } = args.cmd {
            let (msg, ty) = msg.into_pair()?;
            let initial_img = Cli::check_init_img(args.info.initial_img)?;
            Cli::check_consist_len(&initial_img, &args.info.modified_img)?;

            Ok(Self {
                msg,
                initial_img,
                modified_img: ImgPaths::new_any(args.info.modified_img),
                bits_per_chunk,
                chunk_size,
                ty,
            })
        } else {
            Err(Error::Other("Cannot convert into args: command is not an avg sum hide.".into()))
        }
    }
}

impl TryFrom<PicArgs> for algo::DeltaRevealer {
    type Error = steganography::Error;

    fn try_from(args: PicArgs) -> Result<Self> {
        if let PicCmd::DeltaReveal { save, arg } = args.cmd {
            let initial_img = Cli::check_init_img(args.info.initial_img)?;
            Cli::check_consist_len(&initial_img, &args.info.modified_img)?;
            let modified_img = Cli::check_mod_img(args.info.modified_img)?;
            
            Ok(Self {
                initial_img,
                modified_img,
                save_path: save.save,
                bits: arg.bits_per_pixel_chan,
            })
        } else {
            Err(Error::Other("Cannot convert into args: command is not a delta reveal.".into()))
        }
    }
}

impl TryFrom<PicArgs> for algo::AvgSumRevealer {
    type Error = steganography::Error;

    fn try_from(args: PicArgs) -> Result<Self> {
        if let PicCmd::AvgSumReveal { save } = args.cmd {
            let modified_img = Cli::check_mod_img(args.info.modified_img)?;

            Ok(Self {
                modified_img,
                save_path: save.save,
            })
        } else {
            Err(Error::Other("Cannot convert into args: command is not an avg sum reveal.".into()))
        }
    }
}

impl TryFrom<PicArgs> for algo::LessSignHider<Vec<u8>> {
    type Error = steganography::Error;

    fn try_from(args: PicArgs) -> Result<Self> {
        if let PicCmd::LessSignificantHide { msg, bits, gray } = args.cmd {
            let (msg, ty) = msg.into_pair()?;
            let initial_img = Cli::check_init_img(args.info.initial_img)?;
            Cli::check_consist_len(&initial_img, &args.info.modified_img)?;

            Ok(Self {
                msg,
                ty,
                initial_img,
                modified_img: ImgPaths::new_any(args.info.modified_img),
                bits,
                gray,
            })
        } else {
            Err(Error::Other("Cannot convert into args: command is not a less significant hide.".into()))
        }
    }
}

impl TryFrom<PicArgs> for algo::LessSignRevealer {
    type Error = steganography::Error;

    fn try_from(args: PicArgs) -> Result<Self> {
        if let PicCmd::LessSignificantReveal { save } = args.cmd {
            let modified_img = Cli::check_mod_img(args.info.modified_img)?;

            Ok(Self {
                modified_img,
                save_path: save.save,
            })
        } else {
            Err(Error::Other("Cannot convert into args: command is not a less significant reveal.".into()))
        }
    }
}

impl TryFrom<TxtArgs> for steganography::text::RepeatCharHider<'static, RepeatConstTypo> {
    type Error = steganography::Error;

    fn try_from(args: TxtArgs) -> Result<Self> {
        if let TxtCmd::RepeatHide { msg, bit_freq, typo_a, typo_b, modified: _ } = args.cmd {
            Ok(Self {
                initial: Cow::from(args.init.into_string()?),
                bit_freq,
                msg: Cow::Owned(msg.into_bytes()?),
                typo: RepeatConstTypo::new(typo_a, typo_b),
            })
        } else {
            Err(Error::Other("Cannot convert into args: command is not a txt repeat hide.".into()))
        }
    }
}

impl TryFrom<TxtArgs> for steganography::text::RepeatCharRevealer<'static> {
    type Error = steganography::Error;

    fn try_from(args: TxtArgs) -> Result<Self> {
        if let TxtCmd::RepeatReveal { modified, bit_freq, .. } = args.cmd {
            Ok(Self {
                initial: Cow::from(args.init.into_string()?),
                modified: Cow::from(modified.into_string()?),
                bit_freq,
                with_header: false,
            })
        } else {
            Err(Error::Other("Cannot convert into args: command is not a txt repeat reveal.".into()))
        }
    }
}