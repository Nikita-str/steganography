use std::{borrow::Cow, path::Path};
use image::{ImageReader, RgbImage};
use crate::{Error, Result};

pub struct Img<AsPath: AsRef<Path>> {
    pub img: RgbImage,
    pub path: AsPath,
}
impl<AsPath: AsRef<Path>> Img<AsPath> {
    pub fn open_img(path: AsPath) -> Self {
        let img = ImageReader::open(&path)
            .expect("expected file")
            .decode()
            .expect("expected valid img")
            .into_rgb8();

        Self {
            img,
            path,
        }
    }

    pub fn save_img_by_path(&self, path: &str) -> Result<()> {
        let save_result = self.img.save_with_format(&path, image::ImageFormat::Png);
        save_result.map_err(|err|Error::ImageSave(Box::new(err)))
    }
    pub fn save_img(&self, paths: &ImgPaths, index: usize) -> Result<()> {
        let path = paths.modified_path(index, &self.path)?;
        self.save_img_by_path(&path)
    }
    pub fn width(&self) -> u32 {
        self.img.width()
    }
    pub fn height(&self) -> u32 {
        self.img.height()
    }
}

pub struct ImgPaths {
    paths: Vec<String>,
}
impl ImgPaths {
    pub fn new(imgs: Vec<String>) -> Self {
        Self { paths: imgs }
    }
    pub fn new_empty() -> Self {
        Self { paths: Vec::new() }
    }
    pub fn new_any(imgs: Option<Vec<String>>) -> Self {
        Self { paths: imgs.unwrap_or_default() }
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    pub fn modified_path(&self, index: usize, img_path: impl AsRef<Path>) -> Result<Cow<'_, str>> {
        if let Some(path) = self.paths.get(index) {
            return Ok(Cow::Borrowed(path))
        }

        if img_path.as_ref().file_name().is_none() {
            return Err(Error::PathIsNotAFile(img_path.as_ref().to_path_buf()))
        };

        let path = img_path.as_ref().to_str().unwrap();
        let mut path = path.strip_suffix(".png").unwrap_or(&path).to_string();
        path.push_str("_mod.png");

        Ok(Cow::Owned(path))
    }    
}
