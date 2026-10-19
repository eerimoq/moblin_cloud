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
pub mod tests {
    use image::{ImageFormat, Rgb, RgbImage};

    use super::*;

    pub fn png(width: u32, height: u32) -> Vec<u8> {
        let image = RgbImage::from_fn(width, height, |x, _| {
            if x < width / 2 {
                Rgb([255, 0, 0])
            } else {
                Rgb([0, 0, 255])
            }
        });
        let mut png = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
            .unwrap();
        png
    }

    fn decode(jpeg: &[u8]) -> RgbImage {
        let reader = ImageReader::new(Cursor::new(jpeg))
            .with_guessed_format()
            .unwrap();
        assert_eq!(reader.format(), Some(ImageFormat::Jpeg));
        reader.decode().unwrap().to_rgb8()
    }

    #[test]
    fn wide_images_are_scaled() {
        let banner = decode(&banner(&png(3000, 1000)).unwrap());
        assert_eq!(banner.dimensions(), (1500, 500));
        assert!(banner.get_pixel(10, 250)[0] > 200);
        assert!(banner.get_pixel(1490, 250)[2] > 200);
    }

    #[test]
    fn other_shapes_are_cropped() {
        let tall = decode(&banner(&png(800, 1200)).unwrap());
        assert_eq!(tall.dimensions(), (1500, 500));
        let wide = decode(&banner(&png(3900, 600)).unwrap());
        assert_eq!(wide.dimensions(), (1500, 500));
        assert!(wide.get_pixel(10, 250)[0] > 200);
        assert!(wide.get_pixel(1490, 250)[2] > 200);
    }

    #[test]
    fn bad_images_are_refused() {
        assert_eq!(banner(b"not an image"), None);
        assert_eq!(banner(&png(300, 100)), None);
        assert_eq!(banner(&png(4001, 1000)), None);
        assert_eq!(banner(&png(1000, 4001)), None);
        let mut cut = png(1200, 400);
        cut.truncate(cut.len() / 2);
        assert_eq!(banner(&cut), None);
    }
}
