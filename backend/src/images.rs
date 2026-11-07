use std::io::Cursor;

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{ImageReader, Limits};

pub fn banner(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(4000);
    limits.max_image_height = Some(4000);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode().ok()?;
    if image.width() < 600 {
        return None;
    }
    let image = image
        .resize_to_fill(1500, 500, FilterType::Lanczos3)
        .to_rgb8();
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, 85)
        .encode_image(&image)
        .ok()?;
    Some(jpeg)
}

#[cfg(test)]
#[path = "tests/images.rs"]
pub mod tests;
