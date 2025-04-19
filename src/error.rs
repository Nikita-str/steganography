use thiserror::Error;
use image::ImageError;
use std::string::FromUtf8Error;
use crate::MAX_BIT_PER_CHAN;

#[derive(Error, Debug)]
#[must_use]
pub enum Error {
    #[error("Empty initial paths, use `--init` cli arg: `--init png_path_0 ... png_path_n`")]
    EmptyInit,
    #[error("Inconsistent lenght of modified paths, `--modt` arg should have same amount of paths as `--init`")]
    InconsistModLen,
    #[error("The delta {0} is too big, should be no more than {MAX_BIT_PER_CHAN}")]
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
    #[error("Unexpected byte({0}) of msg type")]
    InvalidMsgTypeByte(u8),
    #[error("Header was not readed (img too small)")]
    UnreadedHeader,
    #[error("Not enough size of initial images in total (need to hide {0} bytes more). To fix it add more images into `--init` arg.")]
    NotEnoughSizeOfInit(usize),
}
impl Error {
    pub fn test_too_big_msg(msg_len: usize) -> Result<()> {
        if msg_len > u32::MAX as usize {
            Err(Error::TooBigMsg)
        } else {
            Ok(())
        }
    }
}
impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::ErrorIO(err)
    }
}

pub type Result<T> = std::result::Result<T, Error>;