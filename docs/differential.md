# Lining vimyc up with Go

What it took to make vimyc agree with the engine it replaces, what broke on the
way, and what the failures had in common.

Figures are from the analysis window — 64 archived games, 4,876 doctrines — and
from the recording of game 69. Both grow as more games are played, so treat them
as the shape of the thing rather than as live counts.

The end state: **16,317 rule evaluations from a real 31,950-tick game, no
disagreements**, across 49 doctrine-compiled rule sets that all parse and type
check — plus 5,200 synthetic comparisons on the seed rules.

## The bugs vimyc found in Vimy

Three, all pre-existing, none visible to any test Vimy had. Filed rather than
fixed: changing bot behaviour while the rule engine is being replaced would make
a later behaviour change unattributable.

### Priority collisions inside exclusive categories — `vimy-axv`

`rebuild-airfield` and `rebuild-naval-yard` both sit at priority 800 in category
`rebuild`, which is exclusive. `compileRules` sorts with `sort.Slice`, which Go
does not guarantee stable, so which of them fires is arbitrary and can differ
between runs of an identical rule set.

Both are eligible together on a water map where an airfield and a naval yard have
been destroyed, the Building queue is free, and cash is at least 300 — verified
against their conditions rather than assumed. `build-aa-defense` and
`build-gap-generator` collide the same way at 520 in exclusive `defense`.

Found by the type checker on the first real doctrine it was pointed at.

### `form-ground-attack` re-forms its squad constantly — `vimy-1h7`

149 firings over 11,300 ticks, and `squad-exists(ground-attack)` was false at
every single one — so the squad is created and gone again by the next sample. It
accounted for 86% of all firings in that game. Cheapest hypothesis to check
first: `Engine.Swap` deletes `Memory["squads"]` on every doctrine change, and a
game swaps ~73 times.

Found by reading a state export, not by any check.

### `ActionRegistry` had drifted

Sixteen actions used by compiled rules were missing from the map. Nothing reads
it, so nothing noticed. vimyc needs every action nameable in order to emit a rule
that runs one, which is what surfaced it.

## The bugs in the harness

More numerous than the bugs in vimyc, and worth recording because every one of
them *looked* like a vimyc bug first.

### Projecting once per evaluation

The exporter recorded one state per `Evaluate` call, so all 78 rules in a tick
shared the pre-loop snapshot. But actions mutate `Memory` as the loop runs:
`FormSquad` assigns units, `UnassignedIdleGround` drops, and every later rule
sees the change. **36 disagreements in 19,925**, every one a rule that ran after
a firing.

`docs/design.md` describes this hazard in the section on evaluation semantics.
It was written before the exporter and then walked into anyway, because the
offline dump runs no actions and the optimisation looked free there.

Fixed by re-projecting after each firing — only a firing can change anything, so
it costs two or three projections a tick rather than eighty.

### Pairing a recording to its rule set

Three attempts, each closer and none exact:

1. **By tick.** `archived_doctrines.tick` is when a doctrine was *generated*, but
   the LLM call takes time and the engine keeps running the previous set until
   the swap lands. The lag is not a fixed offset. 332 cases unpaired.
2. **By rule-name set.** Every rule evaluated in a tick is recorded, so the set of
   names identifies the rule set — except two doctrines routinely emit the same
   names with different thresholds. Measured: **200 real doctrines produce 200
   distinct rule sets but only 118 distinct name lists.**
3. **By the state's own key set.** A projection asks exactly the questions its
   rules ask, so the keys fingerprint the rule set. Closer, but a threshold
   inside a comparison — `>= 8` — never appears as a key. 1 case in 12,442 left.

Settled by having the exporter record `RuleSetID`, a hash over each rule's name,
priority, category, exclusivity, condition and action. Inference is not
recoverable after the fact; the recording has to say.

That hash then had to be made **order-independent**: `compileRules` sorts by
priority, so the engine holds a different ordering from what `CompileDoctrine`
returned, and `sort.Slice` being unstable means two sorts need not even agree
with each other. The first version matched 1 of 47 real rule sets — the one whose
source order already happened to be sorted.

### `0.10` is not `0.1`

The one that mattered. Go built projection keys from the literal text in a
condition; vimyc parsed the same literal into an `f64` and formatted it back. So
`squad-threat-ratio(ground-attack, 0.10)` was recorded under `…0.10` and looked
up under `…0.1`, the lookup missed, the zero default came back, and the rule
evaluated false against a real value of 6.54.

This is the failure mode worth internalising, because it is **silent and
always-false**. A key that misses reads a default; a default makes a condition
false; a rule that is false agrees with a rule that is false. It survived until
one evaluation in 16,479 happened to go true.

## What the failures had in common

**Almost every one was always-false.** A wrong answer that says "no" agrees with
a correct "no", so it hides wherever the correct answer is also no. In the game
this corpus comes from, **74 of 91 rules only ever evaluated false**, and across
64 archived games 21 of 117 rules never fired at all. Coverage cannot reach
them.

The response was to stop chasing coverage and test the *mechanism* instead:
`lookup_keys_match_what_go_records` compares all 266 keys vimyc would look up
against the keys Go records, byte for byte. That catches the whole class without
needing the rule to fire.

Its first version was worthless: it reimplemented the key rendering rather than
calling it, so it kept passing when the real rendering was deliberately broken.
Single-sourcing it in `state::render_number` fixed that — and breaking the format
now produces ten failures naming the exact predicates.

**Synthetic data misleads in ways that look fine.** Doctrines drawn uniformly at
random bear no resemblance to what an LLM produces: real `naval_weight` averages
0.011, `economy_priority` 0.761, and nothing ever reaches 1.0. Uniform sampling
tested rule shapes that never occur while under-testing the ones that dominate.
The fix was to sample 500 real doctrines out of the 4,876 archived.

Hand-built game states have the same problem in a form no amount of tuning fixes.
They cannot produce accumulated intel, formed squads or threat fields, so the
predicates reading them go untested. That is what the runtime exporter is for,
and the two corpora are complementary rather than competing: real states cover
`Memory`-backed predicates, synthetic states cover the rare branches real play
never reaches.

**A green test proves nothing until you have seen it fail.** Every check worth
keeping here was verified by deliberately breaking what it guards: swapping
`.chars().count()` for byte subtraction, deleting a registry entry, starving a
conjunct, changing the float format. Two tests passed vacuously until that was
done — the key-rendering one above, and an early `ActionRegistry` guard whose
exclusion list happened to name exactly the rules it should have caught.

**Measure the corpus, not just the result.** `seed_agrees_with_expr` passed while
six of thirteen rules were always false and therefore untested. Two coverage
tests now assert that every rule and every conjunct varies, and that each is
exercised in at least 5% of cases either way — "varies at all" is too weak, since
a conjunct true once in 400 discriminates on paper while leaving a bug in it
almost certain to survive.

## Things that turned out cheaper than expected

**Translation is mechanical.** A survey of every condition the compiler emits
found calls, literals, boolean operators, comparisons and arithmetic — no
ternaries, no indexing, no field access. So `translate.go` rewrites expr into
vimyc source rather than anyone hand-translating 117 rules.

**Reflection carries most of the env surface.** `RuleEnv` has 102 exported
methods; Go can report arity, parameter and return types for all of them, and
which 64 any rule names. The only judgement left is which domain each string
parameter belongs to, and even that is largely inferable from the literals rules
actually pass. 64 predicates, 22 classified automatically.

**The database answers questions the code cannot.** 64 games of `rule_firings`
say which rules ever fire — 96 of 117 — which is empirical ground truth for
`check_shadowed_rules` to be checked against. It also gives a porting order: 86
of 117 rules cover 95% of appearances.

## Things that turned out more expensive

**Performance intuitions were wrong by three orders of magnitude.** A projection
was measured at 85ms and used to justify sampling; it was actually 79µs, and the
85ms was `argLiterals` recompiling 500 doctrines on every call. Hoisting it took
the corpus dump from 34 seconds to 1.8.

The real number is ~1ms per projection against 17µs to evaluate the seed rules —
60x — which still justifies sampling, but for a different reason than the one
first given.

**Asking every question is not free.** The first projector took the cartesian
product of every argument in every domain: 52 roles times 10 thresholds times
5,200 projections. Narrowing it to the literals rules actually pass cut the work
roughly 15x and the file size with it, since 62 of 78 collection keys were
threshold variants no live rule set uses.

**Inlining repeated data.** Storing a full state on every one of its 13 cases
made a corpus 21MB. Referencing states by index made it 1.9MB.
