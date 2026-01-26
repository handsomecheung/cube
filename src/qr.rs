use anyhow::{anyhow, Result};

#[cfg(feature = "encode")]
use image::{imageops, Rgb, RgbImage};

#[cfg(any(feature = "decode", feature = "wasm"))]
use image::{DynamicImage, GrayImage};

#[cfg(feature = "encode")]
use qrcode::{Color, EcLevel, QrCode, Version};

#[cfg(any(feature = "decode", feature = "wasm"))]
use rqrr::PreparedImage;

#[cfg(any(feature = "encode", feature = "decode"))]
use std::path::Path;

#[cfg(feature = "encode")]
pub fn generate_qr_image(
    data: &[u8],
    specific_version: Option<Version>,
    pixel_scale: u32,
    halftone_path: Option<&Path>,
) -> Result<(RgbImage, Version)> {
    let ec_level = if halftone_path.is_some() {
        EcLevel::H
    } else {
        EcLevel::M
    };

    let code = if let Some(v) = specific_version {
        QrCode::with_version(data, v, ec_level)
            .map_err(|e| anyhow!("Failed to create QR code with specific version: {}", e))?
    } else {
        QrCode::with_error_correction_level(data, ec_level)
            .map_err(|e| anyhow!("Failed to create QR code: {}", e))?
    };

    let version = code.version();

    let qr_image = code
        .render::<Rgb<u8>>()
        .min_dimensions(200, 200)
        .quiet_zone(true)
        .module_dimensions(pixel_scale, pixel_scale)
        .build();

    if let Some(path) = halftone_path {
        let bg_img =
            image::open(path).map_err(|e| anyhow!("Failed to open halftone image: {}", e))?;
        let bg_img = bg_img.into_rgb8();

        let (w, h) = qr_image.dimensions();
        let mut bg_resized = imageops::resize(&bg_img, w, h, imageops::FilterType::CatmullRom);

        // Blend logic: specific dot shape (Halftone-like)
        // Instead of modifying every pixel, we only modify the center of each module.
        // This preserves the background structure in the "gaps" between modules.
        for (x, y, qr_pixel) in qr_image.enumerate_pixels() {
            let bg_pixel = bg_resized.get_pixel_mut(x, y);

            // Determine if we are in the "center" of a module
            // e.g., for scale 4, we want indices 1 and 2 (center 2x2) to be modified.
            // 0 and 3 (borders) remain background.
            let rel_x = x % pixel_scale;
            let rel_y = y % pixel_scale;

            // Calculate border size. Ensure we modify at least central pixel.
            // scale 4 -> border 1 -> center range [1, 3) -> 2 pixels
            // scale 3 -> border 1 -> center range [1, 2) -> 1 pixel
            let border = if pixel_scale > 2 { pixel_scale / 4 } else { 0 };

            let is_center = rel_x >= border
                && rel_x < (pixel_scale - border)
                && rel_y >= border
                && rel_y < (pixel_scale - border);

            if is_center {
                let is_dark = qr_pixel[0] < 128;

                if is_dark {
                    // Darken center strongly to ensure readability
                    // Blend 80% black
                    bg_pixel[0] = (bg_pixel[0] as f32 * 0.2) as u8;
                    bg_pixel[1] = (bg_pixel[1] as f32 * 0.2) as u8;
                    bg_pixel[2] = (bg_pixel[2] as f32 * 0.2) as u8;
                } else {
                    // Lighten center strongly
                    // Blend 80% white
                    bg_pixel[0] = (bg_pixel[0] as f32 + (255.0 - bg_pixel[0] as f32) * 0.8) as u8;
                    bg_pixel[1] = (bg_pixel[1] as f32 + (255.0 - bg_pixel[1] as f32) * 0.8) as u8;
                    bg_pixel[2] = (bg_pixel[2] as f32 + (255.0 - bg_pixel[2] as f32) * 0.8) as u8;
                }
            }
            // If not center, leave bg_pixel completely untouched!
        }

        return Ok((bg_resized, version));
    }

    Ok((qr_image, version))
}

#[cfg(feature = "encode")]
pub fn save_qr_image(image: &RgbImage, path: &Path) -> Result<()> {
    image.save(path)?;
    Ok(())
}

#[cfg(feature = "decode")]
pub fn decode_qr_image(path: &Path) -> Result<Vec<u8>> {
    let img = image::open(path)?;
    decode_qr_from_dynamic_image(&img)
}

#[cfg(any(feature = "decode", feature = "wasm"))]
pub fn decode_qr_from_dynamic_image(img: &DynamicImage) -> Result<Vec<u8>> {
    let gray = img.to_luma8();

    // Standard decode (rqrr uses adaptive thresholding, but might fail on noise)
    if let Ok(content) = decode_qr_from_gray(&gray) {
        return Ok(content);
    }

    // Explicit thresholding strategies.
    // Halftone QR codes use strong black/white dots on a mid-tone background.
    // Forcing a binary threshold can effectively remove the background noise.
    let thresholds = [80, 100, 128, 160, 200];

    for &t in &thresholds {
        let mut t_img = gray.clone();
        for p in t_img.pixels_mut() {
            // p.0 is [u8; 1]
            p.0[0] = if p.0[0] < t { 0 } else { 255 };
        }

        if let Ok(content) = decode_qr_from_gray(&t_img) {
            return Ok(content);
        }
    }

    Err(anyhow!(
        "No QR code found in image (tried standard and thresholding)"
    ))
}

#[cfg(any(feature = "decode", feature = "wasm"))]
pub fn decode_qr_from_gray(gray: &GrayImage) -> Result<Vec<u8>> {
    let mut prepared = PreparedImage::prepare(gray.clone());
    let grids = prepared.detect_grids();

    if grids.is_empty() {
        return Err(anyhow!("No QR code found in image"));
    }

    let (_, content) = grids[0]
        .decode()
        .map_err(|e| anyhow!("Failed to decode QR code: {:?}", e))?;

    Ok(content.into_bytes())
}

#[cfg(feature = "encode")]
pub fn render_qr_to_terminal(data: &[u8]) -> Result<String> {
    use terminal_size::{terminal_size, Height, Width};

    let code = QrCode::with_error_correction_level(data, EcLevel::M)
        .map_err(|e| anyhow!("Failed to create QR code: {}", e))?;

    let qr_size = code.width();
    let colors = code.to_colors();

    let (term_width, term_height) = terminal_size()
        .map(|(Width(w), Height(h))| (w as usize, h as usize))
        .unwrap_or((80, 24));

    let qr_with_quiet = qr_size + 4; // Add quiet zone

    // Fixed scale=1: each QR module = 1 char wide, uses half-blocks for height
    // This gives the most compact and square appearance
    let scale: usize = 1;

    let display_width = qr_with_quiet * scale;
    let display_height = (qr_with_quiet + 1) / 2 * scale;

    // Center padding
    let pad_left = term_width.saturating_sub(display_width) / 2;
    let pad_top = term_height.saturating_sub(display_height + 8) / 2;

    let mut result = String::new();
    let left_pad: String = " ".repeat(pad_left);

    // Top padding
    for _ in 0..pad_top {
        result.push('\n');
    }

    // Helper to check if a position is dark
    let is_dark = |row: usize, col: usize| -> bool {
        if row >= 2 && row < qr_size + 2 && col >= 2 && col < qr_size + 2 {
            let qr_y = row - 2;
            let qr_x = col - 2;
            colors[qr_y * qr_size + qr_x] == Color::Dark
        } else {
            false // Quiet zone is white
        }
    };

    // Render using half-block characters
    // Process 2 QR rows at a time, each becomes 1 terminal row (with scale repetition)
    for qr_row_pair in 0..((qr_with_quiet + 1) / 2) {
        let top_row = qr_row_pair * 2;
        let bottom_row = top_row + 1;

        // Repeat each output row 'scale' times for vertical scaling
        for _ in 0..scale {
            result.push_str(&left_pad);

            for qr_col in 0..qr_with_quiet {
                let top_dark = is_dark(top_row, qr_col);
                let bottom_dark = if bottom_row < qr_with_quiet {
                    is_dark(bottom_row, qr_col)
                } else {
                    false
                };

                let ch = match (top_dark, bottom_dark) {
                    (true, true) => '█',
                    (true, false) => '▀',
                    (false, true) => '▄',
                    (false, false) => ' ',
                };

                // Repeat char 'scale' times for horizontal scaling
                for _ in 0..scale {
                    result.push(ch);
                }
            }
            result.push('\n');
        }
    }

    Ok(result)
}

#[cfg(feature = "encode")]
pub fn fits_in_terminal(data: &[u8]) -> Result<bool> {
    use terminal_size::{terminal_size, Height, Width};

    let code = QrCode::with_error_correction_level(data, EcLevel::M)
        .map_err(|e| anyhow!("Failed to create QR code: {}", e))?;

    let qr_size = code.width();
    let qr_with_quiet = qr_size + 4; // Add quiet zone

    let scale: usize = 1;
    let display_width = qr_with_quiet * scale;
    let display_height = (qr_with_quiet + 1) / 2 * scale;

    let (term_width, term_height) = terminal_size()
        .map(|(Width(w), Height(h))| (w as usize, h as usize))
        .unwrap_or((80, 24));

    // Check if it fits (allow 6 lines for header/footer/spacing)
    if display_width > term_width || display_height + 6 > term_height {
        Ok(false)
    } else {
        Ok(true)
    }
}

#[cfg(all(test, feature = "encode", feature = "decode"))]
mod tests {
    use super::*;

    #[test]
    fn test_qr_generation() {
        let data = b"Hello, World!";
        let (image, _) = generate_qr_image(data, None, 4, None).unwrap();
        assert!(image.width() > 0);
        assert!(image.height() > 0);
    }

    #[test]
    fn test_qr_roundtrip() {
        let data = b"Test data for QR code roundtrip";
        let (image, _) = generate_qr_image(data, None, 4, None).unwrap();

        // Convert to grayscale for decoding
        let gray: GrayImage = image::DynamicImage::ImageRgb8(image).to_luma8();

        let decoded = decode_qr_from_gray(&gray).unwrap();
        assert_eq!(decoded, data);
    }
}
