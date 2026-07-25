//! The pixeldelta command-line tool.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use pixeldelta_core::{CompareOptions, Verdict};

use pixeldelta_cli::{compare_files, exit_code, CompareRun};

#[derive(Parser)]
#[command(name = "pixeldelta", version, about = "Fast image comparison")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Compare two PNG images.
    Compare(CompareArgs),
}

#[derive(Args)]
struct CompareArgs {
    /// Baseline image.
    base: PathBuf,
    /// Image compared against the baseline.
    head: PathBuf,
    /// Write the diff image to this path as PNG.
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Color delta a pixel must exceed to count, as a fraction in [0, 1].
    #[arg(long, default_value_t = 0.1)]
    threshold: f32,
    /// Count anti-aliasing differences instead of excluding them.
    #[arg(long)]
    no_antialiasing: bool,
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Compare(args) => run_compare(args),
    }
}

fn run_compare(args: CompareArgs) -> ExitCode {
    let opts = CompareOptions {
        threshold: args.threshold,
        detect_antialiasing: !args.no_antialiasing,
        ..Default::default()
    };

    match compare_files(&args.base, &args.head, &opts, args.output.as_deref()) {
        Ok(run) => {
            report(&run);
            ExitCode::from(exit_code(run.verdict) as u8)
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(error.exit_code() as u8)
        }
    }
}

fn report(run: &CompareRun) {
    match run.verdict {
        Verdict::Match => println!("match"),
        Verdict::Differ => println!(
            "differ: {} pixels ({:.4}%)",
            run.diff_pixels,
            run.diff_ratio * 100.0
        ),
        Verdict::SizeMismatch => println!("size mismatch"),
    }
}
