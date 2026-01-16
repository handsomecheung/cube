use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use std::fs;
use std::path::Path;

use crate::chunk::{merge_chunks, Chunk};
use crate::qr::decode_qr_image;

pub struct DecodeResult {
    pub original_filename: String,
    pub output_path: String,
    pub num_chunks: usize,
}

pub fn decode_qr_codes(input_dir: &Path, output_path: Option<&Path>) -> Result<DecodeResult> {
    let mut png_files: Vec<_> = fs::read_dir(input_dir)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .map(|ext| ext.to_ascii_lowercase() == "png")
                .unwrap_or(false)
        })
        .map(|entry| entry.path())
        .collect();

    if png_files.is_empty() {
        return Err(anyhow!("No PNG files found in directory"));
    }

    png_files.sort();

    println!("Found {} QR code image(s)", png_files.len());

    let mut chunks = Vec::new();
    for (i, png_path) in png_files.iter().enumerate() {
        println!(
            "  Decoding {}/{}: {}",
            i + 1,
            png_files.len(),
            png_path.file_name().unwrap_or_default().to_string_lossy()
        );

        let qr_data = decode_qr_image(png_path)?;

        let qr_string = String::from_utf8(qr_data)?;
        let chunk_bytes = BASE64
            .decode(&qr_string)
            .map_err(|e| anyhow!("Failed to decode base64: {}", e))?;

        let chunk = Chunk::from_bytes(&chunk_bytes)?;
        chunks.push(chunk);
    }

    let (original_filename, data) = merge_chunks(chunks.clone())?;
    let num_chunks = chunks.len();

    let final_output_path = match output_path {
        Some(p) => p.to_path_buf(),
        None => {
            // Use the input directory's parent and original filename
            let parent = input_dir.parent().unwrap_or(Path::new("."));
            parent.join(&original_filename)
        }
    };

    fs::write(&final_output_path, &data)?;

    Ok(DecodeResult {
        original_filename,
        output_path: final_output_path.to_string_lossy().to_string(),
        num_chunks,
    })
}

pub fn decode_single_qr(qr_path: &Path) -> Result<Chunk> {
    let qr_data = decode_qr_image(qr_path)?;
    let qr_string = String::from_utf8(qr_data)?;
    let chunk_bytes = BASE64.decode(&qr_string)?;
    Chunk::from_bytes(&chunk_bytes)
}
