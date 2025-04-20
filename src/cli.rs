use clap::{Parser, Subcommand};

use steganography::prelude::*;
use steganography::png::algo_args as args;
use steganography::png::prelude::*;

// TODO: text Cli
// TODO: text file
// TODO: text in code comment

// TODO: chacha8rng for noise 

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
    #[command(name = "png-delta-hide", aliases = &["png-dh", "png-d+"])]
    /// Hide a message into `.png`s by using delta of initial & modified(will be created) pictures
    DeltaHide {
        #[arg(long)]
        /// Message that is transmitted. 
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
        /// Message that is transmitted. 
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

impl TryFrom<Cli> for args::DeltaHideArgs {
    type Error = steganography::Error;

    fn try_from(cli: Cli) -> Result<Self> {
        if let CliCmd::DeltaHide { msg, arg } = cli.cmd {
            let (msg, ty) = msg.into_pair()?;
            let initial_img = Cli::check_init_img(cli.info.initial_img)?;
            Cli::check_consist_len(&initial_img, &cli.info.modified_img)?;

            let bits = arg.bits_per_pixel_chan.max(1);
            if arg.bits_per_pixel_chan > MAX_BIT_PER_CHAN {
                return Err(Error::TooBigDelta(arg.bits_per_pixel_chan))
            }

            Ok(Self {
                msg,
                initial_img,
                modified_img: ImgPaths::new_any(cli.info.modified_img),
                bits,
                ty,
            })
        } else {
            Err(Error::Other("Cannot convert into args: command is not a delta hide.".into()))
        }
    }
}

impl TryFrom<Cli> for args::AvgSumHideArgs {
    type Error = steganography::Error;

    fn try_from(cli: Cli) -> Result<Self> {
        if let CliCmd::AvgSumHide { msg, bits_per_chunk, chunk_size } = cli.cmd {
            let (msg, ty) = msg.into_pair()?;
            let initial_img = Cli::check_init_img(cli.info.initial_img)?;
            Cli::check_consist_len(&initial_img, &cli.info.modified_img)?;

            Ok(Self {
                msg,
                initial_img,
                modified_img: ImgPaths::new_any(cli.info.modified_img),
                bits_per_chunk,
                chunk_size,
                ty,
            })
        } else {
            Err(Error::Other("Cannot convert into args: command is not a delta hide.".into()))
        }
    }
}

impl TryFrom<Cli> for args::DeltaRevealArgs {
    type Error = steganography::Error;

    fn try_from(cli: Cli) -> Result<Self> {
        if let CliCmd::DeltaReveal { save, arg } = cli.cmd {
            let initial_img = Cli::check_init_img(cli.info.initial_img)?;
            Cli::check_consist_len(&initial_img, &cli.info.modified_img)?;
            let modified_img = Cli::check_mod_img(cli.info.modified_img)?;
            
            Ok(Self {
                initial_img,
                modified_img,
                save_path: save.save,
                bits: arg.bits_per_pixel_chan,
            })
        } else {
            Err(Error::Other("Cannot convert into args: command is not a delta hide.".into()))
        }
    }
}


impl TryFrom<Cli> for args::AvgSumRevealArgs {
    type Error = steganography::Error;

    fn try_from(cli: Cli) -> Result<Self> {
        if let CliCmd::AvgSumReveal { save } = cli.cmd {
            let modified_img = Cli::check_mod_img(cli.info.modified_img)?;

            Ok(Self {
                modified_img,
                save_path: save.save,
            })
        } else {
            Err(Error::Other("Cannot convert into args: command is not a delta hide.".into()))
        }
    }
}
