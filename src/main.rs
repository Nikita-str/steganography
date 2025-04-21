mod cli;
use clap::Parser;
use cli::{Cli, CliCmd};
use steganography::prelude::*;
use steganography::png::algo as algo;

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

    match &cli.cmd {
        CliCmd::DeltaHide { .. } => {
            let algo: algo::DeltaHider = cli.try_into()?;
            algo.hide()?;
        }

        CliCmd::DeltaReveal { .. } => {
            let algo: algo::DeltaRevealer = cli.try_into()?;
            let (msg, ty) = algo.reveal()?;
            ty.do_action(msg, algo.save_path)?
        }
        
        CliCmd::AvgSumHide { .. } => {
            let algo: algo::AvgSumHider = cli.try_into()?;
            algo.hide()?;
        }
        
        CliCmd::AvgSumReveal { .. } => {
            let algo: algo::AvgSumRevealer = cli.try_into()?;
            let (msg, ty) = algo.reveal()?;
            ty.do_action(msg, algo.save_path)?
        }
        
        CliCmd::LessSignificantHide { .. } => {
            let algo: algo::LessSignHider<Vec<u8>> = cli.try_into()?;
            algo.transmute_msg().hide()?;
        }
        
        CliCmd::LessSignificantReveal { .. } => {
            let algo: algo::LessSignRevealer = cli.try_into()?;
            let (msg, ty) = algo.reveal()?;
            ty.do_action(msg, algo.save_path)?
        }
    }
    Ok(())
}
