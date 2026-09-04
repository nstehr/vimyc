# Language design

Decisions and why. I settled all of this by translating the 13 seed rules by
hand — see [corpus.md](corpus.md) for the numbers behind it.

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

Replacing this expr:

```
!QueueBusy("Building") && CanBuild("Building","powr")
  && (PowerExcess() < 100 || BuildingCount("powr") == 0) && Cash() >= 300
```

## `require` instead of one big expression

All 13 seed rules are a flat AND of independent tests, with ORs only nested
inside parens. That's what production rules look like, so conjunction should be
structural rather than an operator you happen to use a lot.

Each `require` is one conjunct. Conjunct identity comes for free and stays stable
(useful for a later "why didn't this rule fire"), adding a condition is a one-line
diff, and there's no question about where the `and` goes when a line wraps.

The cost is that a real top-level OR needs `require any(...)`. Given 13 out of 13,
I'll take it.

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

`exclusive` reads as a modifier (`category combat exclusive`) rather than a
field. Eight of the thirteen seed rules aren't exclusive, so the common case
stays quiet.

Rule names and action names are separate namespaces and they do collide:
`deploy-mcv`, `produce-infantry`, `defend-base` and `repair-buildings` are each
both a rule and an action. `do` takes an action, so resolution is unambiguous,
but the resolver needs two tables.

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

That last line is the footgun. It survives because the type checker knows every
valid identifier: `size-one` isn't a binding, a predicate or an enum member, so
it's a hard error with a span rather than silently wrong arithmetic. Worth a
special-cased diagnostic suggesting `size - one`.

In practice arithmetic here is rare and always spaced, and the formatter enforces
spacing around binary operators. Of the 1,932 condition forms `CompileDoctrine`
emits, nine contain arithmetic and subtraction appears exactly once, as
`AircraftCapacity() - 1` — a call minus a literal, not confusable even unspaced
since `)` isn't an identifier character. Zero instances of
identifier-minus-identifier.

So this is a forward-looking risk. It arrives with `let`, which expr has no
equivalent of, and which is the first construct that can put two bare identifiers
either side of a `-`.

I considered a stricter rule — drop the "followed by a letter" clause so
whitespace alone decides, making `size-1` an unknown identifier. Symmetric and
easier to explain, but it buys nothing the formatter and type checker don't
already cover.

Rule names alone wouldn't need any of this; they only appear after `rule`, where
nothing else is legal. The general rule exists so squad names and categories,
which show up in expression positions, can look the same.

### Go adapts to the language, not the reverse

The language defines its own vocabulary, uniformly kebab. There is **no mapping
table in `env.rs`**. An earlier version derived Go's spellings and it was
removed: nothing emits yet, and it made the language carry a consumer's
historical inconsistency.

The Go side is inconsistent today — rule and squad names are kebab
(`build-power`, `ground-defense`), roles and categories are snake
(`war_factory`, `squad_form`), predicates are PascalCase Go methods.
Reconciling that is Go's problem, in three passes:

1. **Now** — change what is free. Measured against `vimy-core`: 19 category
   strings contain underscores (not persisted anywhere; 16 test assertions
   reference three of them), and 56 `ActionRegistry` keys — a map that is
   *declared and never read* in the entire repo. Squad names are already kebab.
2. **Next** — build vimyc without accommodating any of it.
3. **Later** — once vimyc actually emits, align the rest against a real
   consumer, when it is testable rather than speculative.

Two things stay unreconciled by design. OpenRA's actor and queue names (`powr`,
`e1`, `Building`) are not Vimy's to rename. And roles are entangled well beyond
the rule engine: persisted in `doctrine_json` as `preferred_infantry` arrays,
written by hand into BAML prompt text and few-shot examples, and fed back to the
LLM by `QueryExemplarDoctrines` — so renaming them would leave the exemplar
history disagreeing with the instructions.

## No comments

There's no comment syntax. `because "..."` is the documentation mechanism, and
it's a field rather than trivia.

A Go comment explaining why a rule exists doesn't survive into `rule_set_json`,
so the most valuable knowledge in `compiler_*.go` — the game 16 squad-poaching
fix, the game 64 cash-reservation starvation — is invisible to everything
downstream of the compiler. A field survives: into the archive, into a diff, into
whatever reads a rule set back later.

Two consequences. The AST carries everything the source carries, so the formatter
is a plain pretty-printer with no trivia machinery. And `because` has to actually
carry that weight — still open whether it should be required, or whether a lint
should flag a rule with a threshold nobody explained.

## Predicates

A fraction of Go's `RuleEnv`, which has 108 methods. Many are action-only and
unusable in a condition — `ApproachWaypoint` returns `(int, int, bool)`. Picking
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
that can't be used where a bool is expected is stricter than what expr allows.

### `count` is generic

One spelling covers what Go splits three ways:

```
count(powr)                BuildingType -> BuildingCount("powr")
count(e1)                  UnitType     -> UnitCount("e1")
count(idle-ground-units)   collection   -> len(IdleGroundUnits())
```

All three answer "how many of this do I have", so one name is honest rather than
just shorter.

The cost: completion inside `count(` can't be scoped the way `has-role(` can. It
has to offer buildings, units and collections together, and an unknown-identifier
suggestion searches all three.

### Why `SquadReadyRatio` is in v0

The 13 seed rules contain no float literals and call nothing returning a float,
so a v0 built strictly to them would never exercise the `f64` path — not in the
type checker, not in the interpreter, not across a wasm host boundary. The squad
rules `CompileDoctrine` emits lean on it heavily
(`SquadReadyRatio("ground-attack") >= 0.7`), so the gap would surface the moment I
widened, after the vertical had already been called working.

Semantics, from `env.go:1340`: idle members over available members, where
available excludes units currently retreating. Returns exactly `0.0` for an
unknown squad, an empty squad, or one whose members are all retreating — so a
caller can't distinguish "no squad" from "squad ready 0%". Preserve that rather
than improving it. Matching expr is the point, and the differential test will
hold me to it.

It also reads `Memory`, which the engine mutates between rules as actions fire.
Nothing in the seed set does that, so it's the first predicate that would force a
shadow harness to interleave with the Go loop rather than run as a separate pass.

### The enums

Queue, Role, BuildingType, UnitType, SquadName. 147 distinct literals across the
full corpus. Interning them is what turns `has-role("war_facotry")` from a silent
no-op into a compile error — the single highest-value thing in the project.

Queue literals are capitalised (`Building`, `Infantry`) and everything else is
lowercase kebab. An inconsistency I kept, because it makes
`can-build(Building, powr)` visibly two different domains.

## Types

Small: `Int`, `Float`, `Bool`, the domain enums, and `Option<T>`. What matters is
what becomes an error that expr accepts today:

- an enum literal that isn't a member (`has-role(war-facotry)`)
- an enum of the wrong domain (a UnitType where a Role belongs)
- an option used as a bool
- comparing values of different types
- arity or argument-type mismatch on a predicate
- an ambiguous `count(...)` argument

### Classifying identifiers

Only keywords and definition names are positional. Everything else is a bare
identifier the type checker sorts out:

| class | example | resolved by |
|---|---|---|
| keyword | `rule`, `require`, `and` | lexer, via `TokenKind::keyword` |
| predicate | `queue-busy`, `cash`, `count` | type checker, against the env table |
| enum literal | `Building`, `powr`, `barracks` | type checker, by argument position |
| binding | `size`, `surplus` | type checker, rule scope |
| definition name | `build-power`, `economy`, `produce-power-plant` | parser, by position |

Predicates are deliberately **not** lexed as their own token kind. Keywords are
grammar — they change how the parser reads what follows. Predicates are data. As
tokens, adding a predicate would mean editing the lexer, and a typo'd predicate
would be a syntax error rather than a type error with a suggestion, which is the
whole point of the language.

**Bindings may not shadow predicates.** `let cash = 5` is an error. A rule where
`cash` means something other than cash is exactly the confusion this language
exists to prevent, and rejecting it costs one lookup.

### Resolving `count`

Look the identifier up across BuildingType, UnitType and the collections. Exactly
one hit resolves the call. Zero hits is an unknown-identifier error whose
suggestions span all three. **Two or more hits is an ambiguity error, never a
silent pick.**

No collision exists today (`fact`/`powr`/`proc`/`weap` versus `e1`/`mcv`), so
that path stays untested unless a fixture forces it. Write the fixture.

### Later: whole-rule-set checks

These need the full set rather than one condition, and they're the checks Go
structurally can't do:

- `squad-exists(X)` with no reachable `form-squad(X, ...)` — currently held
  together only by the two rules being adjacent in one `if` block
- a rule shadowed by a higher-priority exclusive rule in the same category
- priority collisions within a category (ordering is nondeterministic today)
- guards on if/else rule pairs that don't partition (`defend-base` versus
  `form-defense-squad` + `squad-defend-base`)

## Parameters

A doctrine is not a program. Measured across 49 rule sets from real games, what
it varies is:

| | |
|---|---|
| which rules exist | 81 of 91 are conditionally emitted; 10 always appear |
| numeric thresholds | 38 conditions differ in text, 15 once numbers are blanked |
| priority | 27 rules get different priorities under different doctrines |
| the shape of a condition | essentially nothing |

So `CompileDoctrine`'s 2000 lines produce almost no structural variety. It
selects a subset of rules, sets numbers in them, and reorders them. That is
small enough to say in the language:

```
param aggression: float
param naval-weight: float
param ground-attack-group-size: int

rule form-naval-squad {
  priority lerp(200, 400, aggression)
  category squad-form
  require naval-weight >= 0.3
  require count(unassigned-idle-naval) >= ground-attack-group-size
  do form-squad(naval-attack, Naval, ground-attack-group-size, Attack)
}
```

The `require naval-weight >= 0.3` line is Go's `if c.d.NavalWeight >=
DoctrineSignificant { emit ... }`, moved into the language. A doctrine gate and
a game-state condition stop being different kinds of thing.

### Two phases, and why the checker can enforce them

A parameter is constant within a doctrine window; game state changes every tick.
That difference is what makes `priority` expressible at all — it is metadata the
engine sorts by, so it has to be decidable before the tick begins.

**A priority may mention parameters, literals and `lerp`, but never a
predicate.** `priority cash` is a type error, not a runtime surprise. The phase
separation is a property the type checker holds rather than a convention.

### The builtins

```
lerp(min: int, max: int, t: float) -> int        ; rounds
lerpf(min: float, max: float, t: float) -> float
max(a: float, b: float) -> float
min(a: float, b: float) -> float
trunc(x: float) -> int                           ; toward zero
select(cond: bool, a: float, b: float) -> float
```

Not predicates: they read no state, which is exactly why a priority may use one.

`lerp` and `lerpf` cover the doctrine arithmetic in the rule blocks — 66 calls
and 5, and nothing else. The other four come from the savings stack in
`compiler.go`, which the blocks lean on:

- `max` for `if reserveScale < 0 { reserveScale = 0 }`
- `trunc` for `int(800.0 * reserveScale)`, which truncates where `lerp` rounds.
  The difference is real: `800 * 0.29` is `231.999…`, so Go gives 231 and a
  rounding version would give 232 — an off-by-one buried in a cash threshold.
- `select` for `x := a; if cond { x = b }`, a value chosen by a condition. Not a
  lerp and not a clamp: at `vehicle-weight` 0.2 the answer is 1.0, not 0.8.

`select` is expressible without a builtin, by writing both variants as gated
conjuncts so exactly one survives folding. That works, but it doubles every
savings clause in the rules that use it, and those already carry three.

### Binding time is not a design decision

Given the same source, a parameter can be resolved either way:

- **folded by vimyc** when a doctrine lands, emitting a fresh artifact to swap
  in — `CompileDoctrine` rewritten declaratively, needing vimyc at runtime
- **read by the engine** at evaluation time, with the doctrine's values answered
  by `RuleEnv` like any other question — one artifact, compiled at build time,
  no Rust in production

Same file, same semantics; the substitution simply happens somewhere else.
Priorities are the one thing that must be folded per doctrine either way, since
the engine sorts on them. Deferring this choice is most of the reason to put
parameters in the language rather than to keep templating rule text in Go.

### Arguments are static too

A priority is not the only static position. An argument to a predicate or an
action is settled when the doctrine lands, for a reason that comes from Go
rather than from taste: `eval` keys a recorded call by its arguments, and Go's
projection builds those keys from the literals in the source. An argument that
varies per tick has no key to look up. An action's arguments are worse still —
Go finds the function by the text `form-squad(ground-attack, Ground, 8, Attack)`.

So the checker checks arguments in `Phase::Static`, and `form-squad(..., cash,
...)` is rejected. Static rather than *literal*, because the whole point is that
`form-squad(..., ground-attack-group-size, ...)` works: an argument may be any
arithmetic over parameters and `lerp`, and it is folded to a number before it
reaches a key or the emitted text.

A `let` binding is not static, so it cannot be an argument. Nothing in the corpus
does that, and allowing it would mean carrying the rule scope into the fold.

### Specialising, and why a gate must not survive folding

Folding alone turns a gate into a constant comparison rather than removing the
rule, which is not what Go does — `CompileDoctrine` emits no naval rules at all
for a land doctrine. `specialise` restores that: a conjunct decidable from
parameters alone is settled at fold time, false dropping the rule and true
dropping the conjunct.

```
require naval-weight >= 0.3      naval-weight 0.4      MapHasWater() && ...
require map-has-water()      ->                   ->
                                 naval-weight 0.1      (no rule)
```

It matters beyond tidiness. Comparing against real rule sets is the acceptance
test for this whole design, and that compares rule *sets* — a rule Go never
emitted cannot be matched by one that is present but never fires.

A gate is often only *part* of a conjunct — Go appends a clause conditionally,
which reads as `floor <= 0 or role-count(pillbox) >= floor`. Folding alone leaves
`0 <= 0 || RoleCount("pillbox") >= 0`, so `specialise` also absorbs constants
through `and` and `or`: `true || x` is `true`, `false || x` is `x`. That needs a
boolean the IR can hold, which is why `IrExprKind::Bool` exists despite the
language having no boolean literal.

The whole-set checks moved here for the same reason. `check_priority_collisions`
and `check_shadowed_rules` compare priorities, and once a doctrine can set one
they skipped every rule whose priority was a `lerp` — which after a port of
`CompileDoctrine` is nearly all of them. `specialise::validate` runs where the
numbers exist. Comparing the IR rather than the source also made shadowing
stricter: `count(powr)` and `building-count(powr)` are one conjunct there.

One consequence worth knowing: a rule that was nothing but its gate ends up with
no conjuncts at all, and expr will not compile an empty condition. The emitter
writes `true` for it. The same hole was already reachable by writing a rule with
no `require`, where it produced `unexpected token EOF` from Go.

### `def`

The language's only abstraction, added for one shape. Go's
`buildCashCondition(cost, savings)` expands to a cash floor plus up to five
conditional clauses that keep unit production from starving a queued building,
and it is called from twenty-one rules differing only in the unit cost:

```
def reserves(cost: int) =
  cash >= cost
  and (vehicle-weight <= 0.2 or has-role(war-factory) or not has-role(radar)
       or cash >= cost + 2000)
  and ...
```

Written out instead, that is about a hundred near-identical `require` lines that
a reader cannot scan and a hand-port gets wrong.

It earned its keep beyond that one shape. `attack-priority` appears in thirteen
combat rules, the infantry base priority in six production ones, and the
five-way ground-defense disjunction in two — each a name now rather than a
repetition. A def inlines, so none of it changes what is emitted.

Inlined at lowering, at the AST, so the emitted rule looks exactly like Go's —
which has no defs — and lowering itself is unchanged. Capture is impossible:
`bind` already rejects a binding named after a doctrine parameter or a
predicate, and a def body can name nothing else.

Only earlier defs are in scope, which is what rules out recursion. That is not a
restriction anyone will notice, and inlining has to terminate.

### Not parameters

Doctrine also carries `PreferredInfantry` and friends — `[]string` preferences
consumed by `SetPreferences`, not by any rule. They stay where they are. A
parameter is a number a rule can read.

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
stable — you can't trigger a reflow without a grammar change. `require` and `let`
are statements and stay outside the alignment group.

Past looking tidy, the formatter closes the kebab footgun by enforcing spacing
around binary operators, and it's what would make rule-set content hashing stable
if I ever want that for comparing strategies across games.

### v0 needs no line breaking

37 conjuncts across the seed rules: shortest 13 characters, median 20, longest 51.
None over 60. One `require` per line, never wrap.

The doctrine-compiled rules reach 575 characters, so breaking arrives eventually.
Reach for the Wadler/Lindig document algebra then, not before.

## Evaluation semantics, and Go's shared memory

Go's `Evaluate` is **stateful within a tick**. Actions mutate `RuleEnv.Memory`
as the loop runs, and later rules read it: `FormSquad` writes `Memory["squads"]`,
`SquadExists` reads it, and `CompileDoctrine` deliberately places `form-squad` at
`priority + SquadFormBonus` — five above the squad-act rule it feeds, in the same
tick.

vimyc evaluates every rule against one immutable `State`. These do not agree, and
the difference is not a bug in either.

That matters differently depending on what vimyc is being asked to do:

**As a compiler** — vimyc emits a rule set and Go's loop runs it unchanged. The
question does not arise, and this is the actual goal.

`Memory` stays on the Go side throughout. vimyc's `State` is a projection of
`RuleEnv`, not of `model.GameState` — every field answers "what does this env
method return", so `squad_ready` and `has_enemy_intel` arrive as resolved facts
with no trace of the map they were computed from. That is what makes the
following work without any effect modelling in vimyc.

**As a predictor** (shadow mode, counterfactuals) — solvable by snapshotting
**per rule rather than per tick**. Go projects the env at the moment it evaluates
each rule, so earlier actions in the same tick are already reflected. vimyc then
evaluates rule *i* against state *i*. Exact, and it needs no `Memory` modelling
in vimyc at all — Go has already resolved it.

**As a runtime** — the only genuinely hard case, needing either declared action
effects (`form-squad(X, …)` implies `squad-exists(X)` afterwards) or accepting a
tick of latency between forming and acting. Not on the roadmap.

### Recording skipped rules

Go's loop skips a rule in an already-claimed category **without evaluating it**.
A per-rule recording must still project the state for those and mark them
skipped, rather than omitting them.

Two reasons. The shadow comparison stays honest, because vimyc must skip them
too. And "would `build-refinery` have fired if `build-power` had not?" is exactly
the counterfactual worth asking — it is cheap to record and impossible to
recover later.

### What counterfactual replay can and cannot answer

Against a recorded stream of `(tick, rule, state, fired | skipped)`:

- **Yes** — why a rule did not fire (`eval::conjuncts` gives per-conjunct truth),
  what a different rule set would have done at that moment, whether a different
  threshold would have changed the outcome.
- **No** — anything where the game itself diverges. A counterfactual that builds
  a refinery at tick 100 changes every later state, and a state that never
  existed cannot be recovered from a recording. A rule set that forms a squad Go
  never formed will see `squad-exists` false forever after, for the same reason.

Replay-style counterfactuals, not branching-world ones. The first is what tuning
actually needs.

## Deliberately out of scope for v0

`reserve` declarations, `any(...)`, wasm codegen, LLM authoring. All needed
later, none needed to take 13 rules through to a working interpreter.

Two have since landed. Actions with arguments arrived with the seed port.
Doctrine parameters are specified above; the rule-level guard they were paired
with turned out to be unnecessary — a guard is a `require` over a parameter, and
giving it its own syntax would have made two spellings of one idea.
