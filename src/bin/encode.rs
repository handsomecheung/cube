use anyhow::Result;
use clap::Parser;
use std::path::{Path, PathBuf};

use fountain::{
    display_qr_carousel, display_qr_once, encode_file_for_terminal, encode_file_to_gif,
    encode_file_to_images, DEFAULT_PAYLOAD_SIZE, MAX_PAYLOAD_SIZE,
};

#[derive(Parser)]
#[command(name = "fountain-encode")]
#[command(author, version, about = "Encode files to QR codes using RaptorQ (Fountain Codes)", long_about = None)]
struct Cli {
    /// Input file to encode
    input: PathBuf,

    /// Output directory for QR code images
    #[arg(short = 'm', long = "image-output-dir", required_unless_present_any = ["terminal", "gif_output_file"])]
    image_output_dir: Option<PathBuf>,

    /// Output animated GIF file containing all QR codes
    #[arg(short = 'g', long)]
    gif_output_file: Option<PathBuf>,

    /// Display QR codes in terminal instead of saving to files
    #[arg(short, long)]
    terminal: bool,

    /// Interval in milliseconds for auto-switching QR codes in terminal mode or GIF frame duration (default: 2000)
    #[arg(short, long, default_value = "2000")]
    interval: u64,

    /// Show all QR codes at once without carousel (only with --terminal)
    #[arg(long)]
    no_carousel: bool,

    /// Maximum payload size (bytes) per QR code. Smaller values make QR codes less dense and easier to scan.
    /// Default is ~1400 for file output (high density) and 100 for terminal.
    #[arg(short = 's', long, alias = "payload-size")]
    chunk_size: Option<usize>,

    /// Pixel scale for QR code modules (default: 4).
    #[arg(long, default_value = "4")]
    pixel_scale: u32,
}

fn main() -> Result<()> {
    let args = Cli::parse();

    println!("Encoding file: {}", args.input.display());
    if let Some(size) = args.chunk_size {
        println!("Max payload size: {} bytes", size);
    }

    if args.terminal {
        run_terminal(
            &args.input,
            args.chunk_size,
            args.interval,
            args.no_carousel,
        )?;
    } else if let Some(gif_output) = &args.gif_output_file {
        run_gif(
            &args.input,
            gif_output,
            args.chunk_size,
            args.interval,
            args.pixel_scale,
        )?;
    } else if let Some(images_output) = &args.image_output_dir {
        run_images(
            &args.input,
            images_output,
            args.chunk_size,
            args.pixel_scale,
        )?;
    } else {
        anyhow::bail!(
            "No output method specified. Use --terminal, --image-output-dir, or --gif-output-file."
        );
    }

    Ok(())
}

fn run_terminal(
    input_file: &Path,
    chunk_size: Option<usize>,
    interval: u64,
    no_carousel: bool,
) -> Result<()> {
    let data = encode_file_for_terminal(input_file, chunk_size)?;

    println!("Generated {} QR code(s)", data.total);

    let requested_size = chunk_size.unwrap_or(DEFAULT_PAYLOAD_SIZE);
    if data.effective_size < requested_size {
        println!(
            "WARNING! Automatically reduced payload size to {} bytes to fit terminal.",
            data.effective_size
        );
    }
    println!();

    if no_carousel || data.total == 1 {
        display_qr_once(&data);
    } else {
        println!("Starting carousel mode ({}ms interval)...", interval);
        println!("Press Ctrl+C to exit");
        std::thread::sleep(std::time::Duration::from_secs(1));
        display_qr_carousel(&data, interval);
    }

    Ok(())
}

fn run_images(
    input_file: &Path,
    output_dir: &Path,
    chunk_size: Option<usize>,
    pixel_scale: u32,
) -> Result<()> {
    println!("Output directory: {}", output_dir.display());

    let result = encode_file_to_images(input_file, output_dir, chunk_size, pixel_scale)?;

    let requested_size = chunk_size.unwrap_or(MAX_PAYLOAD_SIZE);
    if result.effective_size < requested_size && result.effective_size > 0 {
        println!();
        println!(
            "WARNING! Automatically reduced payload size to {} bytes to fit QR code capacity.",
            result.effective_size
        );
    }

    println!();
    println!("Successfully created {} QR code(s)", result.num_chunks);
    Ok(())
}

fn run_gif(
    input_file: &Path,
    output_file: &Path,
    chunk_size: Option<usize>,
    interval: u64,
    pixel_scale: u32,
) -> Result<()> {
    println!("Output GIF: {}", output_file.display());
    println!("GIF frame interval: {}ms", interval);

    let result = encode_file_to_gif(input_file, output_file, chunk_size, interval, pixel_scale)?;

    let requested_size = chunk_size.unwrap_or(MAX_PAYLOAD_SIZE);
    if result.effective_size < requested_size && result.effective_size > 0 {
        println!();
        println!(
            "WARNING! Automatically reduced payload size to {} bytes to fit QR code capacity.",
            result.effective_size
        );
    }

    println!();
    println!("Successfully created {} QR code(s)", result.num_chunks);
    Ok(())
}
