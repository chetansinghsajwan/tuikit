//! `mdexport file.md [-o out.pdf] [--base dir]`: a Markdown file (or `-` for stdin) as a PDF. `--typst` prints the Typst markup instead.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut typst = false;
    let mut base_dir: Option<PathBuf> = None;
    let mut args = std::env::args_os().skip(1);
    while let Some(a) = args.next() {
        match a.to_str() {
            Some("-o" | "--output") => output = args.next().map(PathBuf::from),
            Some("--typst") => typst = true,
            Some("--base") => base_dir = args.next().map(PathBuf::from),
            Some("-h" | "--help") => {
                println!("usage: mdexport <file.md> [-o out.pdf] [--base dir] [--typst]");
                return ExitCode::SUCCESS;
            }
            _ => input = Some(PathBuf::from(a)),
        }
    }
    let Some(input) = input else {
        eprintln!("usage: mdexport <file.md> [-o out.pdf] [--base dir] [--typst]");
        return ExitCode::from(2);
    };
    let stdin = input.as_os_str() == "-";
    let read = if stdin { std::io::read_to_string(std::io::stdin()) } else { std::fs::read_to_string(&input) };
    let text = match read {
        Ok(t) => t,
        Err(e) => {
            eprintln!("mdexport: {}: {e}", input.display());
            return ExitCode::FAILURE;
        }
    };
    let base = base_dir.or_else(|| input.parent().map(|p| if p.as_os_str().is_empty() { PathBuf::from(".") } else { p.to_path_buf() }));
    if typst {
        print!("{}", mdexport::typst_source(&text, base.as_deref()));
        return ExitCode::SUCCESS;
    }
    let out = output.unwrap_or_else(|| if stdin { PathBuf::from("out.pdf") } else { input.with_extension("pdf") });
    match mdexport::to_pdf(&text, &mdexport::Export { base }).and_then(|pdf| std::fs::write(&out, pdf).map_err(|e| e.to_string())) {
        Ok(()) => {
            println!("{}", out.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("mdexport: {e}");
            ExitCode::FAILURE
        }
    }
}
