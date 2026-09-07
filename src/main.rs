//! CLI: check a rule set, optionally evaluate it against a state, or emit the
//! build artifact Vimy embeds. `docs/implementation.md` has the rest.
use std::env;
use std::path::PathBuf;
use vimyc::check::check;
use vimyc::diag::{Diagnostic, Severity, SourceMap};
use vimyc::eval::evaluate;
use vimyc::state::State;
use vimyc::unit::{self, Unit};

fn main() {
    if let Err(e) = run() {
        eprintln!("vimyc: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    const USAGE: &str = "usage: vimyc <input>... [state.json] [--json|--vy] \
                         [--params <file>|-]\n       vimyc --tokens\n\n\
                         An input is a .vy file or a directory of them; several \
                         make one rule set.";

    // Explicit rather than scanning: `--params` with nothing after it used to
    // index past the end, and stray positional arguments vanished silently.
    let mut emit_json = false;
    let mut emit_vy = false;
    let mut list_tokens = false;
    let mut params_path: Option<String> = None;
    let mut positional: Vec<String> = Vec::new();
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            // stdout carries the artifact and nothing else, so it can be
            // redirected straight into a generated file.
            "--json" => emit_json = true,
            // The rule set as it stands after the doctrine, for reading.
            "--vy" => emit_vy = true,
            // The language describing itself, for a highlighter that cannot
            // fall behind it.
            "--tokens" => list_tokens = true,
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

    if list_tokens {
        // Sections rather than one list: a highlighter needs to know which is
        // which, and the operator order is load-bearing.
        use vimyc::token::TokenKind;
        println!("keyword {}", TokenKind::KEYWORDS.join(" "));
        println!("operator {}", TokenKind::OPERATORS.join(" "));
        println!("punctuation {}", TokenKind::PUNCTUATION.join(" "));
        return Ok(());
    }

    // Classified by extension rather than by position: a rule set is any number of
    // inputs now, so "the second positional is the state" no longer identifies
    // anything. A `.vy` file or a directory is source; the one other positional
    // is the state to evaluate against.
    let mut inputs: Vec<PathBuf> = Vec::new();
    let mut state_path: Option<String> = None;
    for arg in positional {
        let path = PathBuf::from(&arg);
        if path.is_dir() || path.extension().is_some_and(|e| e == "vy") {
            inputs.extend(unit::expand(&path).map_err(|e| e.to_string())?);
        } else if state_path.replace(arg.clone()).is_some() {
            return Err(format!("unexpected argument `{arg}`\n{USAGE}").into());
        }
    }
    if inputs.is_empty() {
        return Err(format!("no input file\n{USAGE}").into());
    }

    let unit = Unit::read(&inputs).map_err(|e| e.to_string())?;
    let Unit {
        ast,
        sources: src,
        diags,
    } = unit;
    report(&src, &diags);

    // Type errors after a syntax error are noise: the tree is full of holes the
    // parser already reported.
    if !diags.is_empty() {
        return Err(format!("{} error(s)", diags.len()).into());
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

    // Checking only: nothing was asked for that needs a doctrine, so stop
    // before binding rather than failing on the first unbound parameter. What
    // this cannot report is the warnings that compare priorities — a priority
    // is not a number until a doctrine sets it — so those need `--params`.
    if params_path.is_none() && !emit_json && !emit_vy && state_path.is_none() {
        return Ok(());
    }

    let supplied = match params_path.as_deref() {
        // `-` for stdin, so a caller compiling a doctrine per game window needs
        // no temporary file.
        Some("-") => {
            let mut json = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut json)
                .map_err(|e| format!("stdin: {e}"))?;
            serde_json::from_str(&json).map_err(|e| format!("stdin: {e}"))?
        }
        Some(p) => {
            let json = std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?;
            serde_json::from_str(&json).map_err(|e| format!("{p}: {e}"))?
        }
        None => std::collections::HashMap::new(),
    };
    let mut checked = checked;
    let params = vimyc::ir::ParamValues::bind(&checked.ir, &supplied)?;
    // Before anything reads the rule set, so `--json` and an evaluation see the
    // same rules.
    vimyc::specialise::specialise(&mut checked.ir, &params);
    // After specialising: these compare priorities, and a doctrine-set priority
    // is not a number until now.
    report(&src, &vimyc::specialise::validate(&checked.ir, &params));

    if emit_vy {
        println!("{}", vimyc::emit::vy::emit_file(&checked.ir, &params));
        return Ok(());
    }

    if emit_json {
        match vimyc::emit::emit(&checked.ir, &params, vimyc::emit::Target::Expr) {
            vimyc::emit::Artifact::Expr(rules) => {
                println!("{}", serde_json::to_string_pretty(&rules)?);
            }
            other => unreachable!("asked for expr, got {other:?}"),
        }
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

fn report(src: &SourceMap, diags: &[Diagnostic]) {
    for d in diags {
        let label = match d.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        match src.resolve(d.span.start) {
            Some((file, lc)) => eprintln!(
                "{}:{}:{}: {label}: {}",
                file.name, lc.line, lc.col, d.message
            ),
            // No file to point at means nothing was compiled, so this is a
            // diagnostic about the unit itself rather than about a line.
            None => eprintln!("{label}: {}", d.message),
        }
    }
}
