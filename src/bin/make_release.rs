//! Local release review/package helper. Never changes versions, git or tags.
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        args.push("--dry-run".to_string());
    }
    match Command::new("python3")
        .arg(".github/package_release.py")
        .args(args)
        .status()
    {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(_) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("Could not run release packager (Python 3.11+ required): {error}");
            ExitCode::FAILURE
        }
    }
}
