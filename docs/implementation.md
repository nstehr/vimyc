# Implementation notes

How the compiler is put together, and the handful of decisions that are
expensive to reverse.

```
source ──lexer──> tokens ──parser──> ast ──types──> checked ast ──eval──> bool
```

| module | job |
|---|---|
| `diag` | spans, source-mapped errors |
| `lexer` | text to tokens |
| `ast` | the tree |
| `parser` | tokens to tree, hand-written recursive descent |
| `env` | the predicate surface |
| `types` | type checking |
| `eval` | tree-walking interpreter |
| `fmt` | canonical formatting |

Everything is hand-written on purpose. The one dependency worth reaching for
eventually is `miette` or `ariadne` for diagnostic rendering, and only once the
hand-rolled version gets boring.

## Order to build in

`Span` → lexer → the rest of `diag` → ast → parser → types → eval.

`diag` splits in two. `Span` itself is about ten lines and the lexer cannot emit
tokens without it, so that comes first. The rendering half — `SourceFile`,
`line_col`, `Diagnostic`, the caret block — has no consumer until something
produces an error, and the lexer is the first producer (unexpected character,
unterminated string). Writing the renderer against real lexer errors is a much
tighter loop than eyeballing hand-constructed spans.

## Spans

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Span { pub start: u32, pub end: u32 }

impl Span {
    pub fn to(self, other: Span) -> Span {
        Span { start: self.start, end: other.end }
    }
}
```

Four deliberate choices:

**`Copy`** — spans are read, passed and merged constantly. Two integers; copy
them rather than fighting the borrow checker over something with no business
being borrowed.

**`u32`, not `usize`** — eight bytes instead of sixteen, and this rides on every
AST node. Rule files will not approach 4GB. rustc makes the same call.

**Byte offsets, not line/column** — line and column are a rendering concern,
computed from the source you already have. Storing them on every node duplicates
information and goes stale under any transform. Offsets also merge trivially.

**`to()` is the operation you use most** — a binary expression's span is
`lhs.span.to(rhs.span)`, a call's runs from the identifier to the closing paren.
Whole-expression spans fall out of their children; you never compute one by hand.

Retrofitting spans is specifically painful in Rust. Adding a field to every AST
variant means touching every construction site and every match arm; wrapping in
`Spanned<T>` infects every recursive position so `Box<Expr>` becomes
`Box<Spanned<Expr>>` and every pattern grows a `.node`. Either way the compiler
makes you finish the whole refactor before anything builds again.

### Rendering

To turn a span into line/column, hold the source alongside a precomputed table of
line-start offsets and binary-search it. `slice::partition_point` is the idiomatic
call.

**Columns must be counted in characters, not bytes**, or the caret drifts the
moment a `because` string contains an em-dash. `&s[a..b]` panics on a non-char
boundary, which is the language telling you something true.

If this ever goes multi-file, `Span` grows a `FileId` and the source map holds
several files. Not worth designing for now.

The diagnostic the whole project exists to produce:

```
error: unknown role `war-facotry`
  --> seed.vy:14:22
   |
14 |   require has-role(war-facotry)
   |                    ^^^^^^^^^^^ did you mean `war-factory`?
```

## AST shape

Conditions are overwhelmingly a top-level conjunction of independent tests, and
the language makes that structural via `require`. Within a `require`, represent
`and`/`or` flatly — `And(Vec<Expr>)` rather than nested `BinOp` — for the same
reason: it matches how the rules are actually written and makes conjunct-level
analysis easy later.

The conventional shape, used by both rustc and rust-analyzer:

```rust
pub struct Expr { pub kind: ExprKind, pub span: Span }

pub enum ExprKind {
    Int(i64),
    EnumLit(Symbol),
    Call(Symbol, Vec<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Unary(UnOp, Box<Expr>),
    Error,                     // see error recovery
}
```

One place for the span, clean matching on `.kind`.

## Parser

Recursive descent, by hand. Precedence loosest to tightest:

```
or
and
== != < <= > >=
+ -
* /
unary not, unary -, exists
call, literal, parenthesised
```

Resist a parser-combinator crate. At this grammar size it is more code, and the
fight is with the library's type signatures rather than with Rust.

### Error recovery

Decide this before writing the first function — it is a shape rather than a
feature, and retrofitting means rewriting every parse method.

Do not return `Result<Ast, Error>` and stop at the first problem. Collect and
continue:

```
parse(src) -> (Ast, Vec<Diagnostic>)
```

On failure, record the diagnostic, skip forward to the next token that can
legally start a construct — `require`, `let`, `do`, `priority`, `category`,
`because`, `rule`, or `}` — and resume. That is panic-mode recovery and it is
enough. `ExprKind::Error` stands in for whatever failed to parse, and the type
checker skips those nodes rather than cascading fresh errors off them.

Two reasons, neither about parsing:

- A file with three typos should show three squiggles. Reporting only the first
  makes fixing a rule set a serial grind, which is exactly what the current
  Sprintf layer already inflicts.
- Completion means parsing incomplete input. `has-role(` with the cursor after
  the paren is the normal state of a file being edited, not an error to report.
  The parser has to produce a usable tree with a hole in it, and the hole needs a
  span, or an editor integration can never know which parameter position the
  cursor is in.

The lexer takes the same shape: `(Vec<Token>, Vec<Diagnostic>)`.

## Evaluator

A tree-walking interpreter, and the oracle for any later backend. Keep it even if
a wasm backend appears — two independent implementations that must agree on the
whole corpus is a far better property test than reading bytecode.

Evaluation needs a mocked env: a trait the tests implement, holding cash, power,
unit and building counts, roles, queue state. Deriving those fixtures from
`testdata/` rather than hand-writing them is what makes the differential
milestone cheap.

## Formatter

A pretty-printer over the AST: parse, discard the original text, print the tree.
No concrete syntax tree, no trivia preservation — the language has no comments, so
the AST carries everything the source carries and a plain printer is lossless.

Layout rules are in [design.md](design.md#formatting). v0 needs no line breaking,
so this is a recursive `print(node, indent, out)` on the order of 80 lines.

When breaking is eventually needed, the Wadler/Lindig document algebra:

```rust
enum Doc {
    Text(String),
    Line,                 // a space, or a newline if the enclosing group broke
    Nest(usize, Box<Doc>),
    Concat(Vec<Doc>),
    Group(Box<Doc>),      // fits flat? stay flat. otherwise break every Line within
}
```

Build a `Doc` from the AST, render against a width budget with a "does this group
fit" lookahead. About 150 lines, and a good exercise: a recursive enum, a tree
consumed by value, and a renderer the borrow checker has opinions about.

## CLI

```
vimyc check <file>          parse + type check, print diagnostics
vimyc fmt   <file>          rewrite canonically
vimyc fmt --check <file>    exit non-zero if not canonical
vimyc eval  <file> <state>  evaluate against a JSON env state
```

`fmt --check` costs nothing extra alongside `fmt` and is what lets CI assert
formatting without a second tool.

## A Rust gotcha worth remembering

A `.rs` file in `src/` that nothing declares is **silently ignored**, not an
error, and `cargo test` passing tells you nothing about whether a module is wired
in. To check, append `compile_error!("x")` to the file and build — if it does not
fail, the file is not in the tree.
