mod cli;
use clap::Parser;
use cli::{Cli, CliCmd};
use steganography::prelude::*;
use steganography::png::algo_args as args;

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
            let args: args::DeltaHideArgs = cli.try_into()?;
            args.hide()?;
        }

        CliCmd::DeltaReveal { .. } => {
            let args: args::DeltaRevealArgs = cli.try_into()?;
            let (msg, ty) = args.reveal()?;
            ty.do_action(msg, args.save_path)?
        }
        
        CliCmd::AvgSumHide { .. } => {
            let args: args::AvgSumHideArgs = cli.try_into()?;
            args.hide()?;
        }
        
        CliCmd::AvgSumReveal { .. } => {
            let args: args::AvgSumRevealArgs = cli.try_into()?;
            let (msg, ty) = args.reveal()?;
            ty.do_action(msg, args.save_path)?
        }
        
        CliCmd::LessSignificantHide { .. } => {
            let args: args::RemainderHider<Vec<u8>> = cli.try_into()?;
            args.transmute_msg().hide()?;
        }
        
        CliCmd::LessSignificantReveal { .. } => {
            let args: args::RemainderRevealer = cli.try_into()?;
            let (msg, ty) = args.reveal()?;
            ty.do_action(msg, args.save_path)?
        }
    }
    Ok(())
}
