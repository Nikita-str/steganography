mod cli;
use clap::Parser;
use cli::{Cli, CliCmd, PicCmd};
use steganography::prelude::*;
use steganography::png::algo as algo;
use steganography::text::{RepeatCharHider, RepeatCharRevealer, RepeatConstTypo};

fn main() {
    let start = std::time::Instant::now();
    
    let cli = Cli::parse();
    if main_inner_err_handle(cli).is_ok() {
        println!("\nDuration: {}ms", start.elapsed().as_millis());
    }
}

fn main_inner_err_handle(cli: Cli) -> Result<()> {
    if let Err(err) = main_inner(cli) {
        println!("Error: {err}");
        return Err(err)
    }
    Ok(())
}

fn main_inner(cli: Cli) -> Result<()> {
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


#[cfg(test)]
mod tests {
    use super::*;
    const PROG_NAME: &str = "steganography";

    #[test]
    fn test_txt_cli() -> Result<()> {
        let freq = "71";

        // HIDE:
        let msg_file = r#"file:tests\txt\Balmont_and_Blake.txt"#;
        let msg_hidden_file = r#"tests\txt\gen_Dostoevsky_Idiot_hidden.txt"#;

        let args = vec![
            PROG_NAME,
            "txt",
            "--init",
            r#"file:tests\txt\Dostoevsky_Idiot.txt"#,
            "txt-rh",
            "--mod",
            msg_hidden_file,
            "--msg",
            msg_file,
            "--freq",
            freq,
        ];
        
        let msg_send = std::fs::read_to_string(msg_file.split_once("file:").unwrap().1)?;
        
        let cli = Cli::parse_from(args);
        main_inner_err_handle(cli)?;

        // REVEAL:
        let msg_hidden_file = "file:".to_owned() + msg_hidden_file;
        let msg_reveal_file = r#"tests\txt\gen_Balmont_and_Blake_reveal.txt"#;

        let args = vec![
            PROG_NAME,
            "txt",
            "--init",
            r#"file:tests\txt\Dostoevsky_Idiot.txt"#,
            "txt-rr",
            "--mod",
            &msg_hidden_file,
            "--freq",
            freq,
            "--save",
            msg_reveal_file,
        ];

        let cli = Cli::parse_from(args);
        main_inner_err_handle(cli)?;

        let msg_received = std::fs::read_to_string(msg_reveal_file)?;

        // ASSERT CORRECTNESS:
        assert_eq!(msg_send, msg_received);

        Ok(())
    }
}