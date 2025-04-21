use crate::{Result, Error};
use std::ffi::OsString;

#[derive(Debug, Clone)]
pub enum Msg {
    /// The text of a message itself.
    Txt(String),
    /// Path to messaged file.
    File(String),
}
impl Msg {
    pub fn ty(&self) -> MsgType {
        match self {
            Msg::Txt(_) => MsgType::Txt,
            Msg::File(_) => MsgType::File,
        }
    }

    pub fn into_bytes(self) -> Result<Vec<u8>> {
        match self {
            Msg::Txt(msg) => Ok(msg.into_bytes()),
            Msg::File(path) => {
                // TODO: we can read it chunked/buffered, to read files of any length (more than RAM allow) 
                Ok(std::fs::read(path)?)
            }
        }
    }
    
    pub fn into_string(self) -> Result<String> {
        match self {
            Msg::Txt(s) => Ok(s),
            Msg::File(path) => {
                let file_bytes = std::fs::read(path)?;
                match String::from_utf8(file_bytes) {
                    Ok(s) => Ok(s),
                    Err(err) => Err(Error::InvalidMsg(Box::new(err))),
                }
            }
        }
    }

    pub fn into_pair(self) -> Result<(Vec<u8>, MsgType)> {
        let ty = self.ty();
        self.into_bytes().map(|x|(x, ty))
    }
}

impl From<OsString> for Msg {
    fn from(value: OsString) -> Self {
        let Some(str) = value.to_str() else {
            panic!("bad OS string (not a valid UTF-8): {value:?}")
        };

        const FILE_PREFIX: &str = "file:";
        if str.starts_with(FILE_PREFIX) {
            Self::File(str[FILE_PREFIX.len()..].to_string())
        } else {
            Self::Txt(str.to_string())
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy)]
#[must_use]
pub enum MsgType {
    Txt = 1,
    File = 2,
    ReservedPre = 254,
    Reserved = 255,
}
impl MsgType {
    pub fn is_reserved(self) -> bool {
        match self {
            MsgType::ReservedPre => true,
            MsgType::Reserved => true,
            _ => false,
        }
    }

    pub fn try_from_u8(byte: u8) -> Option<Self> {
        Some(match byte {
            1 => Self::Txt,
            2 => Self::File,
            _ => return None,
        })
    }

    pub fn do_action(self, msg: Vec<u8>, save: Option<String>) -> Result<()> {
        match self {
            MsgType::Txt => {
                let msg = String::from_utf8(msg)
                    .map_err(|err|Error::InvalidMsg(Box::new(err)))?;
                println!("msg: \"{msg}\"");
            }
            MsgType::File => {
                let save_path = save.unwrap_or("file.bin".to_string());
                if let Err(err) = std::fs::write(&save_path, msg) {
                    return Err(Error::SaveProblem(err, save_path))
                }
                println!("Done!\nfile saved into \"{save_path}\"");
            }
            _ => {
                return Err(Error::InvalidMsgTypeByte(self as u8));      
            }
        }
        Ok(())
    }
}
