# Implementation notes

How the compiler fits together, and the decisions that are expensive to reverse.

```
source ──lexer──> tokens ──parser──> ast ──check──> ir ──┬──eval──> bool
                                                         └──emit──> expr
```

| module | job |
|---|---|
| `diag` | spans, severities, source-mapped errors |
| `lexer` | text to tokens |
| `ast` | the tree |
| `parser` | tokens to tree, hand-written recursive descent |
| `env` | the predicate surface |
| `types` | the type lattice and enum domains |
| `check` | type checking, and the only route to an `Ir` |
| `ir` | names resolved to ids, bindings to slots |
| `lower` | `Ast → Ir`, crate-private |
| `eval` | tree-walking interpreter over the IR |
| `emit` | backends; `expr` is the first |
| `fmt` | canonical formatting |

All hand-written on purpose. The one dependency worth reaching for eventually is
`miette` or `ariadne` for diagnostic rendering, once the hand-rolled version gets
boring.

## Order to build in

`Span` → lexer → the rest of `diag` → ast → parser → check → eval → ir → lower
→ emit.

`diag` splits in two. `Span` is about ten lines and the lexer can't emit tokens
without it, so it comes first. The rendering half — `SourceFile`, `line_col`,
`Diagnostic`, the caret block — has no consumer until something produces an
error, and the lexer is the first producer (unexpected character, unterminated
string). Writing the renderer against real lexer errors is a much tighter loop
than eyeballing hand-constructed spans.

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

**`Copy`** — spans are read, passed and merged constantly. Two integers; copy
them rather than fight the borrow checker over something with no business being
borrowed.

**`u32`, not `usize`** — eight bytes instead of sixteen, and this rides on every
AST node. Rule files won't approach 4GB. rustc makes the same call.

**Byte offsets, not line/column** — line and column are a rendering concern,
computed from the source you already have. Storing them on every node duplicates
information and goes stale under any transform. Offsets also merge trivially.

**`to()` is the operation you use most** — a binary expression's span is
`lhs.span.to(rhs.span)`, a call's runs from the identifier to the closing paren.
Whole-expression spans fall out of their children.

Retrofitting spans in Rust is specifically painful. A field on every AST variant
means touching every construction site and every match arm; wrapping in
`Spanned<T>` infects every recursive position, so `Box<Expr>` becomes
`Box<Spanned<Expr>>` and every pattern grows a `.node`. Either way the compiler
makes you finish the whole refactor before anything builds.

### Rendering

To turn a span into line/column, `SourceFile` holds the source alongside a
precomputed table of line-start offsets and binary-searches it with
`slice::partition_point`.

**Count columns in characters, not bytes**, or the caret drifts the moment a
`because` string contains an em-dash. `&s[a..b]` panics on a non-char boundary,
which is the language telling you something true.

`.chars().count()` still isn't visual width — a combining accent is two chars and
one glyph, and CJK characters take two terminal columns. rustc reports char
columns too, so it's the right pragmatic answer; just expect drift under exotic
input.

### Line and column conventions

`SourceFile::line_column` returns a `LineColumn { line, col }` rather than a
`(u32, u32)`, so the two can't be swapped at a call site.

**Both fields are 1-based, and zero-based indices never leave the module.** The
`line_starts` table is indexed from zero internally, and the single `- 1` / `+ 1`
happens inside `line_column`. Anything public that takes a line number —
`line_text` — takes the 1-based one and converts internally. Two conventions
crossing the same boundary is where off-by-ones breed.

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
the language makes that structural via `require` — one conjunct per line, no
`and` node needed at the top level at all.

The conventional shape, used by both rustc and rust-analyzer:

```rust
pub struct Expr { pub kind: ExprKind, pub span: Span }

pub enum ExprKind {
    Int(i64),
    Float(f64),
    Ident(Name),
    Call(Name, Vec<Expr>),
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Error,                     // see error recovery
}
```

One place for the span, clean matching on `.kind`.

`and` and `or` inside a `require` are ordinary `Binary` nodes, nested rather than
flattened into an `And(Vec<Expr>)`. Flat n-ary nodes would make conjunct-level
analysis easier, but `require` already gives me conjunct identity where it
matters, and a uniform `Binary` keeps the parser's precedence chain and the
evaluator's match to one shape each. Worth revisiting if `any(...)` lands.

`Name` is a `String` plus a `Span`, not an interned `Symbol` — interning is a
`types` concern, and the parser has no table to intern against.

`Ident(name)` and `Call(name, args)` stay distinct — a bare zero-arg predicate is
not normalised into `Call(name, vec![])`. The tree stays faithful to what was
written, which matters when a caret is pointed at it.

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

Resist a parser-combinator crate. At this grammar size it's more code, and the
fight is with the library's type signatures rather than with Rust.

### Error recovery

Decide this before writing the first function. It's a shape rather than a
feature, and retrofitting means rewriting every parse method.

Don't return `Result<Ast, Error>` and stop at the first problem. Collect and
continue:

```
parse(src) -> (Ast, Vec<Diagnostic>)
```

On failure, record the diagnostic and resume. That's panic-mode recovery, and it
happens at two levels. Inside a rule body, an unrecognised token reports, bumps
one token and keeps reading fields — so one bad line doesn't cost the rest of the
rule. When the rule itself can't be salvaged, `recover` skips to the next `rule`
keyword, deliberately without an unconditional bump first, which would swallow
the very rule it's aiming for. `ExprKind::Error` stands in for whatever failed to
parse, and the type checker skips those nodes rather than cascading fresh errors
off them.

Two reasons, neither about parsing:

- A file with three typos should show three squiggles. Reporting only the first
  makes fixing a rule set a serial grind, which is what the current Sprintf layer
  already inflicts.
- Completion means parsing incomplete input. `has-role(` with the cursor after
  the paren is the normal state of a file being edited, not an error to report.
  The parser has to produce a usable tree with a hole in it, and the hole needs a
  span, or an editor integration can never know which parameter position the
  cursor is in.

The lexer takes the same shape: `(Vec<Token>, Vec<Diagnostic>)`.

## Evaluator

A tree-walking interpreter, and the oracle for any later backend. Keep it even if
a wasm backend appears — two independent implementations that must agree across
the whole corpus is a far better property test than reading bytecode.

Evaluation needs a mocked env: a trait the tests implement, holding cash, power,
unit and building counts, roles, queue state. Deriving those fixtures from
`testdata/` rather than hand-writing them is what makes the differential
milestone cheap.

## The IR

Name resolution used to happen three times. `check` resolves `cash` to
`Predicate::Cash`, decides which of three things `count(x)` means, and finds
which domain `powr` belongs to — then returned only `Vec<Diagnostic>` and threw
all of it away. `eval` resolved again at runtime, calling `env::predicate(name)`
per call, a linear scan of 64 signatures. An emitter would have been a third.

A lowered IR resolves once, and it is what makes a second backend cheap:

```text
source → lex → parse → check → lower → Ir → { eval, emit::expr, emit::wasm }
```

### Shape

```rust
pub struct IrRule {
    pub name: String,          // output-only; no reason to intern
    pub priority: i64,
    pub category: CategoryId,
    pub exclusive: bool,
    pub action: IrAction,      // resolved id, lowered args
    pub lets: Vec<IrExpr>,     // by slot; the name is gone
    pub requires: Vec<IrExpr>,
    pub span: Span,
}

pub enum IrExpr {
    Int(i64),
    Float(f64),
    Predicate(Predicate, Vec<IrExpr>),
    Member(Domain, u32),       // index into the table, not a string
    Binding(u32),              // slot, not a name
    Unary(UnOp, Box<IrExpr>),
    Binary(BinOp, Box<IrExpr>, Box<IrExpr>),
}
```

**No `Ident` and no `Error`.** That is the point, and the same move as
`ParamType` and `RuleChecker`: a backend cannot forget to handle an unresolved
name because there are none, and cannot be handed a tree that failed to check.

`Member(Domain, u32)` also hands a future wasm backend its ABI — every enum
literal is already an integer.

**Spans stay on `IrExpr`.** Backends do not diagnose, but blocked-on analysis
does: "which conjunct was false for 1,400 ticks" has to point at source, and
`eval::conjuncts` exists for exactly that.

### Decisions this forces

**`check` returns a `Result`.** Lowering can only run on a clean tree, so it is
the `Result` shape rather than the `(thing, Vec<Diagnostic>)` used elsewhere.
Parse can return a tree with holes; lowering cannot.

```
check(ast) -> Result<Checked, Vec<Diagnostic>>   // Checked { ir, warnings }
```

Warnings ride on the `Ok` side so they are not lost by succeeding, and the `Err`
side carries both kinds so a warning is not lost by an error appearing next to
it. `lower` is `pub(crate)`, which is what makes its panics sound: the only path
to an `Ir` runs through a check that passed, so "unresolved name here means a
compiler bug" is enforced rather than merely documented.

**Errors and warnings are separated by soundness, not by severity of
consequence.** An error means the rule set cannot be lowered. A warning means it
lowers to something that runs but is near-certainly wrong.

That puts both whole-set passes on the warning side. Priority collisions were
errors, and they are what found `vimy-axv` — but every real doctrine trips one,
so as errors they forced every consumer to filter by matching on the message
text. `real_rule_sets_check` did exactly that, and now says what it means:
`check` must succeed, and nothing may warn except a priority collision.

**`count` resolves at lowering.** It is the one overloaded name, and the IR
should carry `Predicate(BuildingCount, …)` or `Predicate(IdleGroundUnits, …)`
already decided. Nothing downstream should ever see a `count` node — it is the
clearest demonstration of what lowering is for.

### Selecting a backend

An enum, not `dyn Backend`:

```rust
pub enum Target { Expr, Wasm }
pub enum Artifact { Expr(Vec<String>), Wasm(Vec<u8>) }
```

The set is closed and known at compile time, so an enum beats a trait object: no
vtable, no lifetime friction, and exhaustiveness — adding a target becomes a
compile error everywhere it matters, the way `Predicate` did when `apply` refused
to build. A trait with an associated `Output` would abstract the wrong thing
anyway; the emitters share almost no interface, and what they actually share is
the resolution work the IR now does once.

### Sequence, and why the risk was low

1. `ir.rs` — types only
2. `lower.rs` — `Ast → Ir`, reusing the checker's resolution
3. Port `eval` to consume `Ir`. **The differential was the gate**: if lowering
   changed behaviour at all, 16,317 real evaluations would say so
4. `emit/expr.rs` — `Ir → Vec<RuleSource>`
5. Verify the emitter against Go

All five are done. Step 3 cost `eval` three helpers rather than adding any:
`eval_call`, `lookup` and `count` all resolved names that lowering had already
resolved, and the scope became a `Vec<Value>` indexed by slot instead of an
association list searched by name. The differential passed unchanged.

Step 5 landed as a stronger check than the one planned here. Rather than running
the emitted expr back through Go, the corpus now carries the conditions Go
compiled, and the test compares emitted source against them directly — which
reaches the rules the recorded states never make true, and in a real game that is
most of them. 3810 conditions across 49 rule sets.

Steps 1 to 3 carry no risk that cannot be detected. Step 4 is small. Step 5 is
where an unfaithful emitter would show up.

### Why expr before wasm

Emitting expr means Go's engine is unchanged, so everything already verified
stays verified. WASM's advantages turned out thinner than they first looked:
expr is already an expression evaluator with no I/O, so the sandbox is nearly
free already; evaluation is 88µs for 75 rules, so speed is irrelevant; cgo is
already required by the BAML client, so wazero buys no purity; and a wasm module
still calls host imports for all 64 predicates, so it is no more self-contained
than a string.

What remains is that it drops expr as a dependency and is an interesting piece of
engineering — which are honest reasons, just not urgent ones. Doing expr first
also means a wasm backend arrives with an oracle and a corpus already in place.

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
in. To check, append `compile_error!("x")` to the file and build — if it doesn't
fail, the file isn't in the tree.
