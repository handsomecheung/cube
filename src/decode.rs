use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use image::codecs::gif::GifDecoder;
use image::{AnimationDecoder, DynamicImage};
use opencv::{
    core::Mat,
    imgproc,
    objdetect::QRCodeDetector,
    prelude::*,
    videoio::{self, VideoCapture},
};
use raptorq::{Decoder, EncodingPacket, ObjectTransmissionInformation};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::Path;

use crate::chunk::{decompress, unpack_data, Chunk};
use crate::qr::{decode_qr_from_dynamic_image, decode_qr_image};

pub struct DecodeResult {
    pub original_filename: String,
    pub output_path: String,
    pub num_chunks: usize,
}

fn reconstruct_raptorq(chunks: Vec<Chunk>) -> Result<(String, Vec<u8>)> {
    if chunks.is_empty() {
        return Err(anyhow!("No chunks to reconstruct"));
    }

    // Assume all chunks belong to the same file/encoding
    let first_header = &chunks[0].header;
    let transfer_length = first_header.total as u64;
    let packet_size = first_header.packet_size;

    let config = ObjectTransmissionInformation::with_defaults(transfer_length, packet_size);
    let mut decoder = Decoder::new(config);

    let mut result = None;
    for chunk in chunks {
        let packet = EncodingPacket::deserialize(&chunk.data);
        if let Some(data) = decoder.decode(packet) {
            result = Some(data);
            break;
        }
    }

    match result {
        Some(data) => {
            // RaptorQ pads with zeros to fill the last packet.
            // We need to truncate to the exact transfer length.
            let mut final_data = data;
            final_data.truncate(transfer_length as usize);

            let packed = decompress(&final_data)?;
            unpack_data(&packed)
        }
        None => Err(anyhow!("Not enough chunks to reconstruct data")),
    }
}

pub fn decode_from_gif(input_file: &Path, output_path: Option<&Path>) -> Result<DecodeResult> {
    let file = File::open(input_file)?;
    let reader = BufReader::new(file);
    let decoder = GifDecoder::new(reader)?;
    let frames = decoder.into_frames();

    println!("Decoding QR codes from GIF: {}", input_file.display());

    let mut chunks = HashMap::new();
    let mut frame_count = 0;
    let mut decoder_raptorq: Option<Decoder> = None;

    for (i, frame_result) in frames.enumerate() {
        let frame = frame_result?;
        frame_count += 1;

        let buffer = frame.buffer();
        let dynamic_image = DynamicImage::ImageRgba8(buffer.clone());

        if let Ok(qr_bytes) = decode_qr_from_dynamic_image(&dynamic_image) {
            let qr_string = String::from_utf8_lossy(&qr_bytes).to_string();
            if let Ok(chunk_bytes) = BASE64.decode(&qr_string) {
                if let Ok(chunk) = Chunk::from_bytes(&chunk_bytes) {
                    if decoder_raptorq.is_none() {
                        let config = ObjectTransmissionInformation::with_defaults(
                            chunk.header.total as u64,
                            chunk.header.packet_size,
                        );
                        decoder_raptorq = Some(Decoder::new(config));
                        println!(
                            "Initialized RaptorQ decoder (Size: {}, Packet: {})",
                            chunk.header.total, chunk.header.packet_size
                        );
                    }

                    if !chunks.contains_key(&chunk.header.index) {
                        chunks.insert(chunk.header.index, chunk.clone());
                        println!(
                            "Found RaptorQ packet ESI {} in frame {}",
                            chunk.header.index,
                            i + 1
                        );

                        if let Some(dec) = &mut decoder_raptorq {
                            let packet = EncodingPacket::deserialize(&chunk.data);
                            if let Some(result_data) = dec.decode(packet) {
                                println!("RaptorQ decoding successful at frame {}!", i + 1);
                                let mut final_data = result_data;
                                final_data.truncate(chunk.header.total as usize);
                                let packed = decompress(&final_data)?;
                                let (original_filename, data) = unpack_data(&packed)?;

                                let final_output_path = match output_path {
                                    Some(p) => p.to_path_buf(),
                                    None => Path::new(".").join(&original_filename),
                                };
                                fs::write(&final_output_path, &data)?;

                                return Ok(DecodeResult {
                                    original_filename,
                                    output_path: final_output_path
                                        .to_string_lossy()
                                        .to_string(),
                                    num_chunks: chunks.len(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    if chunks.is_empty() {
        return Err(anyhow!("No QR codes found in GIF"));
    }

    Err(anyhow!(
        "Could not decode with RaptorQ (insufficient packets after {} frames)",
        frame_count
    ))
}

pub fn decode_from_images(input_dir: &Path, output_path: Option<&Path>) -> Result<DecodeResult> {
    let png_files: Vec<_> = fs::read_dir(input_dir)?
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

    println!("Found {} QR code image(s)", png_files.len());

    let mut chunks = HashMap::new();

    for (i, png_path) in png_files.iter().enumerate() {
        println!(
            "  Decoding {}/{}: {}",
            i + 1,
            png_files.len(),
            png_path.file_name().unwrap_or_default().to_string_lossy()
        );

        let qr_data = match decode_qr_image(png_path) {
            Ok(d) => d,
            Err(e) => {
                println!("    Failed to decode: {}", e);
                continue;
            }
        };

        let qr_string = match String::from_utf8(qr_data) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let chunk_bytes = match BASE64.decode(&qr_string) {
            Ok(b) => b,
            Err(_) => continue,
        };

        if let Ok(chunk) = Chunk::from_bytes(&chunk_bytes) {
            chunks.insert(chunk.header.index, chunk);
        }
    }

    if chunks.is_empty() {
        return Err(anyhow!("No valid QR chunks found"));
    }

    let num_chunks = chunks.len();
    let (original_filename, data) = reconstruct_raptorq(chunks.into_values().collect())?;

    let final_output_path = match output_path {
        Some(p) => p.to_path_buf(),
        None => {
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

pub fn decode_from_video(input_file: &Path, output_path: Option<&Path>) -> Result<DecodeResult> {
    let mut cam = VideoCapture::from_file(&input_file.to_string_lossy(), videoio::CAP_ANY)?;
    if !cam.is_opened()? {
        return Err(anyhow!(
            "Failed to open video file: {}",
            input_file.display()
        ));
    }

    let frame_count = cam.get(videoio::CAP_PROP_FRAME_COUNT)? as u64;
    println!("Video has {} frames. Starting scan...", frame_count);

    let mut chunks = HashMap::new();
    let mut frame = Mat::default();
    let mut gray_frame = Mat::default();
    let mut points = Mat::default();
    let mut straight_code = Mat::default();
    let detector = QRCodeDetector::default()?;
    let mut decoder_raptorq: Option<Decoder> = None;

    for i in 0..frame_count {
        if !cam.read(&mut frame)? {
            break;
        }

        imgproc::cvt_color(
            &frame,
            &mut gray_frame,
            imgproc::COLOR_BGR2GRAY,
            0,
            opencv::core::AlgorithmHint::ALGO_HINT_DEFAULT,
        )?;

        let mut qr_bytes =
            detector.detect_and_decode(&gray_frame, &mut points, &mut straight_code)?;

        if qr_bytes.is_empty() {
            let mut inverted_frame = Mat::default();
            opencv::core::bitwise_not(&gray_frame, &mut inverted_frame, &opencv::core::no_array())?;
            qr_bytes =
                detector.detect_and_decode(&inverted_frame, &mut points, &mut straight_code)?;
        }

        if !qr_bytes.is_empty() {
            let qr_string = String::from_utf8_lossy(&qr_bytes).to_string();
            if let Ok(chunk_bytes) = BASE64.decode(&qr_string) {
                if let Ok(chunk) = Chunk::from_bytes(&chunk_bytes) {
                    if decoder_raptorq.is_none() {
                        let config = ObjectTransmissionInformation::with_defaults(
                            chunk.header.total as u64,
                            chunk.header.packet_size,
                        );
                        decoder_raptorq = Some(Decoder::new(config));
                        println!("Initialized RaptorQ decoder");
                    }

                    if !chunks.contains_key(&chunk.header.index) {
                        println!(
                            "Found RaptorQ chunk {} in frame {}",
                            chunk.header.index,
                            i + 1,
                        );
                        chunks.insert(chunk.header.index, chunk.clone());

                        if let Some(dec) = &mut decoder_raptorq {
                            let packet = EncodingPacket::deserialize(&chunk.data);
                            if let Some(result_data) = dec.decode(packet) {
                                println!("RaptorQ decoding successful!");
                                let mut final_data = result_data;
                                final_data.truncate(chunk.header.total as usize);
                                let packed = decompress(&final_data)?;
                                let (original_filename, data) = unpack_data(&packed)?;

                                let final_output_path = match output_path {
                                    Some(p) => p.to_path_buf(),
                                    None => Path::new(".").join(&original_filename),
                                };
                                fs::write(&final_output_path, &data)?;

                                return Ok(DecodeResult {
                                    original_filename,
                                    output_path: final_output_path
                                        .to_string_lossy()
                                        .to_string(),
                                    num_chunks: chunks.len(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    Err(anyhow!(
        "Could not decode with RaptorQ (insufficient packets after scanning {} frames)",
        frame_count
    ))
}