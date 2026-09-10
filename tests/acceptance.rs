//! Holding a ported block against the Go compiler it replaces.
//!
//! `testdata/acceptance.json` is written by `TestDumpAcceptanceCorpus` in
//! vimy-core. It needs no recorded game: `CompileDoctrine` is a pure function of
//! a `Doctrine`, so the 500 archived doctrines are the whole input worth testing
//! — a wider corpus than the recorded rule sets, and with nothing to pair.
//!
//! Only the rules the `.vy` file defines are compared. A block that has not been
//! ported yet is simply absent from both sides of the diff.
//!
//! The corpus is frozen: `CompileDoctrine` is deleted, so nothing can regenerate
//! it. It verifies the port, and rules written since are outside its scope —
//! `POST_PORT` names them, so a new rule does not fail the test and an
//! accidental one does not slip through.

use serde::Deserialize;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Deserialize)]
struct Case {
    params: HashMap<String, f64>,
    rules: Vec<GoRule>,
}

#[derive(Debug, Deserialize, PartialEq)]
struct GoRule {
    name: String,
    priority: i64,
    category: String,
    exclusive: bool,
    action: String,
    condition: String,
}

/// Rules that postdate the port, and so have no counterpart in the corpus.
///
/// Explicit rather than "ignore anything unmatched": the whole value of the
/// frozen corpus is that an unexpected rule is still an error.
const POST_PORT: &[&str] = &["form-harvester-guard", "guard-harvesters"];

/// Which field of a rule is allowed to differ from the corpus.
///
/// Per field rather than per rule: a deliberate change to a priority says
/// nothing about the condition, and exempting the whole rule would hide the
/// difference between "we retuned this" and "this drifted".
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Field {
    Priority,
    Condition,
    /// vimyc emits the rule for doctrines where Go did not. A retune that
    /// widens a gate changes which doctrines a rule appears in at all, not just
    /// what it says, and that is as deliberate as any other divergence.
    Presence,
}

/// Rules deliberately changed after the port, what changed, and why.
///
/// Everything not named here is still held against Go — including the other
/// fields of these same rules — so an entry cannot quietly hide a rule that
/// drifted somewhere else.
const RETUNED: &[(&str, Field, &str)] = &[
    (
        "build-barracks",
        Field::Condition,
        "will not produce a second while one is already in the queue. Construction is produce-then-place, so a rule whose limit reads a ROLE COUNT — whether `not has-role` or a numeric cap — is satisfied the whole time an unplaced one sits waiting. Game 99 built three helipads under a cap of two. build-radar carried this guard alone; seventeen rules have it now",
    ),
    (
        "build-extra-refinery",
        Field::Condition,
        "will not produce a second while one is already in the queue. Construction is produce-then-place, so a rule whose limit reads a ROLE COUNT — whether `not has-role` or a numeric cap — is satisfied the whole time an unplaced one sits waiting. Game 99 built three helipads under a cap of two. build-radar carried this guard alone; seventeen rules have it now",
    ),
    (
        "build-extra-war-factory",
        Field::Condition,
        "will not produce a second while one is already in the queue. Construction is produce-then-place, so a rule whose limit reads a ROLE COUNT — whether `not has-role` or a numeric cap — is satisfied the whole time an unplaced one sits waiting. Game 99 built three helipads under a cap of two. build-radar carried this guard alone; seventeen rules have it now",
    ),
    (
        "build-ore-silo",
        Field::Condition,
        "will not produce a second while one is already in the queue. Construction is produce-then-place, so a rule whose limit reads a ROLE COUNT — whether `not has-role` or a numeric cap — is satisfied the whole time an unplaced one sits waiting. Game 99 built three helipads under a cap of two. build-radar carried this guard alone; seventeen rules have it now",
    ),
    (
        "build-refinery",
        Field::Condition,
        "will not produce a second while one is already in the queue. Construction is produce-then-place, so a rule whose limit reads a ROLE COUNT — whether `not has-role` or a numeric cap — is satisfied the whole time an unplaced one sits waiting. Game 99 built three helipads under a cap of two. build-radar carried this guard alone; seventeen rules have it now",
    ),
    (
        "build-second-refinery",
        Field::Condition,
        "will not produce a second while one is already in the queue. Construction is produce-then-place, so a rule whose limit reads a ROLE COUNT — whether `not has-role` or a numeric cap — is satisfied the whole time an unplaced one sits waiting. Game 99 built three helipads under a cap of two. build-radar carried this guard alone; seventeen rules have it now",
    ),
    (
        "build-service-depot",
        Field::Condition,
        "will not produce a second while one is already in the queue. Construction is produce-then-place, so a rule whose limit reads a ROLE COUNT — whether `not has-role` or a numeric cap — is satisfied the whole time an unplaced one sits waiting. Game 99 built three helipads under a cap of two. build-radar carried this guard alone; seventeen rules have it now",
    ),
    (
        "build-tech-center",
        Field::Condition,
        "will not produce a second while one is already in the queue. Construction is produce-then-place, so a rule whose limit reads a ROLE COUNT — whether `not has-role` or a numeric cap — is satisfied the whole time an unplaced one sits waiting. Game 99 built three helipads under a cap of two. build-radar carried this guard alone; seventeen rules have it now",
    ),
    (
        "build-war-factory",
        Field::Condition,
        "will not produce a second while one is already in the queue. Construction is produce-then-place, so a rule whose limit reads a ROLE COUNT — whether `not has-role` or a numeric cap — is satisfied the whole time an unplaced one sits waiting. Game 99 built three helipads under a cap of two. build-radar carried this guard alone; seventeen rules have it now",
    ),
    (
        "build-extra-barracks",
        Field::Condition,
        "will not produce a second while one is already in the queue. Construction is produce-then-place, so a rule whose limit reads a ROLE COUNT — whether `not has-role` or a numeric cap — is satisfied the whole time an unplaced one sits waiting. Game 99 built three helipads under a cap of two. build-radar carried this guard alone; seventeen rules have it now",
    ),
    (
        "build-iron-curtain",
        Field::Condition,
        "will not produce a second while one is already in the queue. Construction is produce-then-place, so a rule whose limit reads a ROLE COUNT — whether `not has-role` or a numeric cap — is satisfied the whole time an unplaced one sits waiting. Game 99 built three helipads under a cap of two. build-radar carried this guard alone; seventeen rules have it now",
    ),
    (
        "build-missile-silo",
        Field::Condition,
        "will not produce a second while one is already in the queue. Construction is produce-then-place, so a rule whose limit reads a ROLE COUNT — whether `not has-role` or a numeric cap — is satisfied the whole time an unplaced one sits waiting. Game 99 built three helipads under a cap of two. build-radar carried this guard alone; seventeen rules have it now",
    ),
    (
        "build-naval-yard",
        Field::Condition,
        "will not produce a second while one is already in the queue. Construction is produce-then-place, so a rule whose limit reads a ROLE COUNT — whether `not has-role` or a numeric cap — is satisfied the whole time an unplaced one sits waiting. Game 99 built three helipads under a cap of two. build-radar carried this guard alone; seventeen rules have it now",
    ),
    (
        "build-extra-naval-yard",
        Field::Condition,
        "will not produce a second while one is already in the queue. Construction is produce-then-place, so a rule whose limit reads a ROLE COUNT — whether `not has-role` or a numeric cap — is satisfied the whole time an unplaced one sits waiting. Game 99 built three helipads under a cap of two. build-radar carried this guard alone; seventeen rules have it now",
    ),
    (
        "build-extra-airfield",
        Field::Condition,
        "will not produce a second pad while one is already in the queue. \
         Construction is produce-then-place, and its cap reads aircraft-capacity \
         — PLACED pads — so three could be produced before any of them landed \
         and the cap was satisfied throughout. Game 99 built three helipads at \
         an air_weight of 0.06 to 0.15, where the cap permits two: about 1500 \
         credits in a game whose base defence could not afford 540. build-radar \
         has carried this guard all along and nothing else did",
    ),
    (
        "build-airfield",
        Field::Condition,
        "holds while the war factory is unaffordable: an exclusive category \
         picks the highest-priority rule whose condition HOLDS, so Go's cash \
         gate kept the dearer war factory out of the running entirely and the \
         category built cheapest-first, against its own priority numbers",
    ),
    (
        "squad-attack",
        Field::Condition,
        "a live ground-defence squad satisfies the base-defence floor: Go \
         counted only static defences, so a doctrine that under-built them \
         could never attack at all while its army watched visible enemies",
    ),
    (
        "squad-attack-known-base",
        Field::Condition,
        "a live ground-defence squad satisfies the base-defence floor, as for \
         squad-attack",
    ),
    (
        "form-defense-squad",
        Field::Condition,
        "no surplus clause: Go also demanded enough spare units for an attack \
         squad on top of the defence squad's own size, which needed a pool of \
         eight and so only formed once the base was already under attack",
    ),
    (
        "scout-with-idle-units",
        Field::Condition,
        "two idle units rather than a whole attack group: the action sends at \
         most two, and Go's bar of six to ten meant nothing scouted until the \
         second half of the game. And since 2026-09-09 it stops once the enemy \
         base is known: Go pulled combat units to patrol waypoints whenever no \
         enemy was on screen, which is 44% of game 88's states and 34% of game \
         90's while the base position was already known. A scattered unit is \
         also not idle, so form-ground-attack cannot count it — scouting \
         consumed the pool that massing needs. Game 93 dispatched scouts 112 \
         times against 4 attacks",
    ),
    (
        "build-aa-defense",
        Field::Priority,
        "below base defense when the doctrine weights air and ground equally",
    ),
    (
        "build-extra-refinery",
        Field::Priority,
        "above the tech centre when economy and tech are weighted equally",
    ),
    (
        "build-gap-generator",
        Field::Priority,
        "below both other defenses; it is the least urgent of the three",
    ),
    (
        "build-naval-yard",
        Field::Priority,
        "below the airfield when air and naval are weighted equally",
    ),
    (
        "build-second-refinery",
        Field::Priority,
        "above the tech centre when economy and tech are weighted equally",
    ),
    (
        "build-base-defense",
        Field::Condition,
        "affordable() now holds a reserve for whichever tech gate is still \
         missing, released the moment net cash turns positive. Go's version \
         reserved only for the radar. Defenses are the rules that outspent it: \
         game 72 sampled thirty-three tesla coils against no radar at all",
    ),
    (
        "build-aa-defense",
        Field::Condition,
        "affordable() reserves for the missing tech gate, as for \
         build-base-defense",
    ),
    (
        "build-gap-generator",
        Field::Condition,
        "affordable() reserves for the missing tech gate, as for \
         build-base-defense",
    ),
    (
        "produce-siege-vehicle",
        Field::Priority,
        "outranks the general vehicle producer only when the doctrine puts \
         siege FIRST. Go promoted it to 485 — above produce-vehicle's 480 in \
         the same exclusive category — whenever artillery appeared ANYWHERE in \
         preferred_vehicle, because prefers-artillery uses contains() while its \
         siblings prefers-radar-gated-primary and specialist-infantry-first use \
         head(). Game 96 listed medium_tank first and artillery second, and \
         built arty 49, light tanks 20, medium tanks 0. It now reads \
         siege-vehicle-first, which is head-based and already existed for the \
         siege cap. prefers-artillery still raises the WAR FACTORY's priority: \
         wanting artillery at all is a fine reason to want a factory sooner",
    ),
    (
        "build-service-depot",
        Field::Presence,
        "gate lowered from vehicle-weight > 0.3 to > 0.1, matching \
         produce-vehicle, so it appears for doctrines Go excluded. The \
         strategist swings vehicle_weight between 0.20 and 0.75 from one \
         twenty-second window to the next — in game 92 a jump of 0.3 or more \
         in ten of thirty windows — so a gate at 0.3 sat inside the swing and \
         the rule kept being deleted from the rule set before it could save the \
         1200 credits it needs. Present in 17 of game 92's 30 windows before, \
         30 of 30 after; no service depot was ever built and no medium tank \
         was ever available",
    ),
    (
        "build-service-depot",
        Field::Priority,
        "ABOVE the radar when the doctrine wants vehicles — the reverse of Go, \
         and of what this exemption said until 2026-09-08. fix is the \
         prerequisite for the medium tank, the heavy tank and the mammoth, so \
         without one the only armour either side can field is the Allied light \
         tank; game 84 never built one and fought six tesla tanks with light \
         tanks and artillery",
    ),
    (
        "build-tesla-coil-for-shock-trooper",
        Field::Priority,
        "below the flame tower, the cheaper unlock",
    ),
    (
        "defend-base",
        Field::Priority,
        "above the scramble it duplicates, being the more specific rule",
    ),
    (
        "produce-apc",
        Field::Priority,
        "above the flak truck; an engineer is already built and waiting",
    ),
    (
        "produce-assault-apc",
        Field::Priority,
        "above the vehicle rules, being capped and doctrine-opted-in",
    ),
    (
        "produce-attack-dog",
        Field::Priority,
        "below specialist infantry when the doctrine puts specialists first",
    ),
    (
        "produce-bridge-infantry",
        Field::Priority,
        "below the rocket soldier; rifle top-ups are the more disposable",
    ),
    (
        "produce-spy",
        Field::Priority,
        "below capture-defense rifles, which are cheaper and defensive",
    ),
    (
        "recall-overextended-naval-attack",
        Field::Priority,
        "below its ground mirror",
    ),
    (
        "rebuild-naval-yard",
        Field::Priority,
        "below the airfield, which is useful on every map",
    ),
    (
        "squad-disengage-naval-attack",
        Field::Priority,
        "below its ground mirror",
    ),
    (
        "squad-focus-fire",
        Field::Priority,
        "capped below the retreat band, so retreating outranks it",
    ),
    (
        "squad-attack-known-base",
        Field::Priority,
        "the aggression threshold that chooses between pressing the base and \
         engaging what is visible sat at 0.3, below anything the strategist \
         ever chose, so this rule always won and `squad-attack` never fired in \
         eighty games",
    ),
    (
        "produce-extra-harvester",
        Field::Condition,
        "cash floor 1400 -> 600: at 1400 it fired 72 times in 77 games, against \
         a p90 cash of ~1100. And the cap is `< refineries + 2` rather than \
         Go's `< refineries + 1`. It was `refineries * 2` between 8 and 10 \
         September, which put ten harvesters on the field; game 98 lost nine of \
         them to raids, roughly 12,600 credits of replacements in a game where \
         a 540-credit pillbox was unaffordable in 90% of states. Go's original \
         let this rule \
         act exactly ONCE in each of games 85, 86 and 87, which held the \
         economy at four refineries and five harvesters in all three, with \
         median cash ~350 and a median income rate of zero. It then yields \
         past one harvester per refinery until two combat vehicles exist: the \
         mod gives a player ONE Vehicle queue however many war factories it \
         builds, this rule outranks every combat-vehicle rule in that shared \
         exclusive category by design, and doubling the cap therefore bought \
         income by spending the army's queue time",
    ),
    (
        "rebuild-harvester",
        Field::Condition,
        "replaces against the refinery count rather than waiting for the last harvester to die",
    ),
    // `reserves()` went from `cost + N` to `cost + min(cost * k, N)`. A flat
    // sum taxed a 100-credit rifleman elevenfold and a 2,000-credit tank twice,
    // so the cheap escorts that make any plan work were the worst-hit line in
    // the ledger. The reserve is now proportional up to the old cap, which
    // leaves anything at or above the cap exactly where it was — only the cheap
    // end moves, which is the end that was wrong.
    (
        "produce-assault-apc",
        Field::Condition,
        "reserve is a multiple of the unit's price, not a flat sum added to it",
    ),
    (
        "produce-attack-dog",
        Field::Condition,
        "reserve is a multiple of the unit's price, not a flat sum added to it",
    ),
    (
        "produce-grenadier",
        Field::Condition,
        "reserve is a multiple of the unit's price, not a flat sum added to it",
    ),
    (
        "produce-infantry",
        Field::Condition,
        "reserve is a multiple of the unit's price, not a flat sum added to it; \
         and since 2026-09-08 it also holds while a war factory stands with no \
         combat vehicle beside it and no cash for one. Go released the hold as \
         soon as a factory existed, which is precisely when saving for vehicles \
         starts to matter: game 86 bought 72 infantry and 3 vehicles, with a \
         median of one tank and a maximum of two",
    ),
    (
        "produce-rocket-soldier",
        Field::Condition,
        "reserve is a multiple of the unit's price, not a flat sum added to it",
    ),
    (
        "produce-scout-vehicle",
        Field::Condition,
        "reserve is a multiple of the unit's price, not a flat sum added to it",
    ),
    (
        "produce-specialist-infantry",
        Field::Condition,
        "reserve is a multiple of the unit's price, not a flat sum added to it",
    ),
    (
        "produce-spy",
        Field::Condition,
        "reserve is a multiple of the unit's price, not a flat sum added to it",
    ),
    (
        "produce-aircraft",
        Field::Condition,
        "reserve is a multiple of the unit's price, not a flat sum added to it",
    ),
    (
        "produce-bridge-infantry",
        Field::Condition,
        "reserve is a multiple of the unit's price, not a flat sum added to it",
    ),
    (
        "produce-flak-truck",
        Field::Condition,
        "reserve is a multiple of the unit's price, not a flat sum added to it",
    ),
    (
        "produce-minelayer",
        Field::Condition,
        "reserve is a multiple of the unit's price, not a flat sum added to it",
    ),
    (
        "produce-gunboat",
        Field::Condition,
        "reserve is a multiple of the unit's price, not a flat sum added to it",
    ),
    (
        "produce-ship",
        Field::Condition,
        "reserve is a multiple of the unit's price, not a flat sum added to it",
    ),
];

/// Where Vimy's rule sets live.
///
/// They are Vimy's strategy, not this compiler's, so they live in that repo —
/// the way its `.go` files do. `VIMY_RULES` overrides the sibling default, and
/// these tests skip when it is not there: they verify another project's content
/// and cannot run without it. `rules/fixture.vy` is what the compiler's own
/// tests use.
fn vy_dir() -> Option<std::path::PathBuf> {
    let dir = std::env::var("VIMY_RULES")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../vimy/vimy-core/rules/vy")
        });
    dir.is_dir().then_some(dir)
}

/// The files of Vimy's rule set, in the order vimyc would read the directory.
const BLOCKS: &[&str] = &[
    "buildings.vy",
    "combat.vy",
    "core.vy",
    "economy.vy",
    "micro.vy",
    "production.vy",
];

/// Vimy's rule set, parsed as the one unit a game compiles.
///
/// No block stands alone — `core.vy` holds the defs the others call — so there
/// is nothing to read but the whole set.
fn rule_set(dir: &std::path::Path) -> vimyc::unit::Unit {
    let paths: Vec<_> = BLOCKS.iter().map(|f| dir.join(f)).collect();
    let unit = vimyc::unit::Unit::read(&paths).unwrap_or_else(|e| panic!("{e}"));
    assert!(unit.diags.is_empty(), "{:?}", unit.diags);
    unit
}

/// The rules one block declares, by name.
///
/// Parsed rather than checked: a block on its own does not type-check now that
/// the defs live in `core.vy`, and the names are all this needs.
fn rules_in(dir: &std::path::Path, file: &str) -> HashSet<String> {
    let path = dir.join(file);
    let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    let (tokens, ld) = vimyc::lexer::lex(&src);
    assert!(ld.is_empty(), "{file}: {ld:?}");
    let (ast, pd) = vimyc::parser::parse(&tokens);
    assert!(pd.is_empty(), "{file}: {pd:?}");
    ast.rules.into_iter().map(|r| r.name.text).collect()
}

fn corpus() -> Option<Vec<Case>> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/acceptance.json");
    let json = std::fs::read_to_string(path).ok()?;
    Some(serde_json::from_str(&json).expect("acceptance.json"))
}

/// Go writes whatever its templates contain; vimyc writes a canonical form. The
/// differences that survive are parentheses and spacing, neither of which
/// reaches the engine — `emitted_expr_round_trips_on_a_real_game` covers the
/// same ground for the seed set.
fn normalise(s: &str) -> String {
    trim_zeros(
        &s.split_whitespace()
            .collect::<String>()
            .replace(['(', ')'], ""),
    )
}

/// Go writes a threshold with `%.2f`, so `0.5` arrives as `0.50`. The value is
/// the same and Go's own projection normalises the literal before recording a
/// state key, so the trailing zeros reach nothing.
fn trim_zeros(s: &str) -> String {
    let b: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == '.' && i > 0 && b[i - 1].is_ascii_digit() {
            let mut end = i + 1;
            while end < b.len() && b[end].is_ascii_digit() {
                end += 1;
            }
            let frac: String = b[i + 1..end].iter().collect();
            let frac = frac.trim_end_matches('0');
            if !frac.is_empty() {
                out.push('.');
                out.push_str(frac);
            }
            i = end;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

#[test]
fn the_economy_block_matches_go() {
    block_matches_go(Some("economy.vy"));
}

#[test]
fn the_buildings_block_matches_go() {
    block_matches_go(Some("buildings.vy"));
}

#[test]
fn the_production_block_matches_go() {
    block_matches_go(Some("production.vy"));
}

#[test]
fn the_combat_block_matches_go() {
    block_matches_go(Some("combat.vy"));
}

#[test]
fn the_micro_block_matches_go() {
    block_matches_go(Some("micro.vy"));
}

#[test]
fn the_core_block_matches_go() {
    block_matches_go(Some("core.vy"));
}

/// Compares one block's rules against Go, or the whole set when `file` is
/// `None`.
///
/// The unit is always the whole rule set — a block does not compile alone — and
/// `file` only narrows which rules are compared.
fn block_matches_go(file: Option<&str>) {
    let Some(cases) = corpus() else {
        eprintln!("no acceptance corpus; run TestDumpAcceptanceCorpus");
        return;
    };

    let Some(dir) = vy_dir() else {
        eprintln!("Vimy's rules are not beside this checkout; skipping");
        return;
    };
    let label = file.unwrap_or("the rule set");
    let ast = rule_set(&dir).ast;

    // The names being compared. Anything outside them belongs to a block that
    // has not been ported.
    let ported: HashSet<String> = {
        let ir = vimyc::check::check(&ast)
            .unwrap_or_else(|d| panic!("{label} does not check: {d:?}"))
            .ir;
        let own = file.map(|f| rules_in(&dir, f));
        ir.rules
            .iter()
            .map(|r| r.name.clone())
            .filter(|n| own.as_ref().is_none_or(|o| o.contains(n)))
            .filter(|n| !POST_PORT.contains(&n.as_str()))
            .collect()
    };

    let mut differ: Vec<String> = Vec::new();
    let mut compared = 0usize;

    for (i, case) in cases.iter().enumerate() {
        let mut ir = vimyc::check::check(&ast).expect("checks").ir;
        let params = vimyc::ir::ParamValues::bind(&ir, &case.params)
            .unwrap_or_else(|e| panic!("doctrine {i}: {e}"));
        vimyc::specialise::specialise(&mut ir, &params);
        let vimyc::emit::Artifact::Expr(mine) =
            vimyc::emit::emit(&ir, &params, vimyc::emit::Target::Expr)
        else {
            unreachable!()
        };

        let theirs: HashMap<&str, &GoRule> = case
            .rules
            .iter()
            .filter(|r| ported.contains(&r.name))
            .map(|r| (r.name.as_str(), r))
            .collect();

        for r in &mine {
            // The unit emits every block's rules; `ported` is the subset under
            // comparison, and already has the post-port additions removed.
            if !ported.contains(&r.name) {
                continue;
            }
            let Some(want) = theirs.get(r.name.as_str()) else {
                if !RETUNED
                    .iter()
                    .any(|(n, d, _)| *n == r.name && *d == Field::Presence)
                {
                    differ.push(format!("doctrine {i}: emitted `{}`, Go did not", r.name));
                }
                continue;
            };
            compared += 1;
            let retuned = |f: Field| RETUNED.iter().any(|(n, d, _)| *n == r.name && *d == f);
            let mismatch = if r.priority != want.priority && !retuned(Field::Priority) {
                Some(format!("priority {} vs {}", r.priority, want.priority))
            } else if r.category != want.category {
                Some(format!("category {} vs {}", r.category, want.category))
            } else if r.exclusive != want.exclusive {
                Some(format!("exclusive {} vs {}", r.exclusive, want.exclusive))
            } else if r.action != want.action {
                Some(format!("action {} vs {}", r.action, want.action))
            } else if normalise(&r.condition) != normalise(&want.condition)
                && !retuned(Field::Condition)
            {
                Some(format!(
                    "condition\n      go:    {}\n      vimyc: {}",
                    want.condition, r.condition
                ))
            } else {
                None
            };
            if let Some(m) = mismatch {
                differ.push(format!("doctrine {i}: `{}` {m}", r.name));
            }
        }

        for name in theirs.keys() {
            if !mine.iter().any(|r| r.name == *name) {
                differ.push(format!("doctrine {i}: Go emitted `{name}`, vimyc did not"));
            }
        }
    }

    assert!(compared > 500, "corpus looks truncated: {compared}");
    assert!(
        differ.is_empty(),
        "{} disagreements over {compared} rules:\n{}",
        differ.len(),
        differ
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    eprintln!(
        "{compared} rules from {label} across {} doctrines match Go",
        cases.len()
    );
}

/// Every rule `CompileDoctrine` can emit is claimed by some block.
///
/// The per-block tests only compare the names their file defines, so a rule
/// nobody ported would pass unnoticed everywhere. This is what says the port is
/// finished rather than merely correct as far as it goes.
/// The blocks combined: one rule set, which is what a game would actually run.
///
/// Not a formality — the blocks share parameter and def names, so combining
/// them means keeping one of each, and two definitions that merely looked alike
/// would show up here as a rule set that no longer matches Go.
#[test]
fn the_combined_rule_set_matches_go() {
    block_matches_go(None);
}

/// Every `RETUNED` entry names a real rule whose named field really does differ.
///
/// Without this the list only ever grows: an entry for a rule that was deleted,
/// or whose value was later put back, would sit there silently widening the hole
/// in the comparison.
#[test]
fn every_retuned_rule_exists_and_still_differs() {
    let Some(cases) = corpus() else {
        eprintln!("no acceptance corpus; run TestDumpAcceptanceCorpus");
        return;
    };
    let Some(dir) = vy_dir() else {
        eprintln!("Vimy's rules are not beside this checkout; skipping");
        return;
    };
    let ast = rule_set(&dir).ast;

    let names: HashSet<&str> = ast.rules.iter().map(|r| r.name.text.as_str()).collect();
    for (n, _, _) in RETUNED {
        assert!(names.contains(n), "`{n}` is retuned but no longer exists");
    }

    // An entry only earns its exemption if some doctrine really does produce a
    // different value from the one Go recorded, in the field it names.
    let mut differs: HashSet<(&str, Field)> = HashSet::new();
    for case in cases.iter().step_by(7) {
        let mut ir = vimyc::check::check(&ast).expect("checks").ir;
        let params = vimyc::ir::ParamValues::bind(&ir, &case.params).expect("binds");
        vimyc::specialise::specialise(&mut ir, &params);
        let vimyc::emit::Artifact::Expr(mine) =
            vimyc::emit::emit(&ir, &params, vimyc::emit::Target::Expr)
        else {
            unreachable!()
        };
        for r in &mine {
            // Not `continue` on a missing Go rule: a rule Go never emitted for
            // this doctrine is exactly what a Presence entry claims, so it has
            // to be able to prove itself here.
            let want = case.rules.iter().find(|g| g.name == r.name);
            for (n, field, _) in RETUNED {
                if *n != r.name {
                    continue;
                }
                let changed = match (field, want) {
                    (Field::Presence, None) => true,
                    (Field::Presence, Some(_)) => false,
                    (_, None) => false,
                    (Field::Priority, Some(w)) => r.priority != w.priority,
                    (Field::Condition, Some(w)) => {
                        normalise(&r.condition) != normalise(&w.condition)
                    }
                };
                if changed {
                    differs.insert((*n, *field));
                }
            }
        }
    }
    let stale: Vec<(&str, Field)> = RETUNED
        .iter()
        .map(|(n, f, _)| (*n, *f))
        .filter(|k| !differs.contains(k))
        .collect();
    assert!(
        stale.is_empty(),
        "retuned but matching Go on every doctrine, so the entry is dead: {stale:?}"
    );
}

#[test]
fn the_blocks_cover_every_rule_go_emits() {
    let Some(cases) = corpus() else {
        eprintln!("no acceptance corpus; run TestDumpAcceptanceCorpus");
        return;
    };
    let Some(dir) = vy_dir() else {
        eprintln!("Vimy's rules are not beside this checkout; skipping");
        return;
    };

    // Per block rather than from the unit, so this still catches a rule that
    // two blocks both define — which the unit would happily accept as two
    // rules with the same name.
    let mut ported: HashSet<String> = HashSet::new();
    for file in BLOCKS {
        for name in rules_in(&dir, file) {
            assert!(
                ported.insert(name.clone()),
                "`{name}` is defined by two blocks"
            );
        }
    }
    // Every block belongs to the set that actually compiles.
    let compiled: HashSet<String> = rule_set(&dir)
        .ast
        .rules
        .iter()
        .map(|r| r.name.text.clone())
        .collect();
    assert_eq!(ported, compiled, "a block is missing from BLOCKS");

    let mut go: HashSet<&str> = HashSet::new();
    for case in &cases {
        for r in &case.rules {
            go.insert(&r.name);
        }
    }

    let missing: Vec<&&str> = go.iter().filter(|n| !ported.contains(**n)).collect();
    assert!(missing.is_empty(), "not ported: {missing:?}");

    // A name Go never emits is not automatically wrong — `build-barracks-prereq`
    // wants a doctrine with air, naval or tech but no vehicles, no infantry and
    // no ground defense, and nothing in the corpus is shaped like that. What
    // matters is that vimyc does not emit it either, since no per-block test
    // would compare it.
    let mut emitted: HashSet<String> = HashSet::new();
    let ast = rule_set(&dir).ast;
    for case in &cases {
        let mut ir = vimyc::check::check(&ast).expect("checks").ir;
        let params = vimyc::ir::ParamValues::bind(&ir, &case.params).expect("binds");
        vimyc::specialise::specialise(&mut ir, &params);
        emitted.extend(ir.rules.iter().map(|r| r.name.clone()));
    }

    // The post-port rules must actually be reachable, or naming one here would
    // be a way to hide a rule that never fires.
    for n in POST_PORT {
        assert!(
            emitted.contains(*n),
            "`{n}` is never emitted by any doctrine"
        );
    }

    let invented: Vec<&String> = emitted
        .iter()
        .filter(|n| !go.contains(n.as_str()) && !POST_PORT.contains(&n.as_str()))
        .collect();
    assert!(
        invented.is_empty(),
        "vimyc emits what Go does not: {invented:?}"
    );

    let unreachable: Vec<&String> = ported.difference(&emitted).collect();
    eprintln!(
        "{} rule names ported; {} unreachable for every doctrine, in Go too: {unreachable:?}",
        ported.len(),
        unreachable.len()
    );
}

/// The printed `.vy` is the rule set it claims to be.
///
/// Nothing executes it — Go runs the expr beside it — so a rendering bug would
/// otherwise be invisible on a dashboard nobody could check. Round-tripping
/// makes "what you read is what runs" a property: print the IR, parse it back,
/// lower it, and require the same rules with the same conditions.
///
/// Run against every archived doctrine, so it covers the shapes real play
/// produces rather than the ones a fixture would.
#[test]
fn emitted_vy_round_trips() {
    let Some(cases) = corpus() else {
        eprintln!("no acceptance corpus; skipping");
        return;
    };

    let Some(dir) = vy_dir() else {
        eprintln!("Vimy's rules are not beside this checkout; skipping");
        return;
    };
    let ast = rule_set(&dir).ast;

    let mut checked = 0usize;
    for (i, case) in cases.iter().enumerate().step_by(17) {
        let mut ir = vimyc::check::check(&ast).expect("checks").ir;
        let params = vimyc::ir::ParamValues::bind(&ir, &case.params).expect("binds");
        vimyc::specialise::specialise(&mut ir, &params);

        // The expr each rule compiles to, before the round trip.
        let vimyc::emit::Artifact::Expr(before) =
            vimyc::emit::emit(&ir, &params, vimyc::emit::Target::Expr)
        else {
            unreachable!()
        };

        // Print, and read it back as a fresh rule set. It has no parameters
        // left — specialising folded them — so it binds against nothing.
        let printed = vimyc::emit::vy::emit_file(&ir, &params);
        let (tokens, lex_diags) = vimyc::lexer::lex(&printed);
        assert!(
            lex_diags.is_empty(),
            "doctrine {i} does not lex:\n{printed}"
        );
        let (reparsed, parse_diags) = vimyc::parser::parse(&tokens);
        assert!(
            parse_diags.is_empty(),
            "doctrine {i} does not parse: {parse_diags:?}\n{printed}"
        );
        let round = vimyc::check::check(&reparsed)
            .unwrap_or_else(|d| panic!("doctrine {i} does not check: {d:?}\n{printed}"))
            .ir;

        let vimyc::emit::Artifact::Expr(after) = vimyc::emit::emit(
            &round,
            &vimyc::ir::ParamValues::default(),
            vimyc::emit::Target::Expr,
        ) else {
            unreachable!()
        };

        assert_eq!(before.len(), after.len(), "doctrine {i}: rule count");
        for (b, a) in before.iter().zip(&after) {
            assert_eq!(b.name, a.name, "doctrine {i}");
            assert_eq!(b.priority, a.priority, "doctrine {i}: {}", b.name);
            assert_eq!(b.category, a.category, "doctrine {i}: {}", b.name);
            assert_eq!(b.exclusive, a.exclusive, "doctrine {i}: {}", b.name);
            assert_eq!(b.action, a.action, "doctrine {i}: {}", b.name);
            assert_eq!(b.because, a.because, "doctrine {i}: {}", b.name);
            // Exact, not `normalise`: that strips parentheses, which is right
            // against Go and wrong here. Both sides are this emitter, so a
            // printer that loses a parenthesis must fail rather than pass on a
            // comparison that cannot see it.
            assert_eq!(b.condition, a.condition, "doctrine {i}: {}", b.name);
            checked += 1;
        }
    }

    assert!(checked > 1000, "only {checked} rules round tripped");
    eprintln!("{checked} rules survive printing and reading back");
}
