# Project: `fountain-core`

## For the AI Agents

When interacting with this project, consider the following:

- **Portable Build:** `./script/build.sh` (Builds both binaries in Docker).
- **Pure Rust:** No external libraries like OpenCV are needed. All decoding is done via `rqrr` and `image` crates.
- **directory /local:** The `/local` directory is for local use only and will not be committed to git, so do not use scripts from the `/local` directory in `README.md`.

The project uses `rqrr` for QR code decoding in `fountain-decode` and `fountain-wasm`.

## Recent Features
- **Base45 Encoding:** Migrated from Base64 to Base45. This enables QR codes to use the highly efficient **Alphanumeric mode**, significantly increasing single-frame data capacity and reducing image density (making them easier to scan).
- **Efficiency Benchmarking:** Added an end-to-end efficiency test in `tests/integration_test.rs` to measure the conversion ratio from original files to final GIF transfers.
- **Removed OpenCV:** The project is now 100% Rust. Video file decoding has been removed in favor of simpler image/GIF-based transfers.
- **RaptorQ-only Mode:** The project exclusively uses RaptorQ (Fountain Codes) for encoding and decoding.
- **Incremental Decoding (Early Exit):** `fountain-decode` now supports incremental decoding for both GIFs and image sequences. It stops processing as soon as enough RaptorQ packets are collected to reconstruct the file.
- **Unified Codebase:** Core encoding and decoding logic has been abstracted into shared internal functions (`prepare_chunks`, `decode_core`) to ensure consistency and maintainability.
- **GIF Optimization:** `fountain-encode` generates GIFs with an initial "Anchor Frame" (2s duration) containing file metadata.
- **Configurable Pixel Scale:** `fountain-encode` supports a `--pixel-scale` argument to control the size of generated QR codes.
