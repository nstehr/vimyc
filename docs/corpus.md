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
