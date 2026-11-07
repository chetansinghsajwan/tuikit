//! `mdexport file.md [-o out.pdf] [options]`: a Markdown file (or `-` for stdin) as a PDF.
//!
//! Options: `--base dir` (where images are looked for), `--paper a4|letter`, `--margin cm`, `--font name`,
//! `--mono name`, `--size pt`, `--no-justify`, `--no-numbers`; `--typst` prints the Typst markup, `--fonts`
//! lists the usable font families, `--defaults` shows the settings as `key=value` lines.

use mdexport::{Export, Settings};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "usage: mdexport <file.md|-> [-o out.pdf] [--base dir] [--paper a4|letter] [--margin cm] [--font name] [--mono name] [--size pt] [--no-justify] [--no-numbers] [--typst] | --fonts | --defaults";

fn main() -> ExitCode {
    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut typst = false;
    let mut base_dir: Option<PathBuf> = None;
    let mut s = Settings::load();
    let mut args = std::env::args_os().skip(1);
    while let Some(a) = args.next() {
        let value = |args: &mut std::iter::Skip<std::env::ArgsOs>| {
            args.next().and_then(|v| v.into_string().ok())
        };
        match a.to_str() {
            Some("-o" | "--output") => output = args.next().map(PathBuf::from),
            Some("--typst") => typst = true,
            Some("--base") => base_dir = args.next().map(PathBuf::from),
            Some("--paper") => s.paper = value(&mut args).unwrap_or(s.paper),
            Some("--margin") => {
                s.margin_cm = value(&mut args).and_then(|v| v.parse().ok()).unwrap_or(s.margin_cm)
            }
            Some("--font") => s.font = value(&mut args).unwrap_or(s.font),
            Some("--mono") => s.mono = value(&mut args).unwrap_or(s.mono),
            Some("--size") => {
                s.size_pt = value(&mut args).and_then(|v| v.parse().ok()).unwrap_or(s.size_pt)
            }
            Some("--no-justify") => s.justify = false,
            Some("--no-numbers") => s.page_numbers = false,
            Some("--fonts") => {
                for f in mdexport::font_families() {
                    println!("{f}");
                }
                return ExitCode::SUCCESS;
            }
            Some("--defaults") => {
                print!("{}", s.lines());
                return ExitCode::SUCCESS;
            }
            Some("-h" | "--help") => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            _ => input = Some(PathBuf::from(a)),
        }
    }
    let Some(input) = input else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let stdin = input.as_os_str() == "-";
    let read = if stdin {
        std::io::read_to_string(std::io::stdin())
    } else {
        std::fs::read_to_string(&input)
    };
    let text = match read {
        Ok(t) => t,
        Err(e) => {
            eprintln!("mdexport: {}: {e}", input.display());
            return ExitCode::FAILURE;
        }
    };
    let base = base_dir.or_else(|| {
        input
            .parent()
            .map(|p| if p.as_os_str().is_empty() { PathBuf::from(".") } else { p.to_path_buf() })
    });
    if typst {
        print!("{}", mdexport::typst_source_with(&text, base.as_deref(), &s));
        return ExitCode::SUCCESS;
    }
    let out = output.unwrap_or_else(|| {
        if stdin { PathBuf::from("out.pdf") } else { input.with_extension("pdf") }
    });
    match mdexport::to_pdf(&text, &Export { base, settings: s })
        .and_then(|pdf| std::fs::write(&out, pdf).map_err(|e| e.to_string()))
    {
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
