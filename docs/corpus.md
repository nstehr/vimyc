# Corpus

`testdata/` is generated from Vimy's Go side, not hand-written.

## `seed_rules.json`

The 13 rules of `DefaultRules()` in `vimy-core/rules/seed.go` — name, priority,
category, exclusive, and the expr condition string. This is the v0 target and the
source of truth for `rules/seed.vy`.

Regenerate with a throwaway test in `vimy-core/rules` that marshals
`DefaultRules()` to JSON.

## What the seed rules exercise

```
18 predicates
top-level pure conjunction: 13/13
37 conjuncts: min 13 chars, median 20, max 51 — none over 60
float literals: none
arithmetic:     none
len(...):       6 uses
!= nil:         1 use
```

Every host-import shape a v0 needs shows up:

```
()        -> i32 (bool)   base-under-attack, enemies-visible, has-enemy-intel
(i32)     -> i32 (bool)   has-role, has-unit, has-building, can-build-role,
                          queue-busy, queue-ready
(i32,i32) -> i32 (bool)   can-build
()        -> i32 (int)    cash, power-excess
(i32)     -> i32 (int)    count(e1), count(powr)
()        -> i32          count-lowering  (collections)
()        -> i32          option-lowering (exists nearest-enemy)
```

The one gap is `f64` — nothing in the seed set returns or compares a float, which
is why `squad-ready-ratio` is in the v0 predicate surface deliberately. See
[design.md](design.md#why-squadreadyratio-is-in-v0).

## What is deliberately absent

The wider corpus. `CompileDoctrine` emits **115 rule names** and, across 300
sampled doctrines, **1,932 distinct (name, condition) text forms** — but only
**386 distinct structural forms** once numeric literals are blanked out. The rest
is the same shapes with different thresholds substituted by `lerp`.

Useful later as a grammar-coverage target; noise now. The longest condition in
that set is 575 characters, which is the thing that will eventually force real
line breaking in the formatter.

## Numbers about the problem itself

Measured against `vimy-core/rules` as it stands:

- **2,054 lines** of `fmt.Sprintf` across `compiler*.go` (excluding tests)
- **391 call sites** passing string literals to env predicates inside conditions
- **147 distinct literals** — role, building, queue and squad names, none checked
- **129** `ConditionSrc` references in `compiler_test.go`, mostly "does this
  generated string compile" and `strings.Contains(src, ...)` — assertions that
  exist only because the generation layer is untyped

## A finding worth keeping

Probing `CompileDoctrine` across 300 randomly sampled doctrines: the union of all
rule names is 115, while a single maxed-out doctrine already emits 110. There were
**zero** category or exclusivity conflicts across all 300. Of 27 rules with more
than one structural condition form, only **7** remained after stripping the
reserve clauses spliced in by `initSavings`/`buildCashConditionScaled` — and those
7 are all the same pattern, an optional guard conjunct present or absent by
threshold.

So doctrine's structural influence is a deterministic function of its numbers. A
static rule set with runtime gates could replace `CompileDoctrine` entirely, given
a first-class `reserve` declaration and param-gated conjuncts.

Not verified: runtime behavioural equivalence, and whether guards on if/else rule
pairs partition correctly.

## Real doctrines

`vimy-core/rules/real_doctrines.json` holds 500 doctrines sampled from the 4,876
archived across 64 real games, embedded so tests need no database. It is what
drives the manifest and the differential generator.

Randomly generated doctrines turned out to be a poor stand-in. Real LLM output is
strongly clustered, so uniform sampling both tests rule shapes that never occur
and under-tests the ones that dominate:

| field | real mean | range seen |
|---|---|---|
| `naval_weight` | 0.011 | almost always 0 |
| `economy_priority` | 0.761 | rarely below 0.3 |
| `ground_defense_priority` | 0.719 | never below 0.3 |
| `capture_priority` | 0.09 | mostly 0 |
| `superweapon_priority` | 0.165 | mostly 0 |

Nothing ever reaches 1.0. The 500-doctrine sample reproduces every mean above to
three decimal places.

Regenerate from the sidecar's database:

```sql
sqlite3 ~/.vimy/vimy.db "select distinct doctrine_json from archived_doctrines"
```

then drop `name` and `rationale` (prose, and not read by `CompileDoctrine`) and
sample. The full set is 3MB; the sample is 312KB.

### What the database can and cannot validate

It stores doctrines, rule-set *names*, and per-rule firing counts — **not game
states**. So conditions cannot be replayed against real play, and the
differential corpus stays synthetic. Capturing states would need a change to the
sidecar, and is the same recording shadow mode wants.

What it does give, beyond the doctrine distribution:

- **Empirical ground truth for "never fires".** Across 64 games, 117 distinct
  rules were compiled and 96 ever fired. Seven of the 21 that never fired were
  compiled into *every one* of the 4,876 rule sets — `rebuild-refinery`,
  `rebuild-war-factory`, `rebuild-iron-curtain`, `rebuild-kennel`,
  `rebuild-missile-silo`, and the two naval squad rules. That is a falsifiable
  check on `check_shadowed_rules`: a rule it calls unreachable that appears in
  `rule_firings` is a checker bug.
- **A porting order.** 86 of 117 rules cover 95% of appearances.

## Recording real games

Generated game states cannot realistically produce accumulated intel, formed
squads or threat fields, so the predicates reading them are barely exercised by
the offline corpus. The sidecar can record real ones:

```bash
vimy-core -export-states ~/vimy-export.json -export-every 200
```

Off unless a path is given. Sampled because projecting a state costs about 1ms
against 17µs to evaluate the seed rules — 60x — so recording every evaluation
would be the dominant cost of a tick. `-export-max` caps the file.

The format matches `differential.json`: states stored once and referenced by
index, one case per rule with `fired` or `skipped`. Skipped rules are recorded
too; see `docs/design.md` for why.

`project()` lives in `rules/project.go` rather than in a test, because the
offline dump and the live exporter both need it and two implementations would
drift into differential failures that are not bugs.

## The acceptance corpus

`testdata/acceptance.json`, written by `TestDumpAcceptanceCorpus` in vimy-core:

```
cd vimy-core && DUMP_DIR=../../vimyc/testdata go test ./rules/ -run TestDumpAcceptanceCorpus
```

500 archived doctrines, each with the rule set `CompileDoctrine` produces for it.
No recorded game is involved, because none is needed — `CompileDoctrine` is a
pure function of a `Doctrine`, so its inputs are the whole story. That makes it a
wider corpus than the recorded rule sets and one with nothing to pair.

`tests/acceptance.rs` compares a ported `.vy` block against it, over every
doctrine. Only the rule names the file defines are compared, so an unported block
is absent from both sides rather than a failure. A rule vimyc emits that Go did
not — or the reverse — is a gate that is wrong, which is the mistake a hand-port
makes most.
