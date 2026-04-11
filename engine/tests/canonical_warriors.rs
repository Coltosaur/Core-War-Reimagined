//! Canonical warrior integration tests.
//!
//! These tests live in `engine/tests/` instead of inside `engine/src/` for
//! two reasons:
//!
//!   1. Each `.rs` file in `tests/` is compiled as a *separate binary* that
//!      links against `core_war_engine` as a downstream consumer would. They
//!      can only use items that are `pub use`'d from `lib.rs` — if we ever
//!      accidentally make something `pub(crate)` instead of `pub`, these
//!      tests catch it where the in-crate unit tests can't.
//!
//!   2. The `.red` files in `engine/tests/warriors/` exercise the
//!      parser → load → execute pipeline against text source the same way
//!      real warriors would arrive in production. The unit tests in
//!      `src/parser.rs` use string literals embedded in the test functions;
//!      here the warriors live in standalone files via `include_str!`,
//!      mirroring how a frontend or backend would deliver them.
//!
//! Importantly, none of the tests in this file construct an `Instruction`
//! literal directly. Every warrior — including the marker planted in the
//! scanner test — comes through `parse_warrior`. That makes this file a
//! clean usage example for the eventual frontend and backend code.

use core_war_engine::{parse_warrior, MatchResult, MatchState, Opcode};

const IMP: &str = include_str!("warriors/imp.red");
const DWARF: &str = include_str!("warriors/dwarf.red");
const MICE: &str = include_str!("warriors/mice.red");
const MICE_LITE: &str = include_str!("warriors/mice_lite.red");
const SCANNER: &str = include_str!("warriors/scanner.red");

#[test]
fn parsed_imp_propagates_through_core() {
    let parsed = parse_warrior(IMP).expect("imp.red should parse");
    let mut state = MatchState::new(64, 50);
    state.load_warrior(0, &parsed, 0);

    let imp_instr = parsed.instructions()[0];

    for _ in 0..5 {
        assert!(state.step(), "imp should never die");
    }

    // After 5 steps, cells 0..=5 should all hold the imp.
    for cell in 0..=5 {
        assert_eq!(
            state.core().get(cell),
            imp_instr,
            "cell {cell} should be the imp",
        );
    }

    assert_eq!(state.result(), MatchResult::Victory { winner_id: 0 });
}

#[test]
fn parsed_dwarf_bombs_core_at_intervals_of_four() {
    let parsed = parse_warrior(DWARF).expect("dwarf.red should parse");
    let mut state = MatchState::new(64, 100);
    state.load_warrior(0, &parsed, 0);

    // 5 iterations × 3 instructions per iteration = 15 steps.
    for _ in 0..15 {
        assert!(state.step());
    }

    // Bomb pattern: cells 7, 11, 15, 19, 23 hold DAT bombs whose B-field
    // matches the bomb pointer's value at the time of each MOV.
    for (addr, expected_b) in [(7, 4), (11, 8), (15, 12), (19, 16), (23, 20)] {
        let cell = state.core().get(addr);
        assert_eq!(cell.opcode, Opcode::Dat, "cell {addr} should be a DAT bomb");
        assert_eq!(cell.b.value, expected_b);
    }

    // Cell 3 (the bomb pointer in the dwarf's own program) ended at B=20.
    assert_eq!(state.core().get(3).b.value, 20);

    assert_eq!(state.result(), MatchResult::Victory { winner_id: 0 });
}

#[test]
fn parsed_dwarf_extracts_name_and_author_metadata() {
    let parsed = parse_warrior(DWARF).expect("dwarf.red should parse");
    assert_eq!(parsed.name(), Some("Dwarf"));
    assert_eq!(parsed.author(), Some("A.K. Dewdney"));
}

#[test]
fn parsed_mice_lite_replicates_imp_three_times() {
    let parsed = parse_warrior(MICE_LITE).expect("mice_lite.red should parse");
    let mut state = MatchState::new(32, 50);
    state.load_warrior(0, &parsed, 0);

    // The imp template is at offset 2 in the source layout.
    let template = parsed.instructions()[2];

    // 3 copies × 2 instructions per copy + 1 final DJN fall-through + 1 DAT
    // landing-pad death = 7 steps.
    for _ in 0..7 {
        state.step();
    }

    // Three copies of the imp template at cells 6, 7, 8 (walking backwards
    // as the dest pointer predecrements from 8 → 7 → 6 → 5).
    for cell in [6, 7, 8] {
        assert_eq!(
            state.core().get(cell),
            template,
            "cell {cell} should hold a copy of the imp template",
        );
    }

    // The counter (cell 0) and dest pointer (cell 1) ended in their
    // expected exhausted states.
    assert_eq!(state.core().get(0).b.value, 0, "counter should be exhausted");
    assert_eq!(state.core().get(1).b.value, 5, "dest should have decremented 8 → 5");

    // Process died on the DAT landing pad.
    assert_eq!(state.result(), MatchResult::AllDead);
}

#[test]
fn parsed_scanner_finds_and_bombs_planted_marker() {
    let parsed = parse_warrior(SCANNER).expect("scanner.red should parse");
    let mut state = MatchState::new(32, 100);
    state.load_warrior(0, &parsed, 0);

    // Plant a recognizable marker at cell 12 — the scanner is initialized
    // with ptr.B = 9, so after three ADDs (9 → 10 → 11 → 12) it will
    // compare cell 12 to the blank template, find it differs, and bomb it.
    //
    // We construct the marker by parsing a tiny one-line "warrior", which
    // keeps the test 100% on the public API — no Instruction literals.
    let marker_warrior =
        parse_warrior("JMP.B $123, $456").expect("marker source should parse");
    let marker = marker_warrior.instructions()[0];
    state.core_mut().set(12, marker);

    // 11 steps: 3 scan iterations × 3 instructions each minus the final
    // skip/no-skip, plus the bomb step and the landing-pad DAT death.
    for _ in 0..11 {
        state.step();
    }

    // The marker should have been replaced by the scanner's bomb.
    let bombed = state.core().get(12);
    assert_eq!(bombed.opcode, Opcode::Dat, "cell 12 should now be a DAT");
    assert_eq!(
        bombed.b.value, 99,
        "cell 12 should have the scanner's bomb signature B=99",
    );

    // The scanner died on its landing pad after bombing.
    assert_eq!(state.result(), MatchResult::AllDead);
}

#[test]
fn parsed_dwarf_outlasts_passive_dat_warrior_in_two_warrior_match() {
    // This test exercises multi-warrior loading via the public API: two
    // separate parsed warriors loaded into different regions of the same
    // core, executing in round-robin alternation, with the dead-warrior
    // skip path kicking in after warrior 1 dies.
    let dwarf = parse_warrior(DWARF).expect("dwarf.red should parse");

    // A deliberately suicidal warrior: a single DAT cell. The first time
    // its process executes, it dies. (Parsed via the same public API, just
    // from an inline source string instead of an .red file.)
    let suicide =
        parse_warrior("DAT.F #0, #0").expect("suicide warrior should parse");

    let mut state = MatchState::new(64, 50);
    state.load_warrior(0, &dwarf, 0);
    state.load_warrior(1, &suicide, 32);

    // Run several steps. The suicide warrior dies on its first turn
    // (step 2); the dwarf is then the unique survivor and wins.
    for _ in 0..10 {
        state.step();
    }

    assert_eq!(
        state.result(),
        MatchResult::Victory { winner_id: 0 },
        "dwarf should win against a passive DAT warrior",
    );
}

/// Warriors that use EQU constants should parse and run correctly through
/// the public API pipeline, just like the canonical .red-file warriors.
#[test]
fn parsed_equ_parameterized_dwarf_bombs_at_custom_interval() {
    // A Dwarf that bombs at intervals of 8 instead of the standard 4,
    // parameterized via EQU. This exercises the parser's EQU substitution
    // through the full parse → load → execute pipeline.
    let source = "
step    EQU 8
        ORG start
start   ADD.AB #step, bomb
        MOV.I  bomb, @bomb
        JMP    start
bomb    DAT.F  #0, #0
    ";
    let parsed = parse_warrior(source).expect("EQU-parameterized dwarf should parse");
    let mut state = MatchState::new(64, 100);
    state.load_warrior(0, &parsed, 0);

    // 5 iterations × 3 instructions = 15 steps. With step=8, bombs land at
    // 3+8=11, 3+16=19, 3+24=27, 3+32=35, 3+40=43 (not at 7 and 15 like
    // the standard step=4 Dwarf).
    for _ in 0..15 {
        state.step();
    }

    // Verify the bombs are at intervals of 8, not 4.
    for (addr, expected_b) in [(11, 8), (19, 16), (27, 24), (35, 32), (43, 40)] {
        let cell = state.core().get(addr);
        assert_eq!(cell.opcode, Opcode::Dat, "cell {addr} should be a DAT bomb");
        assert_eq!(cell.b.value, expected_b);
    }

    // The standard Dwarf bomb positions (7, 15) should still be empty.
    assert_eq!(state.core().get(7).opcode, Opcode::Dat);
    assert_eq!(state.core().get(7).b.value, 0, "cell 7 should NOT have a bomb (step is 8, not 4)");
}

/// The real Mice replicator. After one copy cycle (8 MOV iterations via
/// DJN + 1 SPL + 1 ADD + 1 JMZ = 11 instructions), Mice should have:
///   - written a copy of itself to a remote core region
///   - spawned a new process at the remote copy via SPL
///   - advanced the copy pointer for the next cycle
///   - looped back to start another copy
///
/// We verify replication by checking that the warrior has more than 1
/// process after enough steps (SPL succeeded), and that some cells in
/// the remote region were modified (the copy landed).
#[test]
fn parsed_mice_replicates_itself_to_remote_location() {
    let parsed = parse_warrior(MICE).expect("mice.red should parse");
    let mut state = MatchState::new(8000, 500);
    state.load_warrior(0, &parsed, 0);

    // Run enough steps for at least one full copy cycle + SPL.
    // One cycle: MOV.AB(1) + 8×MOV.I(8) + 8×DJN(8) + SPL(1) + ADD(1) + JMZ(1) = ~28 steps.
    // Run 100 to be safe and allow the remote copy to start its own cycle.
    for _ in 0..100 {
        state.step();
    }

    // Mice should have spawned at least one additional process via SPL.
    assert!(
        state.warriors()[0].process_count() > 1,
        "Mice should have spawned remote processes via SPL, but has only {} process(es)",
        state.warriors()[0].process_count(),
    );

    // The remote copy region (around cell 833, based on copy.B=833) should
    // have been written to. Check that at least some cells in that region
    // are no longer the default DAT.F #0, #0.
    let mut non_dat_count = 0;
    for addr in 825..845 {
        if state.core().get(addr).opcode != Opcode::Dat {
            non_dat_count += 1;
        }
    }
    assert!(
        non_dat_count > 0,
        "Mice should have copied its code to the remote region near cell 833",
    );

    // Warrior should still be alive (Mice never executes a DAT in its loop).
    assert!(state.warriors()[0].is_alive());
}
