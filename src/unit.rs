//! Several files, one compilation.
//!
//! A rule set is written across `core.vy`, `combat.vy`, `economy.vy` and the
//! rest, but there is no such thing as checking one of them: a `def` in one
//! file is called from another, and a `param` is only meaningful against the
//! doctrine the whole set is compiled with. So the unit — not the file — is
//! what parses, checks and emits.
//!
//! Each file is lexed with its spans shifted into the unit's coordinate space
//! (see `diag::SourceMap`) and parsed on its own, then the trees are
//! concatenated. Parsing per file rather than concatenating the text first
//! keeps one file's unclosed brace from swallowing the next one's rules.

use crate::ast::Ast;
use crate::diag::{Diagnostic, SourceMap};
use std::path::{Path, PathBuf};

/// A rule set assembled from its files, before checking.
pub struct Unit {
    pub ast: Ast,
    pub sources: SourceMap,
    /// Everything the lexer and parser found, across all files.
    pub diags: Vec<Diagnostic>,
}

impl Unit {
    /// Assembles a unit from named sources, in the order given.
    ///
    /// Order decides only what a diagnostic list looks like: defs resolve across
    /// the whole unit regardless of which file they are in, so no file depends
    /// on being passed first.
    pub fn parse(files: impl IntoIterator<Item = (String, String)>) -> Unit {
        let mut sources = SourceMap::new();
        let mut ast = Ast {
            params: Vec::new(),
            defs: Vec::new(),
            rules: Vec::new(),
        };
        let mut diags = Vec::new();

        for (name, text) in files {
            let base = sources.add(name, text);
            // `sources` owns the text now, so read it back rather than keeping
            // a second copy alive.
            let text = sources
                .files()
                .last()
                .expect("just added")
                .text()
                .to_string();

            let (tokens, lex_diags) = crate::lexer::lex_at(&text, base);
            diags.extend(lex_diags);
            // Parsed even after a lexing error: one bad character should not
            // hide every problem after it.
            let (file_ast, parse_diags) = crate::parser::parse(&tokens);
            diags.extend(parse_diags);

            ast.params.extend(file_ast.params);
            ast.defs.extend(file_ast.defs);
            ast.rules.extend(file_ast.rules);
        }

        Unit {
            ast,
            sources,
            diags,
        }
    }

    /// Reads a unit from disk.
    pub fn read(paths: &[PathBuf]) -> std::io::Result<Unit> {
        let mut files = Vec::with_capacity(paths.len());
        for p in paths {
            let text = std::fs::read_to_string(p)
                .map_err(|e| std::io::Error::new(e.kind(), format!("{}: {e}", p.display())))?;
            files.push((p.display().to_string(), text));
        }
        Ok(Unit::parse(files))
    }
}

/// Expands one command-line input into the files it names.
///
/// A directory is its `.vy` files, sorted — sorted rather than in readdir order
/// so that a diagnostic list, and the rule order inside an emitted artifact, do
/// not depend on the filesystem. Nested directories are left alone: a rule set
/// is a flat set of topics, and recursing would quietly pull in a scratch copy
/// someone left in a subdirectory.
pub fn expand(input: &Path) -> std::io::Result<Vec<PathBuf>> {
    if !input.is_dir() {
        return Ok(vec![input.to_path_buf()]);
    }
    let mut found: Vec<PathBuf> = std::fs::read_dir(input)
        .map_err(|e| std::io::Error::new(e.kind(), format!("{}: {e}", input.display())))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "vy"))
        .collect();
    found.sort();
    if found.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{}: no .vy files", input.display()),
        ));
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(files: &[(&str, &str)]) -> Unit {
        Unit::parse(
            files
                .iter()
                .map(|(n, t)| (n.to_string(), t.to_string()))
                .collect::<Vec<_>>(),
        )
    }

    const RULE: &str = "rule scout-now {\n  priority 300\n  category recon\n  do scout\n  \
                        require cash >= 100\n}";

    #[test]
    fn merges_the_items_of_every_file() {
        let u = unit(&[
            (
                "a.vy",
                "param aggression: float\ndef twice(n: int) = n * 2\n",
            ),
            ("b.vy", RULE),
        ]);
        assert!(u.diags.is_empty(), "{:?}", u.diags);
        assert_eq!(u.ast.params.len(), 1);
        assert_eq!(u.ast.defs.len(), 1);
        assert_eq!(u.ast.rules.len(), 1);
    }

    #[test]
    fn a_span_resolves_to_the_file_it_came_from() {
        // The second file's rule must not report against the first.
        let u = unit(&[
            ("first.vy", "param aggression: float\n"),
            ("second.vy", RULE),
        ]);
        let span = u.ast.rules[0].name.span;
        let (file, lc) = u.sources.resolve(span.start).expect("resolves");
        assert_eq!(file.name, "second.vy");
        assert_eq!((lc.line, lc.col), (1, 6));
    }

    #[test]
    fn a_diagnostic_resolves_to_the_file_it_came_from() {
        let u = unit(&[("ok.vy", "param aggression: float\n"), ("bad.vy", "rule {")]);
        let d = u.diags.first().expect("a parse error");
        let (file, _) = u.sources.resolve(d.span.start).expect("resolves");
        assert_eq!(file.name, "bad.vy");
    }

    #[test]
    fn an_unterminated_file_does_not_swallow_the_next() {
        // Files parse separately, so `open.vy` losing its brace still leaves
        // `closed.vy`'s rule standing.
        let u = unit(&[
            ("open.vy", "rule half {\n  priority 300\n"),
            ("closed.vy", RULE),
        ]);
        assert!(!u.diags.is_empty(), "the unclosed rule should be reported");
        assert!(
            u.ast.rules.iter().any(|r| r.name.text == "scout-now"),
            "the second file's rule survived"
        );
    }

    #[test]
    fn the_first_byte_of_the_first_file_resolves() {
        let u = unit(&[("only.vy", RULE)]);
        let (file, lc) = u.sources.resolve(0).expect("resolves");
        assert_eq!(file.name, "only.vy");
        assert_eq!((lc.line, lc.col), (1, 1));
    }

    #[test]
    fn an_end_of_file_span_stays_in_its_own_file() {
        // The separator byte: an Eof span sits one past the last byte, and must
        // not be read as the first byte of the file after it.
        let a = "param aggression: float\n";
        let u = unit(&[("a.vy", a), ("b.vy", RULE)]);
        let (file, _) = u.sources.resolve(a.len() as u32).expect("resolves");
        assert_eq!(file.name, "a.vy");
    }
}
