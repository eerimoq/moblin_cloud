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
