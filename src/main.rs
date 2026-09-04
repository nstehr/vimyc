//! CLI: check a rule set, optionally evaluate it against a state, or emit the
//! build artifact Vimy embeds. `docs/implementation.md` has the rest.
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
    const USAGE: &str = "usage: vimyc <file> [state.json] [--json] [--params <file>]";

    // Explicit rather than scanning for flags: `--params` with nothing after it
    // used to index past the end, and stray positional arguments were dropped
    // without a word.
    let mut emit_json = false;
    let mut params_path: Option<String> = None;
    let mut positional: Vec<String> = Vec::new();
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            // Emitting is silent on stdout apart from the artifact, so it can be
            // redirected straight into a generated file.
            "--json" => emit_json = true,
            // A flat object of parameter name to number.
            "--params" => {
                params_path = Some(
                    args.next()
                        .ok_or(format!("--params needs a file\n{USAGE}"))?,
                )
            }
            _ if arg.starts_with("--") => {
                return Err(format!("unknown flag `{arg}`\n{USAGE}").into());
            }
            _ => positional.push(arg),
        }
    }

    let mut positional = positional.into_iter();
    let Some(path) = positional.next() else {
        return Err(format!("no input file\n{USAGE}").into());
    };
    let state_path = positional.next();
    if let Some(extra) = positional.next() {
        return Err(format!("unexpected argument `{extra}`\n{USAGE}").into());
    }

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

    let supplied = match &params_path {
        Some(p) => {
            let json = std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?;
            serde_json::from_str(&json).map_err(|e| format!("{p}: {e}"))?
        }
        None => std::collections::HashMap::new(),
    };
    let mut checked = checked;
    let params = vimyc::ir::ParamValues::bind(&checked.ir, &supplied)?;
    // Applied before anything reads the rule set, so `--json` and an evaluation
    // see the same rules the doctrine actually leaves behind.
    vimyc::specialise::specialise(&mut checked.ir, &params);
    // After specialising, not before: these compare priorities, and a
    // doctrine-set priority is not a number until now.
    report(&src, &vimyc::specialise::validate(&checked.ir, &params));

    if emit_json {
        let vimyc::emit::Artifact::Expr(rules) =
            vimyc::emit::emit(&checked.ir, &params, vimyc::emit::Target::Expr);
        println!("{}", serde_json::to_string_pretty(&rules)?);
        return Ok(());
    }

    if let Some(state_path) = state_path {
        let json =
            std::fs::read_to_string(&state_path).map_err(|e| format!("{state_path}: {e}"))?;
        let state: State = serde_json::from_str(&json).map_err(|e| format!("{state_path}: {e}"))?;
        for rule in evaluate(&checked.ir, &params, &state).fired {
            println!(
                "{:>5}  {:<20} {}",
                vimyc::eval::priority(rule, &params),
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
