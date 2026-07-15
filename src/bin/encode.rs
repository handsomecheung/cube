use anyhow::Result;
use clap::Parser;
use std::path::{Path, PathBuf};

use fountain_core::{
    display_qr_carousel, display_qr_once, DEFAULT_PAYLOAD_SIZE, MAX_PAYLOAD_SIZE,
};

#[derive(Parser)]
#[command(name = "fountain-encode")]
#[command(author, version, about = "Encode files or content to QR codes using RaptorQ (Fountain Codes)", long_about = None)]
#[command(group(
    clap::ArgGroup::new("source")
        .required(true)
        .args(["file", "content", "input"]),
))]
struct Cli {
    /// Input file to encode
    #[arg(short = 'f', long = "file", conflicts_with_all = ["content", "input"])]
    file: Option<PathBuf>,

    /// String content to encode
    #[arg(short = 'c', long = "content", conflicts_with_all = ["file", "input"])]
    content: Option<String>,

    /// Backward compatibility: positional argument for input file
    #[arg(conflicts_with_all = ["file", "content"])]
    input: Option<PathBuf>,

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

    let (data, filename) = if let Some(file_path) = args.file.as_ref().or(args.input.as_ref()) {
        println!("Encoding file: {}", file_path.display());
        let data = std::fs::read(file_path)?;
        let filename = file_path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?
            .to_string();
        (data, filename)
    } else if let Some(content_str) = &args.content {
        println!("Encoding text content...");
        let data = content_str.clone().into_bytes();
        let filename = "".to_string();
        (data, filename)
    } else {
        anyhow::bail!("No input file or content specified.");
    };

    if let Some(size) = args.chunk_size {
        println!("Max payload size: {} bytes", size);
    }

    if args.terminal {
        run_terminal(
            &data,
            &filename,
            args.chunk_size,
            args.interval,
            args.no_carousel,
        )?;
    } else if let Some(gif_output) = &args.gif_output_file {
        run_gif(
            &data,
            &filename,
            gif_output,
            args.chunk_size,
            args.interval,
            args.pixel_scale,
        )?;
    } else if let Some(images_output) = &args.image_output_dir {
        run_images(
            &data,
            &filename,
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
    data: &[u8],
    filename: &str,
    chunk_size: Option<usize>,
    interval: u64,
    no_carousel: bool,
) -> Result<()> {
    let qrs_data = fountain_core::encode_data_for_terminal(data, filename, chunk_size)?;

    println!("Generated {} QR code(s)", qrs_data.total);

    let requested_size = chunk_size.unwrap_or(DEFAULT_PAYLOAD_SIZE);
    if qrs_data.effective_size < requested_size {
        println!(
            "WARNING! Automatically reduced payload size to {} bytes to fit terminal.",
            qrs_data.effective_size
        );
    }
    println!();

    if no_carousel || qrs_data.total == 1 {
        display_qr_once(&qrs_data);
    } else {
        println!("Starting carousel mode ({}ms interval)...", interval);
        println!("Press Ctrl+C to exit");
        std::thread::sleep(std::time::Duration::from_secs(1));
        display_qr_carousel(&qrs_data, interval);
    }

    Ok(())
}

fn run_images(
    data: &[u8],
    filename: &str,
    output_dir: &Path,
    chunk_size: Option<usize>,
    pixel_scale: u32,
) -> Result<()> {
    println!("Output directory: {}", output_dir.display());

    let result = fountain_core::encode_data_to_images(data, filename, output_dir, chunk_size, pixel_scale)?;

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
    data: &[u8],
    filename: &str,
    output_file: &Path,
    chunk_size: Option<usize>,
    interval: u64,
    pixel_scale: u32,
) -> Result<()> {
    println!("Output GIF: {}", output_file.display());
    println!("GIF frame interval: {}ms", interval);

    let result = fountain_core::encode_data_to_gif(data, filename, output_file, chunk_size, interval, pixel_scale)?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_cli_parsing_compatibility() {
        let cli = Cli::try_parse_from(["fountain-encode", "test_file.txt", "-t"]).unwrap();
        assert_eq!(cli.input, Some(PathBuf::from("test_file.txt")));
        assert!(cli.file.is_none());
        assert!(cli.content.is_none());
        assert!(cli.terminal);
    }

    #[test]
    fn test_cli_parsing_file_opt() {
        let cli = Cli::try_parse_from(["fountain-encode", "-f", "test_file.txt", "-t"]).unwrap();
        assert_eq!(cli.file, Some(PathBuf::from("test_file.txt")));
        assert!(cli.input.is_none());
        assert!(cli.content.is_none());
        assert!(cli.terminal);

        let cli = Cli::try_parse_from(["fountain-encode", "--file", "test_file.txt", "-t"]).unwrap();
        assert_eq!(cli.file, Some(PathBuf::from("test_file.txt")));
        assert!(cli.input.is_none());
        assert!(cli.content.is_none());
        assert!(cli.terminal);
    }

    #[test]
    fn test_cli_parsing_content_opt() {
        let cli = Cli::try_parse_from(["fountain-encode", "-c", "hello world", "-t"]).unwrap();
        assert_eq!(cli.content, Some("hello world".to_string()));
        assert!(cli.input.is_none());
        assert!(cli.file.is_none());
        assert!(cli.terminal);

        let cli = Cli::try_parse_from(["fountain-encode", "--content", "hello world", "-t"]).unwrap();
        assert_eq!(cli.content, Some("hello world".to_string()));
        assert!(cli.input.is_none());
        assert!(cli.file.is_none());
        assert!(cli.terminal);
    }

    #[test]
    fn test_cli_parsing_conflicts() {
        assert!(Cli::try_parse_from(["fountain-encode", "-f", "file.txt", "-c", "hello", "-t"]).is_err());
        assert!(Cli::try_parse_from(["fountain-encode", "file.txt", "-f", "other.txt", "-t"]).is_err());
        assert!(Cli::try_parse_from(["fountain-encode", "file.txt", "-c", "hello", "-t"]).is_err());
        assert!(Cli::try_parse_from(["fountain-encode", "-t"]).is_err());
    }
}
