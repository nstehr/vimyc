```bnf
; Lines beginning with `;` are comments on this grammar, not language syntax.
; The language itself has no comment form — see `because`.

<program>        ::= ( <param> | <def> | <rule> )*

; A named expression, inlined at its call sites. Only earlier defs are in
; scope, so nothing here can recurse. See docs/design.md, "def".
<def>            ::= "def" <name> "(" [ <def-params> ] ")" "=" <expr>
<def-params>     ::= <def-param> ( "," <def-param> )*
<def-param>      ::= <name> ":" <param-type>

; A doctrine input: file-scoped, constant within a doctrine window, supplied
; from outside the file. See docs/design.md, "Parameters".
<param>          ::= "param" <name> ":" <param-type>
<param-type>     ::= "int" | "float"

<rule>           ::= "rule" <name> "{" <rule-body> "}"

; Items may appear in any order. `priority`, `category` and `do` are each
; required exactly once; `because` at most once. `let` and `require` repeat.
<rule-body>      ::= <rule-item>*

<rule-item>      ::= <priority>
                   | <category>
                   | <do>
                   | <because>
                   | <let>
                   | <require>

; An expression so a doctrine can set it, but one the type checker restricts to
; parameters, literals and `lerp` — the engine sorts on priority, so it must be
; decidable before the first tick rather than from game state.
<priority>       ::= "priority" <expr>
<category>       ::= "category" <name> [ "exclusive" ]
; An action takes arguments when it is built by a factory —
; `form-squad(ground-attack, Ground, 8, Attack)`. Whether a given action takes
; any is settled by the type checker, not here.
<do>             ::= "do" <action>
<action>         ::= <name> [ "(" [ <arg-list> ] ")" ]
<because>        ::= "because" <string>

; A binding may not shadow a predicate name.
<let>            ::= "let" <name> "=" <expr>

; Each `require` is one conjunct; a rule's requires are implicitly ANDed.
<require>        ::= "require" <expr>

; Operator precedence is loosest-first down this chain; all binary
; operators are left-associative.
<expr>           ::= <or-expr>

<or-expr>        ::= <and-expr> ( "or" <and-expr> )*

<and-expr>       ::= <cmp-expr> ( "and" <cmp-expr> )*

; One level, so `a < b < c` parses. The type checker rejects it: ordering
; requires numbers where equality does not.
<cmp-expr>       ::= <add-expr> ( <cmp-op> <add-expr> )*

<add-expr>       ::= <mul-expr> ( <add-op> <mul-expr> )*

<mul-expr>       ::= <unary-expr> ( <mul-op> <unary-expr> )*

<unary-expr>     ::= <unary-op> <unary-expr>
                   | <primary>

<primary>        ::= <integer>
                   | <float>
                   | <call>
                   | <name>
                   | "(" <expr> ")"

; A `(` immediately following a name makes it a call; otherwise the name
; stands alone. Whether a bare name is a predicate, a binding or an enum
; literal is settled by the type checker, not here.
<call>           ::= <name> "(" [ <arg-list> ] ")"

<arg-list>       ::= <expr> ( "," <expr> )* [ "," ]

<cmp-op>         ::= "==" | "!=" | "<" | "<=" | ">" | ">="
<add-op>         ::= "+" | "-"
<mul-op>         ::= "*" | "/"
<unary-op>       ::= "not" | "-" | "exists"

; An identifier that matches a keyword is lexed as that keyword, so keywords
; are not available as names.
<name>           ::= <identifier>

; A `-` continues an identifier only when immediately followed by a letter,
; with no whitespace on either side; otherwise it is subtraction. So
; `size-one` is one identifier, `size-1` and `size - one` are subtraction.
<identifier>     ::= <letter> <ident-rest>*
<ident-rest>     ::= <letter>
                   | <digit>
                   | "-" <letter>

; No sign: a leading `-` is the unary operator. A `.` is part of a float only
; when a digit follows it.
<integer>        ::= <digit>+
<float>          ::= <digit>+ "." <digit>+

; No escape sequences; a string ends at the next `"`.
<string>         ::= '"' <string-char>* '"'
<string-char>    ::= any character except '"'

; ASCII only. Identifiers may not start with a digit or `-`.
<letter>         ::= "a" ... "z" | "A" ... "Z"
<digit>          ::= "0" ... "9"

<keyword>        ::= "rule" | "priority" | "category" | "exclusive" | "do"
                   | "require" | "because" | "let" | "and" | "or" | "not"
                   | "exists" | "param" | "def" | "int" | "float"

; Builtins are ordinary call syntax, not keywords: `lerp`, `lerpf`, `max`,
; `min`, `trunc`, `select`. They are rejected as binding and parameter names by
; the checker, not by the grammar.

; Whitespace separates tokens and is otherwise insignificant.
```

## Keeping this current

The grammar is hand-written, so nothing enforces that it matches `parser.rs`.
Changes that belong here:

- a new keyword, which also means a new `TokenKind::keyword` entry
- a new token, operator or precedence level
- a change to what a construct may contain — `do` gaining arguments was one

`rules/grammar.vy` exercises every production, and
`the_documented_grammar_parses_and_checks` keeps it honest: if a construct stops
parsing or stops type checking, the file or the parser has drifted. It cannot
catch a production the grammar describes and the parser never had, so adding a
shape here means adding it there too.

Changes that do **not** belong here, because they are about meaning rather than
shape: the predicate and action tables, which domains exist, what types an
operator accepts. `a < b < c` parses and is then rejected; that rejection is
`docs/design.md`'s business.
