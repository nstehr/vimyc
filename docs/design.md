# Language design

Decisions and the reasoning behind them. Everything here was settled by
translating the 13 seed rules by hand — see [corpus.md](corpus.md) for the
numbers that back it up.

## A rule

```
rule build-power {
  priority 800
  category economy exclusive
  do       produce-power-plant

  require not queue-busy(Building)
  require can-build(Building, powr)
  require power-excess < 100 or count(powr) == 0
  require cash >= 300
}
```

From the expr it replaces:

```
!QueueBusy("Building") && CanBuild("Building","powr")
  && (PowerExcess() < 100 || BuildingCount("powr") == 0) && Cash() >= 300
```

## `require` instead of one big expression

All 13 seed rules are a flat AND of independent tests, with ORs only ever nested
inside parens. That is what production rules look like, so conjunction is
structural rather than an operator you happen to use a lot.

Each `require` is a conjunct. That buys three things: conjunct identity is free
and stable (useful for any later "why didn't this rule fire" analysis), adding a
condition is a one-line diff, and there is no question about where the `and`
goes when a line wraps.

The cost is that a genuinely top-level OR needs `require any(...)`. Given 13 out
of 13, that trade is fine.

## Rule fields

```
priority  i64        required
category  Symbol     required
exclusive bool       a modifier on `category`; absent means false
do        ActionId   required, exactly one
because   Option     optional
requires  Vec<Expr>  implicitly ANDed, one per line, no shorthand
lets      Vec<Let>   rule-scoped bindings
```

`exclusive` reads as a modifier — `category combat exclusive` — rather than as a
field. Eight of the thirteen seed rules are non-exclusive, so the common case
stays quiet.

Rule names and action names are separate namespaces and do collide in practice:
`deploy-mcv`, `produce-infantry`, `defend-base` and `repair-buildings` are each
both a rule and an action. `do` takes an action so resolution is unambiguous, but
the resolver needs two tables, not one.

## Names

Everything is kebab: `rule build-power`, `squad-exists(ground-defense)`,
`category squad-form`. That collides with `-` as subtraction, so:

> `-` continues an identifier only when immediately preceded by an identifier
> character **and** immediately followed by a letter, with no whitespace either
> side.

```
ground-defense            one identifier
lerp(350, 500, x) - 10    subtraction (whitespace)
size - 1                  subtraction
size-1                    subtraction (digit follows)
size-one                  one identifier   <- the confusable case
```

That last line is the footgun. It is survivable only because the type checker
knows every valid identifier: `size-one` is not a binding, a predicate or an enum
member, so it is a hard error with a span rather than silently wrong arithmetic.
Worth a special-cased diagnostic suggesting `size - one`.

In practice arithmetic in these rules is rare and always spaced, and the
formatter enforces spacing around binary operators, so the case barely arises.

Rule names alone would not need this — they appear only after `rule`, where
nothing else is legal, so a contextual lexer mode would do. The general rule
exists so squad names and categories, which appear in expression positions, can
look the same.

### Source spelling is not the wire format

Enum literals are interned, so what you type and what reaches Go are independent.
The Go side is inconsistent — rule and squad names are kebab (`build-power`,
`ground-defense`), categories are snake (`squad_form`, `air_combat`) — and those
strings are keys in `rule_firings.rule_name` and `rule_set_json`, so changing
them orphans the existing tuning history.

Keep the source uniformly kebab and hold one mapping table on emit. The
compatibility constraint is on emission, not on syntax.

## No comments

There is no comment syntax. `because "..."` is the documentation mechanism, and
it is a field rather than trivia.

A Go comment explaining why a rule exists does not survive into `rule_set_json`,
so the most valuable knowledge in `compiler_*.go` — the game 16 squad-poaching
fix, the game 64 cash-reservation starvation — is invisible to everything
downstream of the compiler. A field survives: into the archive, into a diff, into
whatever reads a rule set back later.

Consequence: the AST carries everything the source carries, so the formatter is a
plain pretty-printer with no trivia machinery.

Obligation: `because` has to actually carry that weight. Still open whether it
should be required, or whether a lint should flag a rule with a threshold nobody
explained.

## Predicates

A fraction of Go's `RuleEnv`, which has 108 methods. Many are action-only and
unusable in a condition — `ApproachWaypoint` returns `(int, int, bool)`. Choosing
the subset is design work, not transcription.

### v0 surface

Everything `DefaultRules()` uses, plus one deliberate addition.

| Go | notes |
|---|---|
| `Cash() int` | spelled `cash` |
| `PowerExcess() int` | spelled `power-excess` |
| `HasUnit(t) bool` | UnitType |
| `UnitCount(t) int` | spelled `count(e1)` |
| `HasBuilding(t) bool` | BuildingType |
| `BuildingCount(t) int` | spelled `count(powr)` |
| `HasRole(name) bool` | Role |
| `CanBuildRole(name) bool` | |
| `CanBuild(q, item) bool` | Queue + BuildingType |
| `QueueBusy(q) bool` | Queue |
| `QueueReady(q) bool` | |
| `BaseUnderAttack() bool` | |
| `EnemiesVisible() bool` | |
| `HasEnemyIntel() bool` | |
| `IdleGroundUnits() []Unit` | `count(idle-ground-units)` |
| `IdleHarvesters() []Unit` | `count(idle-harvesters)` |
| `DamagedBuildings() []Building` | `count(damaged-buildings)` |
| `NearestEnemy() *Enemy` | `exists nearest-enemy` |
| `SquadReadyRatio(name) float64` | **not used by seed** — see below |

### Collections never need to exist

Every collection-returning predicate in the corpus appears inside `len(...)`.
Expose a `count(...)` returning an integer and the language never needs a list
type, which also keeps a future wasm ABI to integers only.

### Pointers become options

`NearestEnemy() != nil` is the only shape pointer returns take. An option type
that cannot be used where a bool is expected is stricter than what expr allows.

### `count` is generic

One spelling covers what Go splits three ways:

```
count(powr)                BuildingType -> BuildingCount("powr")
count(e1)                  UnitType     -> UnitCount("e1")
count(idle-ground-units)   collection   -> len(IdleGroundUnits())
```

All three answer "how many of this do I have", so one name is honest rather than
merely shorter.

The cost: completion inside `count(` cannot be scoped the way `has-role(` can. It
has to offer buildings, units and collections together, and an unknown-identifier
suggestion searches all three.

### Why `SquadReadyRatio` is in v0

The 13 seed rules contain no float literals and call nothing returning a float, so
a v0 built strictly to them would never exercise the `f64` path — not in the type
checker, not in the interpreter, not across a wasm host boundary. The squad rules
`CompileDoctrine` emits lean on it heavily (`SquadReadyRatio("ground-attack") >=
0.7`), so the gap would surface immediately on widening, after the vertical had
already been declared working.

Semantics, from `env.go:1340`: idle members over available members, where
available excludes units currently retreating. Returns exactly `0.0` for an
unknown squad, an empty squad, or one whose members are all retreating — so a
caller cannot distinguish "no squad" from "squad ready 0%". Preserve that rather
than improving it. Matching expr is the point, and the differential test will hold
you to it.

It also reads `Memory`, which the engine mutates between rules as actions fire.
Nothing in the seed set does that, so it is the first predicate that would force a
shadow harness to interleave with the Go loop rather than run as a separate pass.

### The enums

Queue, Role, BuildingType, UnitType, SquadName. 147 distinct literals across the
full corpus. Interning them is what turns `has-role("war_facotry")` from a silent
no-op into a compile error — the single highest-value thing in the project.

Queue literals are capitalised (`Building`, `Infantry`) and everything else is
lowercase kebab. That is an inconsistency, kept because it makes
`can-build(Building, powr)` visibly two different domains.

## Types

Small: `Int`, `Float`, `Bool`, the domain enums, and `Option<T>`. What matters is
what becomes an error that expr accepts today:

- an enum literal that is not a member (`has-role(war-facotry)`)
- an enum of the wrong domain (a UnitType where a Role belongs)
- an option used as a bool
- comparing values of different types
- arity or argument-type mismatch on a predicate
- an ambiguous `count(...)` argument

### Resolving `count`

Look the identifier up across BuildingType, UnitType and the collections. Exactly
one hit resolves the call. Zero hits is an unknown-identifier error whose
suggestions span all three. **Two or more hits is an ambiguity error, never a
silent pick** — the whole point of the language is that a name meaning something
unintended gets caught rather than shipped.

No collision exists today (`fact`/`powr`/`proc`/`weap` versus `e1`/`mcv`), so that
path will be untested unless a fixture forces it. Write the fixture.

### Later: whole-rule-set checks

These need the full set rather than one condition, and they are the checks Go
structurally cannot do:

- `squad-exists(X)` with no reachable `form-squad(X, ...)` — currently held
  together only by the two rules being adjacent in one `if` block
- a rule shadowed by a higher-priority exclusive rule in the same category
- priority collisions within a category (ordering is nondeterministic today)
- guards on if/else rule pairs that do not partition (`defend-base` versus
  `form-defense-squad` + `squad-defend-base`)

## Formatting

```
rule build-power {
  priority 800
  category economy exclusive
  do       produce-power-plant

  require not queue-busy(Building)
  require can-build(Building, powr)
  require power-excess < 100 or count(powr) == 0
  require cash >= 300
}
```

Two-space indent; field block, blank line, requires; one blank line between
rules; mandatory spacing around binary operators.

Field values are column-aligned. The field keywords are a closed set that all fit
in eight columns (`priority`, `category`, `do`, `because`), so alignment is
stable — a user cannot trigger a reflow without a grammar change. `require` and
`let` are statements and stay outside the alignment group.

Beyond looking tidy, the formatter closes the kebab footgun by enforcing spacing
around binary operators, and it is what would make rule-set content hashing
stable if that is ever wanted for comparing strategies across games.

### v0 needs no line breaking

37 conjuncts across the seed rules: shortest 13 characters, median 20, longest 51.
None exceed 60. One `require` per line, never wrap.

The doctrine-compiled rules reach 575 characters, so breaking arrives eventually.
Reach for the Wadler/Lindig document algebra then, not before.

## Deliberately out of scope for v0

`reserve` declarations, doctrine parameter declarations, rule-level guards
(`rule x when tech-priority > 0.4`), `any(...)`, wasm codegen, actions with
arguments, LLM authoring. All known-needed later, none needed to take 13 rules
through to a working interpreter.
