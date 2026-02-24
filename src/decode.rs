use anyhow::{anyhow, Result};
use image::codecs::gif::GifDecoder;
use image::{AnimationDecoder, DynamicImage};
use raptorq::{Decoder, EncodingPacket, ObjectTransmissionInformation};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::Path;

use crate::chunk::{decompress, unpack_data, Chunk};
use crate::qr::{decode_qr_from_dynamic_image, QR_FILE_EXTENSION};

pub struct DecodeResult {
    pub original_filename: String,
    pub output_path: String,
    pub num_chunks: usize,
}

struct RaptorQStreamDecoder {
    chunks: HashMap<u32, Chunk>,
    decoder: Option<Decoder>,
}

impl RaptorQStreamDecoder {
    fn new() -> Self {
        Self {
            chunks: HashMap::new(),
            decoder: None,
        }
    }

    fn add_chunk(&mut self, chunk: Chunk) -> Result<Option<(String, Vec<u8>)>> {
        if self.decoder.is_none() {
            let config = ObjectTransmissionInformation::with_defaults(
                chunk.header.total as u64,
                chunk.header.packet_size,
            );
            self.decoder = Some(Decoder::new(config));
        }

        if !self.chunks.contains_key(&chunk.header.index) {
            let index = chunk.header.index;
            let total_len = chunk.header.total as usize;
            let packet_data = chunk.data.clone();
            self.chunks.insert(index, chunk);

            if let Some(dec) = &mut self.decoder {
                let packet = EncodingPacket::deserialize(&packet_data);
                if let Some(result_data) = dec.decode(packet) {
                    let mut final_data = result_data;
                    final_data.truncate(total_len);
                    let packed = decompress(&final_data)?;
                    return Ok(Some(unpack_data(&packed)?));
                }
            }
        }
        Ok(None)
    }

    fn num_chunks(&self) -> usize {
        self.chunks.len()
    }
}

fn decode_qr_bytes_to_chunk(qr_bytes: &[u8]) -> Option<Chunk> {
    let qr_string = std::str::from_utf8(qr_bytes).ok()?;
    let chunk_bytes = base45::decode(qr_string).ok()?;
    Chunk::from_bytes(&chunk_bytes).ok()
}

fn save_decoded_file(
    original_filename: String,
    data: Vec<u8>,
    num_chunks: usize,
    output_path: Option<&Path>,
    default_dir: &Path,
) -> Result<DecodeResult> {
    let final_output_path = match output_path {
        Some(p) => p.to_path_buf(),
        None => default_dir.join(&original_filename),
    };

    fs::write(&final_output_path, &data)?;

    Ok(DecodeResult {
        original_filename,
        output_path: final_output_path.to_string_lossy().to_string(),
        num_chunks,
    })
}

fn decode_core<I>(
    images: I,
    output_file: Option<&Path>,
    default_dir: &Path,
) -> Result<DecodeResult>
where
    I: Iterator<Item = (Result<DynamicImage>, String)>,
{
    let mut rq_decoder = RaptorQStreamDecoder::new();
    let mut count = 0;

    for (img_result, label) in images {
        count += 1;
        let img = match img_result {
            Ok(img) => img,
            Err(e) => {
                println!("    Failed to load {}: {}", label, e);
                continue;
            }
        };

        if let Ok(qr_bytes) = decode_qr_from_dynamic_image(&img) {
            if let Some(chunk) = decode_qr_bytes_to_chunk(&qr_bytes) {
                if let Some((original_filename, data)) = rq_decoder.add_chunk(chunk)? {
                    println!("RaptorQ decoding successful at {}!", label);
                    return save_decoded_file(
                        original_filename,
                        data,
                        rq_decoder.num_chunks(),
                        output_file,
                        default_dir,
                    );
                }
            }
        }
    }

    if rq_decoder.num_chunks() == 0 {
        return Err(anyhow!("No valid QR chunks found"));
    }

    Err(anyhow!(
        "Could not decode with RaptorQ (insufficient packets after {} items)",
        count
    ))
}

pub fn decode_from_gif(input_file: &Path, output_file: Option<&Path>) -> Result<DecodeResult> {
    let file = File::open(input_file)?;
    let reader = BufReader::new(file);
    let gif_decoder = GifDecoder::new(reader)?;
    let frames = gif_decoder.into_frames();

    println!("Decoding QR codes from GIF: {}", input_file.display());

    let images = frames.enumerate().map(|(i, frame_result)| {
        let label = format!("frame {}", i + 1);
        let res = frame_result
            .map(|frame| DynamicImage::ImageRgba8(frame.buffer().clone()))
            .map_err(anyhow::Error::from);
        (res, label)
    });

    decode_core(images, output_file, Path::new("."))
}

pub fn decode_from_images(input_dir: &Path, output_file: Option<&Path>) -> Result<DecodeResult> {
    let images_files: Vec<_> = fs::read_dir(input_dir)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .map(|ext| ext.to_ascii_lowercase() == QR_FILE_EXTENSION)
                .unwrap_or(false)
        })
        .map(|entry| entry.path())
        .collect();

    if images_files.is_empty() {
        return Err(anyhow!(
            "No image ({}) files found in directory",
            QR_FILE_EXTENSION
        ));
    }

    println!("Found {} QR code image(s)", images_files.len());

    let images = images_files.into_iter().map(|path| {
        let label = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let res = image::open(path).map_err(anyhow::Error::from);
        (res, label)
    });

    decode_core(
        images,
        output_file,
        input_dir.parent().unwrap_or(Path::new(".")),
    )
}
