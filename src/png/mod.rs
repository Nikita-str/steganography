mod writer;
mod reader;

pub mod algo;
pub mod img;

pub use img::{Img, ImgPaths};

pub mod prelude {
    pub use super::{Img, ImgPaths};
}