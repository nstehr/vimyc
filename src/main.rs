//! CLI. Currently `check` only; `docs/implementation.md` has the subcommands it
//! is headed for (`fmt`, `eval`).
use std::env;
use vimyc::check::check;
use vimyc::diag::{Diagnostic, Severity, SourceFile};
use vimyc::eval::evaluate;
use vimyc::lexer::lex;
use vimyc::parser::parse;
use vimyc::state::State;

fn main() {
    if let Err(e) = run() {
        eprintln!("vimyc: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let Some(path) = args.next() else {
        return Err("no input file (usage: vimyc <file> [state.json])".into());
    };
    let state_path = args.next();

    let text = std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
    let src = SourceFile::new(path, text);

    let (tokens, lex_diags) = lex(src.text());
    report(&src, &lex_diags);

    // Parsed even after a lexing error: one bad character should not hide every
    // problem after it.
    let (ast, parse_diags) = parse(&tokens);
    report(&src, &parse_diags);

    // Type errors off the back of a syntax error are noise — the tree is full of
    // holes the parser already reported.
    let errors = lex_diags.len() + parse_diags.len();
    if errors > 0 {
        return Err(format!("{errors} error(s)").into());
    }

    // The only way to an `Ir`, so nothing below can run on a rule set that did
    // not check.
    let checked = match check(&ast) {
        Ok(c) => c,
        Err(diags) => {
            report(&src, &diags);
            let errors = diags.iter().filter(|d| d.is_error()).count();
            return Err(format!("{errors} error(s)").into());
        }
    };
    report(&src, &checked.warnings);

    if let Some(state_path) = state_path {
        let json =
            std::fs::read_to_string(&state_path).map_err(|e| format!("{state_path}: {e}"))?;
        let state: State = serde_json::from_str(&json).map_err(|e| format!("{state_path}: {e}"))?;
        for rule in evaluate(&checked.ir, &state).fired {
            println!(
                "{:>5}  {:<20} {}",
                rule.priority,
                vimyc::env::category_name(rule.category.0),
                rule.name
            );
        }
    }

    Ok(())
}

fn report(src: &SourceFile, diags: &[Diagnostic]) {
    for d in diags {
        let lc = src.line_column(d.span.start);
        let label = match d.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        eprintln!(
            "{}:{}:{}: {label}: {}",
            src.name, lc.line, lc.col, d.message
        );
    }
}
