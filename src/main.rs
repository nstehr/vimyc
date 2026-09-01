//! CLI. Currently lex + parse + dump; `docs/implementation.md` has the
//! subcommands it is headed for (`check`, `fmt`, `eval`).
use std::env;
use vimyc::ast::{Expr, ExprKind, Rule};
use vimyc::check::check;
use vimyc::diag::{Diagnostic, SourceFile};
use vimyc::lexer::lex;
use vimyc::parser::parse;

fn main() {
    if let Err(e) = run() {
        eprintln!("vimyc: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let mut tokens_only = false;
    let mut path = None;
    for a in &mut args {
        match a.as_str() {
            "--tokens" => tokens_only = true,
            _ => path = Some(a),
        }
    }
    let Some(path) = path else {
        return Err("no input file (usage: vimyc [--tokens] <file>)".into());
    };

    let text = std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
    let src = SourceFile::new(path, text);

    let (tokens, lex_diags) = lex(src.text());
    report(&src, &lex_diags);

    if tokens_only {
        for t in &tokens {
            let lc = src.line_column(t.span.start);
            println!("{:>3}:{:<3} {:?}", lc.line, lc.col, t.kind);
        }
        return finish(lex_diags.len());
    }

    // Parse even after lexing errors: they are recoverable, and one bad
    // character should not hide every problem in the rest of the file.
    let (ast, parse_diags) = parse(&tokens);
    report(&src, &parse_diags);

    // Only worth running on a tree that parsed: type errors off the back of a
    // syntax error are noise.
    let type_diags = if parse_diags.is_empty() {
        let d = check(&ast);
        report(&src, &d);
        d.len()
    } else {
        0
    };

    for rule in &ast.rules {
        print_rule(rule);
    }

    finish(lex_diags.len() + parse_diags.len() + type_diags)
}

fn report(src: &SourceFile, diags: &[Diagnostic]) {
    for d in diags {
        let lc = src.line_column(d.span.start);
        eprintln!("{}:{}:{}: {}", src.name, lc.line, lc.col, d.message);
    }
}

fn finish(errors: usize) -> Result<(), Box<dyn std::error::Error>> {
    if errors == 0 {
        Ok(())
    } else {
        Err(format!("{errors} error(s)").into())
    }
}

fn print_rule(r: &Rule) {
    let excl = if r.exclusive { " exclusive" } else { "" };
    println!("rule {}", r.name.text);
    println!("  priority {}", r.priority);
    println!("  category {}{}", r.category.text, excl);
    println!("  do       {}", r.action.text);
    if let Some(b) = &r.because {
        println!("  because  {b:?}");
    }
    for l in &r.lets {
        print!("  let      {} = ", l.name.text);
        print_expr(&l.value);
        println!();
    }
    for req in &r.requires {
        print!("  require  ");
        print_expr(req);
        println!();
    }
    println!();
}

/// A debugging view, not the formatter: fully parenthesised so precedence is
/// visible rather than implied. `fmt` is what prints canonical source.
fn print_expr(e: &Expr) {
    match &e.kind {
        ExprKind::Int(n) => print!("{n}"),
        ExprKind::Float(f) => print!("{f}"),
        ExprKind::Ident(n) => print!("{}", n.text),
        ExprKind::Call(n, args) => {
            print!("{}(", n.text);
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    print!(", ");
                }
                print_expr(a);
            }
            print!(")");
        }
        ExprKind::Unary(op, inner) => {
            print!("({op:?} ");
            print_expr(inner);
            print!(")");
        }
        ExprKind::Binary(op, l, r) => {
            print!("(");
            print_expr(l);
            print!(" {op:?} ");
            print_expr(r);
            print!(")");
        }
        ExprKind::Error => print!("<error>"),
    }
}
