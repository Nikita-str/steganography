mod cli;
use clap::Parser;
use cli::{Cli, CliCmd, PicCmd};
use steganography::prelude::*;
use steganography::png::algo as algo;
use steganography::text::{RepeatCharHider, RepeatCharRevealer, RepeatConstTypo};

fn main() {
    let start = std::time::Instant::now();
    
    if let Err(err) = main_inner() {
        println!("Error: {err}")
    } else {
        println!("\nDuration: {}ms", start.elapsed().as_millis());
    }
}

fn main_inner() -> Result<()> {
    let cli = Cli::parse();

    match cli.cmd {
        CliCmd::PicCmd(args) => match args.cmd {
            PicCmd::DeltaHide { .. } => {
                let algo: algo::DeltaHider = args.try_into()?;
                algo.hide()?;
            }

            PicCmd::DeltaReveal { .. } => {
                let algo: algo::DeltaRevealer = args.try_into()?;
                let (msg, ty) = algo.reveal()?;
                ty.do_action(msg, algo.save_path)?
            }
            
            PicCmd::AvgSumHide { .. } => {
                let algo: algo::AvgSumHider = args.try_into()?;
                algo.hide()?;
            }
            
            PicCmd::AvgSumReveal { .. } => {
                let algo: algo::AvgSumRevealer = args.try_into()?;
                let (msg, ty) = algo.reveal()?;
                ty.do_action(msg, algo.save_path)?
            }
            
            PicCmd::LessSignificantHide { .. } => {
                let algo: algo::LessSignHider<Vec<u8>> = args.try_into()?;
                algo.transmute_msg().hide()?;
            }
            
            PicCmd::LessSignificantReveal { .. } => {
                let algo: algo::LessSignRevealer = args.try_into()?;
                let (msg, ty) = algo.reveal()?;
                ty.do_action(msg, algo.save_path)?
            }
        }
        CliCmd::TxtCmd(args) => match &args.cmd {
            cli::TxtCmd::RepeatHide { modified, msg, .. } => {
                let ty = msg.ty();
                let save = modified.clone();
                let algo: RepeatCharHider<RepeatConstTypo> = args.try_into()?;
                let hidden = algo.hide()?;
                ty.do_action(hidden.into_bytes(), save)?;
            }
            cli::TxtCmd::RepeatReveal { save, .. } => {
                let save = save.clone_inner();
                let algo: RepeatCharRevealer = args.try_into()?;
                let output = algo.reveal()?;

                MsgType::File.do_action(output, save)?;
            }
        }
    }
    Ok(())
}
