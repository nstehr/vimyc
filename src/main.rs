//! CLI. See `docs/implementation.md` for the intended subcommands.
use std::env;
use vimyc::{diag::SourceFile, lexer::lex};

fn main() {
    if let Err(e) = run() {
        eprintln!("vimyc: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let Some(path) = env::args().nth(1) else {
        return Err("no input file (usage: vimyc <file>)".into());
    };

    let text = std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
    let src = SourceFile::new(path, text);
    lex(src.text());
    Ok(())
}
