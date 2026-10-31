use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use image::RgbImage;
use image::imageops::FilterType;

pub fn decode(base64: &str) -> Option<RgbImage> {
    let bytes = BASE64_STANDARD.decode(base64.trim()).ok()?;
    Some(image::load_from_memory(&bytes).ok()?.to_rgb8())
}

pub struct Cells {
    pub width: u16,
    pub rows: Vec<Vec<([u8; 3], [u8; 3])>>,
}

pub fn cells(image: &RgbImage, width: u16, height: u16) -> Option<Cells> {
    if width == 0 || height == 0 || image.width() == 0 || image.height() == 0 {
        return None;
    }
    let (pixels_wide, pixels_high) = (u32::from(width), u32::from(height) * 2);
    let scale = f64::min(
        f64::from(pixels_wide) / f64::from(image.width()),
        f64::from(pixels_high) / f64::from(image.height()),
    );
    let scaled_width = ((f64::from(image.width()) * scale) as u32).clamp(1, pixels_wide);
    let scaled_height = ((f64::from(image.height()) * scale) as u32).clamp(2, pixels_high) & !1;
    let scaled = image::imageops::resize(image, scaled_width, scaled_height, FilterType::Triangle);
    let rows = (0..scaled_height / 2)
        .map(|row| {
            (0..scaled_width)
                .map(|column| {
                    (
                        scaled.get_pixel(column, row * 2).0,
                        scaled.get_pixel(column, row * 2 + 1).0,
                    )
                })
                .collect()
        })
        .collect();
    Some(Cells {
        width: scaled_width as u16,
        rows,
    })
}

#[cfg(test)]
mod tests {
    use image::Rgb;

    use super::*;

    #[test]
    fn images_become_half_block_cells() {
        let image = RgbImage::from_fn(4, 4, |_, y| {
            if y < 2 {
                Rgb([255, 0, 0])
            } else {
                Rgb([0, 0, 255])
            }
        });
        let split = cells(&image, 2, 1).unwrap();
        assert_eq!(split.width, 2);
        assert_eq!(split.rows.len(), 1);
        assert!(
            split.rows[0]
                .iter()
                .all(|(top, bottom)| top[0] > top[2] && bottom[2] > bottom[0])
        );
        let wide = RgbImage::from_pixel(8, 2, Rgb([0, 255, 0]));
        let fitted = cells(&wide, 4, 10).unwrap();
        assert_eq!((fitted.width, fitted.rows.len()), (4, 1));
        let mut png = Vec::new();
        image
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let decoded = decode(&BASE64_STANDARD.encode(png)).unwrap();
        assert_eq!(decoded.get_pixel(0, 3).0, [0, 0, 255]);
        assert!(decode("not an image").is_none());
    }
}
