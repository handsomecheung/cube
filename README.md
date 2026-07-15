# Fountain Core ⛲

Fountain Core is a high-resilience, air-gapped data transmission tool that converts any file into a stream of QR codes. By leveraging **Fountain Codes (specifically RaptorQ)**, it ensures reliable file transfer via screens, cameras, or paper, even when frames are lost or captured out of order.

## 🌟 The Magic of Fountain Codes in QR Transmission

Traditional file-to-QR methods usually split data into a fixed sequence of frames (e.g., Frame 1 of 10, Frame 2 of 10). If a single frame is missed due to camera flicker, motion blur, or a "dirty" frame, the entire transmission fails or hangs indefinitely waiting for that specific missing piece.

**Fountain Codes change the game:**
- **Order-Independent:** It doesn't matter which QR codes you scan or in what order. 
- **Loss-Tolerant:** If you miss 10% of the frames, you just keep scanning new ones. Any $N + \epsilon$ unique packets are enough to reconstruct the original $N$ blocks of data.
- **Infinite Stream:** The encoder can generate a practically endless stream of unique "fountain" packets. The receiver just "catches" enough drops from the fountain to fill its bucket.

This makes Fountain ideal for **one-way, offline transmission** where the sender cannot hear back from the receiver to retransmit lost packets.

## ⚡ Optimized for QR: Base45 Encoding

To maximize the data capacity of each QR code while maintaining high scannability, Fountain uses **Base45 encoding** (RFC 9285) instead of standard Base64.

**Why Base45?**
- **Native QR Support:** QR codes have a built-in **Alphanumeric Mode** that specifically supports the 45 characters used in Base45.
- **Higher Efficiency:** In Alphanumeric Mode, each character takes only **5.5 bits**, compared to the 8 bits required for Base64 (which forces the QR code into "Byte Mode").
- **Better Scannability:** Because Base45 is more compact at the binary level, the resulting QR codes have a **lower module density** (larger "dots") for the same amount of data. This makes them significantly easier for cameras to focus on and decode in real-world conditions.
- **Smaller Footprint:** Our benchmarks show that Base45 reduces the final GIF file size by approximately **20%** compared to Base64.

## ✨ Features

- 🚀 **High Resilience:** Uses RaptorQ (RFC 6330) for industrial-grade erasure coding.
- 📱 **Terminal Mode:** Display QR codes directly in your terminal with a carousel effect.
- 🎞️ **GIF Support:** Generate optimized, dither-free GIFs for easy sharing.
- 🖼️ **Image Export:** Save QR codes as a series of PNG images.
- 🌐 **Web Scanner (WASM):** Decode QR codes directly in your browser using your phone's camera. Perfect for receiving files on mobile without installing any apps.
- 🔌 **C-API (FFI) Support:** Build as a shared (`.so`/`.dylib`/`.dll`) or static (`.a`/`.lib`) library to integrate fountain decoding into C/C++, iOS, Android, or other languages.
- 🛠️ **Configurable:** Adjust pixel scale, payload size, and carousel intervals to match your hardware's capabilities.
- 🦀 **Pure Rust:** The project is now 100% Rust with no heavy external dependencies like OpenCV.

## 📥 Downloads

Pre-compiled binaries for the **Encoder** and the **Web Scanner (WASM)** are available on the [Releases Page](https://github.com/handsomecheung/fountain/releases/latest). 

- **fountain-encode**: Standalone binaries for Linux/macOS and Windows.
- **fountain-wasm**: Pre-built WASM and JS assets for web deployment.

## 📦 Installation

### Prerequisites
- **Encoder:** No special requirements.
- **Decoder (CLI):** No special requirements.
- **Decoder (WASM, Web Scanner):** Requires `wasm-bindgen-cli`.

### Build from Source

#### Encoder and Decoder

**Option 1: Portable Static Build (Recommended)**
Builds a standalone binary using Docker.
```bash
./script/build.sh
```

**Option 2: Local Cargo Build**
```bash
# Build both encoder and decoder
cargo build --release
```

#### Web Scanner (WASM)

Build the browser-based decoder.
```bash
./script/rust/compile.wasm.sh
```
The output will be in `www/pkg/`.

🌍 Live Demo

Try the Web Scanner directly on your mobile device:
👉 **[fountain.curvekey.app/scanner/](https://fountain.curvekey.app/scanner/)**


## 🚀 Usage

### Encoding (Sender)

```bash
fountain-encode [OPTIONS] [INPUT]
```

**Arguments:**
- `[INPUT]`: Backward compatibility: Path to the input file you want to encode.

**Options:**
- `-f, --file <FILE>`: Path to the input file you want to encode.
- `-c, --content <STRING>`: Raw text content string you want to encode.
- `-t, --terminal`: Display QR codes directly in your terminal using a carousel.
- `-g, --gif-output-file <FILE>`: Save the QR stream as an optimized animated GIF.
- `-m, --image-output-dir <DIR>`: Export QR codes as a series of individual image files (PNG).
- `-i, --interval <MS>`: Interval in milliseconds for switching frames in terminal or GIF (default: `2000`).
- `-s, --chunk-size <BYTES>`: Max payload size per QR packet. Smaller values result in simpler, easier-to-scan QR codes but more frames.
- `--pixel-scale <N>`: Scale factor for QR pixels (default: `4`).
- `--no-carousel`: In terminal mode, print all QR codes at once instead of cycling through them.

**Examples:**

*Terminal Carousel (Quickest for one-off transfers):*
```bash
# Using positional argument (backward compatibility)
fountain-encode my_secret.key --terminal --interval 500

# Using explicit --file flag
fountain-encode -f my_secret.key --terminal --interval 500
```

*Encoding raw text content string:*
```bash
fountain-encode -c "Hello, world!" --terminal
```

*Generate an optimized GIF:*
```bash
fountain-encode document.pdf -g output.gif --interval 200
```

### Decoding (Receiver)

```bash
fountain-decode [OPTIONS] <INPUT>
```

**Arguments:**
- `<INPUT>`: Path to a GIF file, or a directory containing QR image frames (PNG).

**Options:**
- `-o, --output <FILE>`: Path for the reconstructed file. If omitted, uses the original filename.

**Examples:**

*Decode from a GIF file:*
```bash
fountain-decode my_transfer.gif -o restored_file.zip
```

*Decode from a directory of images:*
```bash
fountain-decode ./qr_frames/
```

### C-API (FFI) Usage

Fountain Core can be compiled as a shared or static library to be integrated into other languages (such as C/C++, Swift, Kotlin, Python, etc.).

#### Building the Libraries

To build the static (`.a`/`.lib`) and dynamic (`.so`/`.dylib`/`.dll`) libraries, run:

```bash
cargo build --release
```

The output files will be generated in `target/release/`:
- **Static library:** `libfountain_core.a` (Linux/macOS) or `fountain_core.lib` (Windows)
- **Shared library:** `libfountain_core.so` (Linux), `libfountain_core.dylib` (macOS), or `fountain_core.dll` (Windows)

#### API Reference (C Header)

Here are the exported C-compatible functions declared in [src/ffi.rs](file:///mnt/coder-workspaces/private-workspace/repos/github/fountain-core/src/ffi.rs):

```c
#include <stdint.h>

// Opaque struct representing the fountain decoder instance
typedef struct FfiDecoder FfiDecoder;

// Result codes returned by fountain_decoder_feed
#define FOUNTAIN_RESULT_SCANNING   0  // Scanning (No new unique chunk found, or duplicate chunk)
#define FOUNTAIN_RESULT_FOUND      1  // New unique chunk added to the decoder
#define FOUNTAIN_RESULT_COMPLETE   2  // Decoding completed and verified
#define FOUNTAIN_RESULT_ERROR      3  // An error occurred during parsing or decoding

/**
 * Create a new FfiDecoder instance.
 * Caller is responsible for freeing the memory via fountain_decoder_free.
 */
FfiDecoder* fountain_decoder_create(void);

/**
 * Free the memory of the FfiDecoder instance.
 */
void fountain_decoder_free(FfiDecoder* ptr);

/**
 * Feed a Base45 encoded QR frame string to the decoder.
 *
 * @param ptr Pointer to the FfiDecoder.
 * @param qr_str_ptr C-string containing the scanned QR code content.
 * @param out_current Pointer to write the current number of unique chunks decoded.
 * @param out_total Pointer to write the total estimated chunks required.
 * @return Result code (0 to 3).
 */
int32_t fountain_decoder_feed(
    FfiDecoder* ptr,
    const char* qr_str_ptr,
    uint32_t* out_current,
    uint32_t* out_total
);

/**
 * Get the original filename of the decoded file.
 * Returns a C-string allocated on the Rust heap, which must be freed by calling fountain_free_string.
 * Returns NULL if the file has not been fully decoded yet.
 */
char* fountain_decoder_get_filename(FfiDecoder* ptr);

/**
 * Get the size in bytes of the decoded file.
 * Returns 0 if the file has not been fully decoded yet.
 */
uint32_t fountain_decoder_get_data_size(FfiDecoder* ptr);

/**
 * Copy the decoded file content into a pre-allocated buffer.
 *
 * @param ptr Pointer to the FfiDecoder.
 * @param buf Pointer to the output buffer.
 * @param buf_len Size of the output buffer.
 * @return 0 on success, negative values on error (-1: null pointers, -2: buffer too small, -3: data not ready).
 */
int32_t fountain_decoder_copy_data(
    FfiDecoder* ptr,
    uint8_t* buf,
    uint32_t buf_len
);

/**
 * Free a C-string returned by fountain_decoder_get_filename.
 */
void fountain_free_string(char* str_ptr);
```


## 🛠️ How it Works

1. **Chunking:** The file is split into small blocks.
2. **RaptorQ Encoding:** These blocks are transformed into a series of fountain packets. Each packet contains a small piece of the puzzle and metadata describing how it relates to the whole.
3. **Anchor Frame:** For GIFs, Fountain inserts an initial "Anchor Frame" containing the original filename and metadata to help the decoder prepare.
4. **QR Generation:** Each packet is encoded into a high-density QR code.
5. **Reconstruction:** The decoder captures frames (from GIF or images), extracts the fountain packets, and once it has enough mathematical overhead (usually < 5% extra), it instantly reconstructs the original file.

## 🧪 Testing

The project includes a suite of integration tests that verify the end-to-end encoding and decoding process.

```bash
./script/test.sh
```

## 📄 License

👉 [Apache-2.0](https://www.apache.org/licenses/LICENSE-2.0)
