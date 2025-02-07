use std::{
    cmp::{max, min},
    env,
};

use image::{DynamicImage, ImageOutputFormat};
use lazy_static::lazy_static;

lazy_static! {
    static ref MAX_HEIGHT: u32 = env::var("MAX_HEIGHT")
        .map(|h| h.parse().unwrap_or(1200))
        .unwrap_or(1200);
    static ref MAX_WIDTH: u32 = env::var("MAX_WIDTH")
        .map(|h| h.parse().unwrap_or(1200))
        .unwrap_or(1200);
    static ref ONLY_JPEG: bool = env::var("ONLY_JPEG")
        .map(|h| h.parse().unwrap_or(false))
        .unwrap_or(false);
    static ref JPEG_QUALITY: u8 = env::var("JPEG_QUALITY")
        .map(|h| h.parse().unwrap_or(87))
        .unwrap_or(87);
}
pub fn process_image(
    input: &[u8],
    format: &str,
    max_len: Option<u32>,
) -> Result<(Vec<u8>, String), image::ImageError> {
    let img = image::load_from_memory(input)?;
    let max_length = max_len.unwrap_or(max(*MAX_HEIGHT, *MAX_WIDTH));

    let (width, height) = calculate_dimensions(&img, max_length);

    // only resize if necessary
    let processed_img = if img.height() > height || img.width() > width {
        img.resize(width, height, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };

    let mut output = Vec::with_capacity(input.len());
    let format = get_image_format(format);
    processed_img.write_to(
        &mut std::io::Cursor::new(&mut output),
        format.clone()
    )?;

    Ok((output, get_string_from_image_format(format)))
}

#[inline]
fn calculate_dimensions(img: &DynamicImage, max_length: u32) -> (u32, u32) {
    let mut height = min(img.height(), *MAX_HEIGHT);
    let mut width = min(img.width(), *MAX_WIDTH);

    if img.height() > img.width() {
        height = min(height, max_length);
    } else {
        width = min(width, max_length);
    }

    (width, height)
}

#[inline]
fn get_image_format(format: &str) -> ImageOutputFormat {
    if *ONLY_JPEG {
        return ImageOutputFormat::Jpeg(*JPEG_QUALITY);
    };
    match format.to_lowercase().as_str() {
        "png" => ImageOutputFormat::Png,
        "webp" => ImageOutputFormat::WebP,
        "gif" => ImageOutputFormat::Gif,
        _ => ImageOutputFormat::Jpeg(*JPEG_QUALITY),
    }
}

fn get_string_from_image_format(format: ImageOutputFormat) -> String {
    match format {
        ImageOutputFormat::Png => "png".to_string(),
        ImageOutputFormat::Jpeg(_) => "jpeg".to_string(),
        ImageOutputFormat::WebP => "webp".to_string(),
        ImageOutputFormat::Gif => "gif".to_string(),
        ImageOutputFormat::Bmp => "bmp".to_string(),
        ImageOutputFormat::Ico => "ico".to_string(),
        ImageOutputFormat::Tiff => "tiff".to_string(),
        _ => "unknown".to_string()
    }
}
