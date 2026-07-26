use std::ffi::{CStr, CString};
use std::os::raw::{c_char};
use std::ptr;

#[cfg(feature = "decode")]
use std::collections::HashMap;
#[cfg(feature = "decode")]
use raptorq::{Decoder, EncodingPacket, ObjectTransmissionInformation};
#[cfg(feature = "decode")]
use crate::chunk::{decompress, unpack_data, Chunk};

#[cfg(feature = "decode")]
pub struct FfiDecoder {
    chunks: HashMap<u32, Chunk>,
    decoder: Option<Decoder>,
    total_chunks: Option<u32>,
    raptorq_transfer_length: Option<u64>,
    decoded_filename: Option<String>,
    decoded_data: Option<Vec<u8>>,
}

#[cfg(feature = "decode")]
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

#[cfg(feature = "decode")]
#[no_mangle]
pub extern "C" fn fountain_decoder_create() -> *mut FfiDecoder {
    Box::into_raw(Box::new(FfiDecoder::new()))
}

#[cfg(feature = "decode")]
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
#[cfg(feature = "decode")]
#[no_mangle]
pub unsafe extern "C" fn fountain_decoder_feed(
    ptr: *mut FfiDecoder,
    qr_str_ptr: *const c_char,
    out_current: *mut u32,
    out_total: *mut u32,
) -> i32 {
    // The scanned string comes from the camera and may be an unrelated QR code
    // (e.g. a URL or WiFi QR) that happens to pass our loose base45/header
    // validation. Feeding such garbage into raptorq can trigger an internal
    // panic (e.g. division by zero, out-of-bounds slice). A panic unwinding
    // across this `extern "C"` boundary is undefined behavior and aborts the
    // whole process, so we must never let one escape from here.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        fountain_decoder_feed_impl(ptr, qr_str_ptr, out_current, out_total)
    }));
    result.unwrap_or(3) // Treat any internal panic as a decode error.
}

#[cfg(feature = "decode")]
unsafe fn fountain_decoder_feed_impl(
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

    // Reject headers that parsed structurally but carry nonsensical values
    // (e.g. from an unrelated QR code that coincidentally passed the version
    // byte check). packet_size == 0 would divide-by-zero when configuring
    // raptorq below; a data length that doesn't match a raptorq::EncodingPacket
    // (4-byte payload ID + packet_size bytes of symbol data) would panic with
    // an out-of-bounds slice index in EncodingPacket::deserialize.
    if chunk.header.packet_size == 0
        || chunk.header.total == 0
        || chunk.data.len() != chunk.header.packet_size as usize + 4
    {
        return 0;
    }

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

#[cfg(feature = "decode")]
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

#[cfg(feature = "decode")]
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

#[cfg(feature = "decode")]
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

/// Opaque struct representing the fountain encoder instance.
///
/// Holds a pre-computed, fixed-size sequence of Base45-encoded QR packet
/// strings. Callers are expected to render each packet as a QR code
/// themselves (e.g. with a platform-native QR library) and cycle through
/// them; per the Fountain Codes design, any sufficiently large subset,
/// scanned in any order, is enough for the receiver to reconstruct the file.
#[cfg(feature = "encode")]
pub struct FfiEncoder {
    qr_strings: Vec<String>,
}

/// Create a new FfiEncoder from raw file data.
///
/// @param data_ptr Pointer to the raw file bytes.
/// @param data_len Length of the raw file bytes.
/// @param filename_ptr C-string with the original filename, embedded in the stream metadata.
/// @param chunk_size Max payload size (bytes) per QR packet. Pass 0 to use the built-in default.
/// @param out_total_packets Pointer to write the total number of packets generated.
/// @return Pointer to the new FfiEncoder, or NULL on error (invalid input, or data too large
///         to fit at the requested chunk_size). Caller must free via fountain_encoder_free.
#[cfg(feature = "encode")]
#[no_mangle]
pub unsafe extern "C" fn fountain_encoder_create(
    data_ptr: *const u8,
    data_len: u32,
    filename_ptr: *const c_char,
    chunk_size: u32,
    out_total_packets: *mut u32,
) -> *mut FfiEncoder {
    if data_ptr.is_null() || filename_ptr.is_null() {
        return ptr::null_mut();
    }

    let data = std::slice::from_raw_parts(data_ptr, data_len as usize);

    let filename = match CStr::from_ptr(filename_ptr).to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };

    let requested_size = if chunk_size == 0 {
        None
    } else {
        Some(chunk_size as usize)
    };

    // Fountain codes only require ~K (any distinct K source-equivalent packets) to
    // decode, not all of them. A `redundancy_factor` of 1.0 keeps the broadcast
    // pool at K+2 packets (see prepare_chunks_from_data's `.max(source_packets + 2)`
    // floor) instead of doubling it — this keeps the cycling stream close to what
    // the receiver actually needs, while the +2 margin still tolerates a couple of
    // missed/blurry scans without forcing a full extra loop.
    let (chunks, _effective_size) =
        match crate::encode::prepare_chunks_from_data_for_ffi(data, filename, requested_size, 1.0)
        {
            Ok(result) => result,
            Err(_) => return ptr::null_mut(),
        };

    let mut qr_strings = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        let chunk_bytes = match chunk.to_bytes() {
            Ok(b) => b,
            Err(_) => return ptr::null_mut(),
        };
        qr_strings.push(base45::encode(&chunk_bytes));
    }

    if !out_total_packets.is_null() {
        *out_total_packets = qr_strings.len() as u32;
    }

    Box::into_raw(Box::new(FfiEncoder { qr_strings }))
}

/// Free the memory of the FfiEncoder instance.
#[cfg(feature = "encode")]
#[no_mangle]
pub unsafe extern "C" fn fountain_encoder_free(ptr: *mut FfiEncoder) {
    if !ptr.is_null() {
        let _ = Box::from_raw(ptr);
    }
}

/// Get the total number of packets in the encoded stream.
#[cfg(feature = "encode")]
#[no_mangle]
pub unsafe extern "C" fn fountain_encoder_get_total_packets(ptr: *mut FfiEncoder) -> u32 {
    if ptr.is_null() {
        return 0;
    }
    (&*ptr).qr_strings.len() as u32
}

/// Get the Base45-encoded QR packet string at the given index.
///
/// Returns NULL if `ptr` is null or `index` is out of range.
/// Caller must free the returned string via fountain_free_string.
#[cfg(feature = "encode")]
#[no_mangle]
pub unsafe extern "C" fn fountain_encoder_get_packet(
    ptr: *mut FfiEncoder,
    index: u32,
) -> *mut c_char {
    if ptr.is_null() {
        return ptr::null_mut();
    }
    let encoder = &*ptr;
    match encoder.qr_strings.get(index as usize) {
        Some(s) => match CString::new(s.as_str()) {
            Ok(c_str) => c_str.into_raw(),
            Err(_) => ptr::null_mut(),
        },
        None => ptr::null_mut(),
    }
}
