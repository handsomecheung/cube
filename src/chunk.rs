use anyhow::{anyhow, Result};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};

// Default chunk size for QR code generation
// Smaller = smaller QR codes but more of them
// Larger = larger QR codes but fewer of them
//
// QR code size reference (binary mode, M error correction):
//   ~100 bytes -> ~29x29 modules (fits in small terminal)
//   ~200 bytes -> ~37x37 modules
//   ~500 bytes -> ~53x53 modules
//   ~1400 bytes -> ~73x73 modules (original default)
pub const DEFAULT_PAYLOAD_SIZE: usize = 100; // Small default for terminal display
pub const MAX_PAYLOAD_SIZE: usize = 1400; // Max for file output

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkHeader {
    pub filename: String,
    pub total: usize,
    pub index: usize,
    pub checksum: String,
}

#[derive(Debug, Clone)]
pub struct Chunk {
    pub header: ChunkHeader,
    pub data: Vec<u8>,
}

impl Chunk {
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let header_json = serde_json::to_string(&self.header)?;
        let header_bytes = header_json.as_bytes();

        // Format: [header_len (4 bytes)] [header_json] [data]
        let header_len = header_bytes.len() as u32;
        let mut result = Vec::new();
        result.extend_from_slice(&header_len.to_be_bytes());
        result.extend_from_slice(header_bytes);
        result.extend_from_slice(&self.data);

        Ok(result)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 4 {
            return Err(anyhow!("Invalid chunk data: too short"));
        }

        let header_len = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;

        if bytes.len() < 4 + header_len {
            return Err(anyhow!("Invalid chunk data: header truncated"));
        }

        let header_json = std::str::from_utf8(&bytes[4..4 + header_len])?;
        let header: ChunkHeader = serde_json::from_str(header_json)?;
        let data = bytes[4 + header_len..].to_vec();

        Ok(Chunk { header, data })
    }
}

pub fn compress(data: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(data)?;
    Ok(encoder.finish()?)
}

pub fn decompress(data: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(data);
    let mut result = Vec::new();
    decoder.read_to_end(&mut result)?;
    Ok(result)
}

pub fn calculate_checksum(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    hex::encode(&result[..8]) // Use first 8 bytes for shorter checksum
}

pub fn split_into_chunks(data: &[u8], filename: &str) -> Result<Vec<Chunk>> {
    split_into_chunks_with_size(data, filename, MAX_PAYLOAD_SIZE)
}

pub fn split_into_chunks_with_size(
    data: &[u8],
    filename: &str,
    payload_size: usize,
) -> Result<Vec<Chunk>> {
    let compressed = compress(data)?;
    let checksum = calculate_checksum(data);

    let total_chunks = (compressed.len() + payload_size - 1) / payload_size;
    let total_chunks = total_chunks.max(1);

    let mut chunks = Vec::new();

    for (index, chunk_data) in compressed.chunks(payload_size).enumerate() {
        let header = ChunkHeader {
            filename: filename.to_string(),
            total: total_chunks,
            index,
            checksum: checksum.clone(),
        };

        chunks.push(Chunk {
            header,
            data: chunk_data.to_vec(),
        });
    }

    if chunks.is_empty() {
        let header = ChunkHeader {
            filename: filename.to_string(),
            total: 1,
            index: 0,
            checksum,
        };
        chunks.push(Chunk {
            header,
            data: Vec::new(),
        });
    }

    Ok(chunks)
}

pub fn merge_chunks(mut chunks: Vec<Chunk>) -> Result<(String, Vec<u8>)> {
    if chunks.is_empty() {
        return Err(anyhow!("No chunks to merge"));
    }

    chunks.sort_by_key(|c| c.header.index);

    let filename = chunks[0].header.filename.clone();
    let expected_total = chunks[0].header.total;
    let expected_checksum = chunks[0].header.checksum.clone();

    if chunks.len() != expected_total {
        return Err(anyhow!(
            "Missing chunks: expected {}, got {}",
            expected_total,
            chunks.len()
        ));
    }

    // Verify indices are sequential
    for (i, chunk) in chunks.iter().enumerate() {
        if chunk.header.index != i {
            return Err(anyhow!("Missing chunk at index {}", i));
        }
    }

    // Merge data
    let mut compressed_data = Vec::new();
    for chunk in chunks {
        compressed_data.extend_from_slice(&chunk.data);
    }

    let data = decompress(&compressed_data)?;

    let actual_checksum = calculate_checksum(&data);
    if actual_checksum != expected_checksum {
        return Err(anyhow!(
            "Checksum mismatch: expected {}, got {}",
            expected_checksum,
            actual_checksum
        ));
    }

    Ok((filename, data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_roundtrip() {
        let data = b"Hello, World! This is a test.";
        let chunks = split_into_chunks(data, "test.txt").unwrap();

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].header.filename, "test.txt");
        assert_eq!(chunks[0].header.total, 1);
        assert_eq!(chunks[0].header.index, 0);

        let (filename, restored) = merge_chunks(chunks).unwrap();
        assert_eq!(filename, "test.txt");
        assert_eq!(restored, data);
    }

    #[test]
    fn test_large_data_chunking() {
        // Use data large enough to require multiple chunks even after compression
        // Simple LCG pseudo-random to create incompressible data
        let mut x: u64 = 12345;
        let data: Vec<u8> = (0..100000)
            .map(|_| {
                x = x.wrapping_mul(6364136223846793005).wrapping_add(1);
                (x >> 56) as u8
            })
            .collect();
        let chunks = split_into_chunks(&data, "large.bin").unwrap();

        assert!(
            chunks.len() > 1,
            "Expected multiple chunks, got {}",
            chunks.len()
        );

        let (filename, restored) = merge_chunks(chunks).unwrap();
        assert_eq!(filename, "large.bin");
        assert_eq!(restored, data);
    }
}
