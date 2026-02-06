use anyhow::{anyhow, Result};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
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
pub const CHECKSUM_SIZE: usize = 8;
pub const HEADER_SIZE: usize = 11; // 1 (version) + 4 (transfer len) + 4 (esi) + 2 (packet size)

#[derive(Debug, Clone)]
pub struct ChunkHeader {
    pub version: u8,
    pub total: u32,       // Transfer Length
    pub index: u32,       // ESI
    pub packet_size: u16, // Packet Size
}

#[derive(Debug, Clone)]
pub struct Chunk {
    pub header: ChunkHeader,
    pub data: Vec<u8>,
}

impl ChunkHeader {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = vec![0u8; HEADER_SIZE];
        bytes[0] = self.version;
        bytes[1..5].copy_from_slice(&self.total.to_be_bytes());
        bytes[5..9].copy_from_slice(&self.index.to_be_bytes());
        bytes[9..11].copy_from_slice(&self.packet_size.to_be_bytes());
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize)> {
        if bytes.is_empty() {
            return Err(anyhow!("Invalid header: empty"));
        }
        let version = bytes[0];
        if version != 1 {
            return Err(anyhow!("Unsupported chunk version: {}. Only Version 1 (RaptorQ) is supported.", version));
        }

        if bytes.len() < HEADER_SIZE {
            return Err(anyhow!("Invalid header: too short"));
        }
        let total = u32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);
        let index = u32::from_be_bytes([bytes[5], bytes[6], bytes[7], bytes[8]]);
        let packet_size = u16::from_be_bytes([bytes[9], bytes[10]]);
        Ok((
            ChunkHeader {
                version,
                total,
                index,
                packet_size,
            },
            HEADER_SIZE,
        ))
    }
}

impl Chunk {
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let header_bytes = self.header.to_bytes();
        let mut result = Vec::with_capacity(header_bytes.len() + self.data.len());
        result.extend_from_slice(&header_bytes);
        result.extend_from_slice(&self.data);
        Ok(result)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let (header, header_len) = ChunkHeader::from_bytes(bytes)?;
        let data = bytes[header_len..].to_vec();

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

pub fn calculate_checksum(data: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    result[..CHECKSUM_SIZE].to_vec()
}

// Pack data: [Checksum 8B] [Filename] [\0] [Content]
pub fn pack_data(data: &[u8], filename: &str) -> Vec<u8> {
    let checksum = calculate_checksum(data);
    // Sanitize filename: remove null bytes
    let clean_filename = filename.replace('\0', "");

    let mut packed = Vec::with_capacity(CHECKSUM_SIZE + clean_filename.len() + 1 + data.len());
    packed.extend_from_slice(&checksum);
    packed.extend_from_slice(clean_filename.as_bytes());
    packed.push(0); // Null terminator
    packed.extend_from_slice(data);
    packed
}

// Unpack data: -> (Filename, Content)
pub fn unpack_data(packed: &[u8]) -> Result<(String, Vec<u8>)> {
    if packed.len() < CHECKSUM_SIZE + 2 {
        // Min: Checksum + 1 char + \0
        return Err(anyhow!("Invalid packed data: too short"));
    }

    let expected_checksum = &packed[..CHECKSUM_SIZE];

    let mut null_pos = None;
    for i in CHECKSUM_SIZE..packed.len() {
        if packed[i] == 0 {
            null_pos = Some(i);
            break;
        }
    }

    let null_idx =
        null_pos.ok_or_else(|| anyhow!("Invalid packed data: missing filename terminator"))?;

    let filename_bytes = &packed[CHECKSUM_SIZE..null_idx];
    let filename = std::str::from_utf8(filename_bytes)
        .map_err(|_| anyhow!("Invalid filename: not valid UTF-8"))?
        .to_string();

    let content = packed[null_idx + 1..].to_vec();

    let actual_checksum = calculate_checksum(&content);
    if actual_checksum != expected_checksum {
        return Err(anyhow!(
            "Checksum mismatch: expected {:?}, got {:?}",
            expected_checksum,
            actual_checksum
        ));
    }

    Ok((filename, content))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pack_unpack() {
        let data = b"Some random data";
        let filename = "example.file";

        let packed = pack_data(data, filename);
        let (name, content) = unpack_data(&packed).unwrap();

        assert_eq!(name, filename);
        assert_eq!(content, data);
    }
}
