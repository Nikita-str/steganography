mod cli;
use cli::{Cli, CliCmd};
use steganography::prelude::*;
use steganography::png::prelude::*;
use steganography::reader::ConstBytesReader;

fn main() {
    let start = std::time::Instant::now();
    
    if let Err(err) = main_inner() {
        println!("Error: {err}")
    } else {
        println!("\nDuration: {}ms", start.elapsed().as_millis());
    }
}

fn main_inner() -> Result<()> {
    let cli = Cli::cli()?;
    let (mut cli, cmd) = (cli.info, cli.cmd);

    match cmd {
        CliCmd::DeltaHide { msg } => {
            let ty = msg.ty();
            cli.delta_hide_bytes(msg.into_bytes()?, ty)?;
        }

        CliCmd::DeltaReveal { save } => {
            let modified_img = cli.take_modified()?;
            let mut msg_reader = DeltaByteMsgReader::new(cli.bits_per_pixel_chan);
            
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

            let Some(ty) = msg_reader.ty() else {
                return Err(Error::UnreadedHeader);
            };
            let Some(msg) = msg_reader.take_msg() else {
                return Err(Error::UnreadedHeader);
            };

            ty.do_action(msg, save)?;
        }

        CliCmd::AvgSumHide { msg, bits_per_chunk, chunk_size } => {
            let ty = msg.ty();
            let msg_bytes = msg.into_bytes()?;
            cli.avg_sum_hide_bytes(msg_bytes, bits_per_chunk, chunk_size, ty)?;
        }

        CliCmd::AvgSumReveal { save } => {
            let modified_img = cli.take_modified()?;

            const HEADER_SIZE: usize = 7;
            const HEADER_CHUNK_SIZE: u8 = 8;
            const HEADER_BITS_PER_CHUNK: u8 = 4;

            let mut chunk_size = HEADER_CHUNK_SIZE;
            let mut bits_per_chunk = HEADER_BITS_PER_CHUNK;
            let mut rem = 1u16 << bits_per_chunk;
            let mut sum;
            let mut header_reader = ConstBytesReader::new(bits_per_chunk);
            let mut header = Vec::with_capacity(HEADER_SIZE);
            let mut ty = MsgType::Reserved;
            let mut msg_len = 0;
            
            let mut msg_reader: Option<(ConstBytesReader, Vec<u8>)> = None;

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
                    match MsgType::try_from_u8(header[0]) {
                        Some(ty_x) => ty = ty_x,
                        _ => return Err(Error::InvalidMsgTypeByte(header[0])),
                    };
                    bits_per_chunk = header[1];
                    rem = 1u16 << bits_per_chunk;
                    chunk_size = header[2];
                    let len_bytes: [u8; 4] = header[HEADER_SIZE - 4..HEADER_SIZE].try_into().unwrap();
                    msg_len = u32::from_le_bytes(len_bytes) as usize;
                    
                    let msg = Vec::with_capacity(msg_len);
                    msg_reader = Some((ConstBytesReader::new(bits_per_chunk), msg));
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
