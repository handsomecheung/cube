use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char};
use std::ptr;
use raptorq::{Decoder, EncodingPacket, ObjectTransmissionInformation};
use crate::chunk::{decompress, unpack_data, Chunk};

pub struct FfiDecoder {
    chunks: HashMap<u32, Chunk>,
    decoder: Option<Decoder>,
    total_chunks: Option<u32>,
    raptorq_transfer_length: Option<u64>,
    decoded_filename: Option<String>,
    decoded_data: Option<Vec<u8>>,
}

impl FfiDecoder {
    fn new() -> Self {
        Self {
            chunks: HashMap::new(),
            decoder: None,
            total_chunks: None,
            raptorq_transfer_length: None,
            decoded_filename: None,
            decoded_data: None,
        }
    }
}

#[no_mangle]
pub extern "C" fn fountain_decoder_create() -> *mut FfiDecoder {
    Box::into_raw(Box::new(FfiDecoder::new()))
}

#[no_mangle]
pub unsafe extern "C" fn fountain_decoder_free(ptr: *mut FfiDecoder) {
    if !ptr.is_null() {
        let _ = Box::from_raw(ptr);
    }
}

// Result codes:
// 0: Scanning (No new unique chunk found, or duplicate chunk)
// 1: ChunkFound (New unique chunk added to the decoder)
// 2: Complete (Decoding completed and verified)
// 3: Error (An error occurred during parsing or decoding)
#[no_mangle]
pub unsafe extern "C" fn fountain_decoder_feed(
    ptr: *mut FfiDecoder,
    qr_str_ptr: *const c_char,
    out_current: *mut u32,
    out_total: *mut u32,
) -> i32 {
    if ptr.is_null() || qr_str_ptr.is_null() {
        return 3;
    }

    let decoder = &mut *ptr;

    let c_str = CStr::from_ptr(qr_str_ptr);
    let qr_str = match c_str.to_str() {
        Ok(s) => s.trim(),
        Err(_) => return 3,
    };

    let chunk_bytes = match base45::decode(qr_str) {
        Ok(bytes) => bytes,
        Err(_) => return 0, // Not a valid base45 string, might be scanning background noise
    };

    let chunk = match Chunk::from_bytes(&chunk_bytes) {
        Ok(c) => c,
        Err(_) => return 0, // Not a valid chunk format
    };

    // Initialize the RaptorQ decoder using the first valid chunk
    if decoder.decoder.is_none() {
        let transfer_len = chunk.header.total as u64;
        let packet_size = chunk.header.packet_size;
        decoder.raptorq_transfer_length = Some(transfer_len);

        let config = ObjectTransmissionInformation::with_defaults(transfer_len, packet_size);
        decoder.decoder = Some(Decoder::new(config));

        // Estimate total packets needed (K)
        let source_packets = (transfer_len as f64 / packet_size as f64).ceil() as u32;
        decoder.total_chunks = Some(source_packets);
    }

    // Output progress before checking duplicates
    if !out_total.is_null() {
        *out_total = decoder.total_chunks.unwrap_or(0);
    }

    if decoder.chunks.contains_key(&chunk.header.index) {
        if !out_current.is_null() {
            *out_current = decoder.chunks.len() as u32;
        }
        return 0; // Already processed this chunk
    }

    let index = chunk.header.index;
    let packet_data = chunk.data.clone();
    decoder.chunks.insert(index, chunk);

    if !out_current.is_null() {
        *out_current = decoder.chunks.len() as u32;
    }

    if let Some(dec) = &mut decoder.decoder {
        let packet = EncodingPacket::deserialize(&packet_data);
        if let Some(result_data) = dec.decode(packet) {
            let mut final_data = result_data;
            if let Some(len) = decoder.raptorq_transfer_length {
                final_data.truncate(len as usize);
            }

            match decompress(&final_data).and_then(|packed| unpack_data(&packed)) {
                Ok((filename, data)) => {
                    decoder.decoded_filename = Some(filename);
                    decoder.decoded_data = Some(data);
                    return 2; // Complete
                }
                Err(_) => {
                    return 3; // Error decompressing
                }
            }
        }
    }

    1 // ChunkFound
}

#[no_mangle]
pub unsafe extern "C" fn fountain_decoder_get_filename(ptr: *mut FfiDecoder) -> *mut c_char {
    if ptr.is_null() {
        return ptr::null_mut();
    }
    let decoder = &*ptr;
    match &decoder.decoded_filename {
        Some(filename) => {
            if let Ok(c_str) = CString::new(filename.as_str()) {
                c_str.into_raw()
            } else {
                ptr::null_mut()
            }
        }
        None => ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn fountain_decoder_get_data_size(ptr: *mut FfiDecoder) -> u32 {
    if ptr.is_null() {
        return 0;
    }
    let decoder = &*ptr;
    match &decoder.decoded_data {
        Some(data) => data.len() as u32,
        None => 0,
    }
}

#[no_mangle]
pub unsafe extern "C" fn fountain_decoder_copy_data(
    ptr: *mut FfiDecoder,
    buf: *mut u8,
    buf_len: u32,
) -> i32 {
    if ptr.is_null() || buf.is_null() {
        return -1;
    }
    let decoder = &*ptr;
    match &decoder.decoded_data {
        Some(data) => {
            if data.len() as u32 > buf_len {
                return -2; // Buffer too small
            }
            ptr::copy_nonoverlapping(data.as_ptr(), buf, data.len());
            0 // Success
        }
        None => -3, // No data decoded yet
    }
}

#[no_mangle]
pub unsafe extern "C" fn fountain_free_string(str_ptr: *mut c_char) {
    if !str_ptr.is_null() {
        let _ = CString::from_raw(str_ptr);
    }
}
