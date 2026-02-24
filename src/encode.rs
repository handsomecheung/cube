use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use image::codecs::gif::GifEncoder;
use image::{Delay, Frame, RgbaImage};
use qrcode::Version;
use raptorq::Encoder as RQEncoder;
use std::fs;
use std::path::Path;
use std::time::Duration;

use crate::chunk::{compress, pack_data, Chunk, ChunkHeader, DEFAULT_PAYLOAD_SIZE, HEADER_SIZE};
use crate::qr::{generate_qr_image, render_qr_to_terminal, save_qr_image, QR_FILE_EXTENSION};

pub struct EncodeResult {
    pub num_chunks: usize,
    pub output_files: Vec<String>,
    pub effective_size: usize,
}

pub struct TerminalQrData {
    pub filename: String,
    pub total: usize,
    pub qr_strings: Vec<String>,
    pub effective_size: usize,
}

/// Internal helper to handle the common logic of reading, compressing, and finding the optimal
/// packet size for RaptorQ encoding while ensuring it fits via a provided check.
fn prepare_chunks<F>(
    input_path: &Path,
    chunk_size: Option<usize>,
    default_size: usize,
    min_size: usize,
    reduction_step: usize,
    redundancy_factor: f64,
    fit_check_fn: F,
) -> Result<(Vec<Chunk>, usize, String)>
where
    F: Fn(&[u8]) -> Result<bool>,
{
    let data = fs::read(input_path)?;
    let filename = input_path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow!("Invalid filename"))?
        .to_string();

    let packed = pack_data(&data, &filename);
    let compressed = compress(&packed)?;

    let mut current_size = chunk_size.unwrap_or(default_size);

    loop {
        // Ensure packet size is even for RaptorQ
        let packet_size = (current_size.saturating_sub(HEADER_SIZE)) as u16;
        let packet_size = packet_size - (packet_size % 2);

        if packet_size < 4 {
            if current_size <= min_size {
                break;
            }
            current_size = current_size.saturating_sub(reduction_step).max(min_size);
            continue;
        }

        let rq_encoder = RQEncoder::with_defaults(&compressed, packet_size);

        // Generate one packet to test fit
        let test_packets = rq_encoder.get_encoded_packets(1);
        if let Some(first_packet) = test_packets.first() {
            let chunk = Chunk {
                header: ChunkHeader {
                    version: 1,
                    total: compressed.len() as u32,
                    index: 0,
                    packet_size,
                },
                data: first_packet.serialize(),
            };

            let chunk_bytes = chunk.to_bytes()?;
            let encoded = BASE64.encode(&chunk_bytes);

            if fit_check_fn(encoded.as_bytes())? {
                // Fits. Generate all packets.
                let source_packets = (compressed.len() as f64 / packet_size as f64).ceil() as u32;
                let total_packets = (source_packets as f64 * redundancy_factor).ceil() as u32;
                let total_packets = total_packets.max(source_packets + 2);

                let packets_data = rq_encoder.get_encoded_packets(total_packets);
                let mut chunks = Vec::with_capacity(packets_data.len());

                for (i, packet) in packets_data.into_iter().enumerate() {
                    chunks.push(Chunk {
                        header: ChunkHeader {
                            version: 1,
                            total: compressed.len() as u32,
                            index: i as u32,
                            packet_size,
                        },
                        data: packet.serialize(),
                    });
                }

                return Ok((chunks, current_size, filename));
            }
        }

        if current_size > min_size {
            current_size = current_size.saturating_sub(reduction_step).max(min_size);
        } else {
            break;
        }
    }

    Err(anyhow!(
        "Data too large to fit in QR code even at minimum payload size ({} bytes).",
        min_size
    ))
}

/// Helper function to split data into chunks using RaptorQ and ensure they fit into QR codes.
/// Returns the chunks, the effective payload size used, and the filename string.
fn prepare_chunks_for_img(
    input_path: &Path,
    chunk_size: Option<usize>,
    pixel_scale: u32,
    redundancy_factor: f64,
) -> Result<(Vec<Chunk>, usize, String)> {
    prepare_chunks(
        input_path,
        chunk_size,
        crate::chunk::MAX_PAYLOAD_SIZE,
        100, // min_size
        50,  // reduction_step
        redundancy_factor,
        |encoded| Ok(generate_qr_image(encoded, None, pixel_scale).is_ok()),
    )
    .map_err(|e| anyhow!("Failed to generate QR codes: {}", e))
}

pub fn encode_file_for_terminal(
    input_path: &Path,
    chunk_size: Option<usize>,
) -> Result<TerminalQrData> {
    let (chunks, effective_size, filename) = prepare_chunks(
        input_path,
        chunk_size,
        DEFAULT_PAYLOAD_SIZE,
        50, // min_size
        20, // reduction_step
        2.0, // redundancy_factor
        |encoded| crate::qr::fits_in_terminal(encoded),
    )
    .map_err(|e| anyhow!("Terminal too small to display QR codes even at minimum payload size. Please increase terminal size. Underlying error: {}", e))?;

    let total = chunks.len();
    let mut qr_strings = Vec::with_capacity(total);

    for chunk in chunks {
        let chunk_bytes = chunk.to_bytes()?;
        let encoded = BASE64.encode(&chunk_bytes);
        let qr_string = render_qr_to_terminal(encoded.as_bytes())?;
        qr_strings.push(qr_string);
    }

    Ok(TerminalQrData {
        filename,
        total,
        qr_strings,
        effective_size,
    })
}

/// Internal helper to process a sequence of chunks as QR images with a consistent version.
fn process_chunks_as_qr_images<F>(
    chunks: &[Chunk],
    pixel_scale: u32,
    mut processor: F,
) -> Result<()>
where
    F: FnMut(&Chunk, image::RgbImage, usize, usize) -> Result<()>,
{
    let mut fixed_version: Option<Version> = None;
    let total = chunks.len();

    for (i, chunk) in chunks.iter().enumerate() {
        let chunk_bytes = chunk.to_bytes()?;
        let encoded = BASE64.encode(&chunk_bytes);

        let (qr_image, version) =
            generate_qr_image(encoded.as_bytes(), fixed_version, pixel_scale)?;

        if fixed_version.is_none() {
            fixed_version = Some(version);
        }

        processor(chunk, qr_image, i, total)?;
    }

    Ok(())
}

pub fn encode_file_to_images(
    input_path: &Path,
    output_dir: &Path,
    chunk_size: Option<usize>,
    pixel_scale: u32,
) -> Result<EncodeResult> {
    fs::create_dir_all(output_dir)?;

    let (chunks, effective_size, filename) =
        prepare_chunks_for_img(input_path, chunk_size, pixel_scale, 1.5)?;

    let mut output_files = Vec::with_capacity(chunks.len());

    process_chunks_as_qr_images(&chunks, pixel_scale, |chunk, qr_image, i, total| {
        let output_filename = format!(
            "{}_{:04}.{}",
            filename.replace('.', "_"),
            chunk.header.index + 1,
            QR_FILE_EXTENSION
        );
        let output_path = output_dir.join(&output_filename);
        save_qr_image(&qr_image, &output_path)?;

        println!(
            "  Generated QR code {}/{}: {}",
            i + 1,
            total,
            &output_filename
        );

        output_files.push(output_filename);
        Ok(())
    })?;

    Ok(EncodeResult {
        num_chunks: chunks.len(),
        output_files,
        effective_size,
    })
}

pub fn encode_file_to_gif(
    input_path: &Path,
    output_gif: &Path,
    chunk_size: Option<usize>,
    interval_ms: u64,
    pixel_scale: u32,
) -> Result<EncodeResult> {
    let (chunks, effective_size, _filename) =
        prepare_chunks_for_img(input_path, chunk_size, pixel_scale, 1.5)?;

    if let Some(parent) = output_gif.parent() {
        fs::create_dir_all(parent)?;
    }

    let file = fs::File::create(output_gif)?;
    let mut encoder = GifEncoder::new(file);
    encoder.set_repeat(image::codecs::gif::Repeat::Infinite)?;

    process_chunks_as_qr_images(&chunks, pixel_scale, |_, qr_image, i, total| {
        let rgba_image: RgbaImage = image::DynamicImage::ImageRgb8(qr_image).into_rgba8();

        let delay = Delay::from_saturating_duration(Duration::from_millis(interval_ms));
        let frame = Frame::from_parts(rgba_image, 0, 0, delay);

        encoder.encode_frame(frame)?;

        if total <= 10 || ((i + 1) % 10 == 0 || i + 1 == total) {
            println!("  Processed frame {}/{}", i + 1, total);
        }
        Ok(())
    })?;

    Ok(EncodeResult {
        num_chunks: chunks.len(),
        output_files: vec![output_gif.to_string_lossy().to_string()],
        effective_size,
    })
}
