use crate::capture;
use crate::inference::pre_process::{pre_process, raw_to_img, to_gray, uint8_raw_to_img};
use crate::info::info::ScanInfo;
use anyhow::{anyhow, Result};
use image::{GrayImage, ImageBuffer, ImageResult, RgbImage};
use log::info;
use std::time::SystemTime;

pub mod color;
pub mod utils;

use color::Color;

#[derive(Clone, Debug)]
pub struct PixelRect {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
}

impl PixelRect {
    pub fn scale(&mut self, ratio: f64) {
        self.left = (self.left as f64 * ratio).round() as i32;
        self.top = (self.top as f64 * ratio).round() as i32;
        self.width = (self.width as f64 * ratio).round() as i32;
        self.height = (self.height as f64 * ratio).round() as i32;
    }
    pub fn shifted(rect: &PixelRect, shift_x: i32, shift_y: i32) -> PixelRect {
        PixelRect {
            left: rect.left + shift_x,
            top: rect.top + shift_y,
            width: rect.width,
            height: rect.height,
        }
    }
    pub fn to_bound(&self) -> PixelRectBound {
        PixelRectBound {
            left: self.left,
            top: self.top,
            right: self.left + self.width,
            bottom: self.top + self.height,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PixelRectBound {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl PixelRectBound {
    pub fn capture_absolute(&self) -> Result<RawImage> {
        let w = self.right - self.left;
        let h = self.bottom - self.top;
        let rect = PixelRect {
            left: self.left,
            top: self.top,
            width: w,
            height: h,
        };
        let raw_u8 = capture::capture_absolute(&rect)?;
        let raw_gray = to_gray(raw_u8, w as u32, h as u32);
        let raw_after_pp = pre_process(raw_gray);
        Ok(raw_after_pp)
    }

    pub fn capture_relative(&self, info: &ScanInfo) -> Result<RawImage> {
        let w = self.right - self.left;
        let h = self.bottom - self.top;
        let rect = PixelRect {
            left: self.left + info.left as i32,
            top: self.top + info.top as i32,
            width: w,
            height: h,
        };
        let now = SystemTime::now();
        let raw_u8 = capture::capture_absolute(&rect)?;
        info!("capture raw time: {}ms", now.elapsed()?.as_millis());
        let raw_gray = to_gray(raw_u8, w as u32, h as u32);
        let raw_after_pp = pre_process(raw_gray);
        info!("preprocess time: {}ms", now.elapsed()?.as_millis());
        Ok(raw_after_pp)
    }

    pub fn capture_relative_image(&self, info: &ScanInfo) -> Result<RgbImage> {
        let w = self.right - self.left;
        let h = self.bottom - self.top;
        let rect = PixelRect {
            left: self.left + info.left as i32,
            top: self.top + info.top as i32,
            width: w,
            height: h,
        };

        capture::capture_absolute_image(&rect)
    }
}

pub struct RawImage {
    pub data: Vec<f32>,
    pub w: u32,
    pub h: u32,
}

pub struct RawCaptureImage {
    pub data: Vec<u8>,
    pub w: u32,
    pub h: u32,
}

impl RawImage {
    pub fn to_gray_image(&self) -> GrayImage {
        raw_to_img(&self)
    }

    pub fn grayscale_to_gray_image(&self) -> GrayImage {
        uint8_raw_to_img(&self)
    }
}

impl RawCaptureImage {
    pub fn save(&self, path: &str) -> ImageResult<()> {
        let data = &self.data;

        let img = ImageBuffer::from_fn(self.w, self.h, |x, y| {
            let p = ((self.h - 1 - y) * self.w + x) as usize * 4;
            image::Rgb([data[p + 2], data[p + 1], data[p]])
            // image::Luma([pixel])
        });

        img.save(path)
    }
    pub fn crop_to_raw_img(&self, rect: &PixelRect) -> RawImage {
        // let now = SystemTime::now();
        let vol = rect.width * rect.height;
        let mut data = vec![0.0; vol as usize];
        for i in rect.left..rect.left + rect.width {
            for j in rect.top..rect.top + rect.height {
                let x = i;
                let y = self.h as i32 - j - 1;
                let b: u8 = self.data[((y * self.w as i32 + x) * 4) as usize];
                let g: u8 = self.data[((y * self.w as i32 + x) * 4 + 1) as usize];
                let r: u8 = self.data[((y * self.w as i32 + x) * 4 + 2) as usize];

                let gray = r as f32 * 0.2989 + g as f32 * 0.5870 + b as f32 * 0.1140;
                let new_index = ((j - rect.top) * rect.width + i - rect.left) as usize;
                data[new_index] = gray;
            }
        }

        let im = RawImage {
            data,
            w: rect.width as u32,
            h: rect.height as u32,
        };
        // let im = pre_process(im);
        // No preprocess!

        // info!("preprocess time: {}ms", now.elapsed().unwrap().as_millis());
        // im.to_gray_image().save("test.png");
        im
    }
    pub fn get_color(&self, x: u32, y: u32) -> Result<Color> {
        if x >= self.w || y >= self.h {
            return Err(anyhow!("Pixel coord out of bounds"));
        }
        let p = ((self.h - 1 - y) * self.w + x) as usize * 4;
        Ok(Color(self.data[p + 2], self.data[p + 1], self.data[p]))
    }
    pub fn set_color(&mut self, x: u32, y: u32, color: &Color) -> Result<()> {
        if x >= self.w || y >= self.h {
            return Err(anyhow!("Pixel coord out of bounds"));
        }
        let p = ((self.h - 1 - y) * self.w + x) as usize * 4;
        self.data[p + 0] = color.2;
        self.data[p + 1] = color.1;
        self.data[p + 2] = color.0;
        Ok(())
    }
    pub fn mark(&mut self, rect: &PixelRect, color: &Color, alpha: f64) -> Result<()> {
        if rect.width < 0
            || rect.height < 0
            || rect.left < 0
            || rect.top < 0
            || rect.left + rect.width >= self.w as i32
            || rect.top + rect.height >= self.h as i32
        {
            return Err(anyhow!("Invalid marking area"));
        }
        for i in 0..rect.width {
            for j in 0..rect.height {
                let x = rect.left as u32 + i as u32;
                let y = rect.top as u32 + j as u32;
                let p = ((self.h - 1 - y) * self.w + x) as usize * 4;
                self.data[p + 0] =
                    (self.data[p + 0] as f64 * (1.0 - alpha) + color.2 as f64 * alpha) as u8;
                self.data[p + 1] =
                    (self.data[p + 1] as f64 * (1.0 - alpha) + color.1 as f64 * alpha) as u8;
                self.data[p + 2] =
                    (self.data[p + 2] as f64 * (1.0 - alpha) + color.0 as f64 * alpha) as u8;
            }
        }
        Ok(())
    }
}

// pub struct
