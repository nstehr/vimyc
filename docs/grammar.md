```bnf
; Lines beginning with `;` are comments on this grammar, not language syntax.
; The language itself has no comment form — see `because`.

<program>        ::= <rule>*

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

<priority>       ::= "priority" <integer>
<category>       ::= "category" <name> [ "exclusive" ]
<do>             ::= "do" <name>
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
                   | "exists"

; Whitespace separates tokens and is otherwise insignificant.
```
