//! MARS execution: core memory, processes, warriors, and the step function.
//!
//! Currently implements the subset of ICWS '94 needed to run the canonical
//! Imp, Dwarf, and Mice-style replicator warriors:
//!
//!   opcodes:          DAT, MOV, ADD, JMP, SPL, DJN, JMZ
//!   modifiers:        all seven for arithmetic / MOV (via modifier_field_pairs);
//!                     .A and .B only for DJN / JMZ (multi-field variants
//!                     panic — they need a separate semantics decision).
//!   addressing modes: Immediate, Direct, AIndirect, BIndirect, BPredecrement
//!
//! Anything outside that subset panics with a clear "not yet implemented"
//! message rather than silently no-op-ing — partial silence makes broken
//! warriors look like working ones, which is the worst possible failure
//! mode for a simulator. New opcodes / modes get added one at a time, each
//! with its own unit test, and the panic surface shrinks as we go.

use std::collections::VecDeque;

use crate::instruction::{AddressMode, Field, Instruction, Modifier, Opcode, Operand};

/// The shared memory array. Indexing is circular modulo `size()`.
#[derive(Debug, Clone)]
pub struct Core {
    cells: Vec<Instruction>,
}

impl Core {
    pub fn new(size: usize) -> Self {
        assert!(size > 0, "core size must be positive");
        Self {
            cells: vec![Instruction::dat_zero(); size],
        }
    }

    pub fn size(&self) -> usize {
        self.cells.len()
    }

    /// Read a cell at an arbitrary signed address, wrapping modulo size.
    pub fn get(&self, addr: i32) -> Instruction {
        self.cells[self.wrap(addr)]
    }

    /// Write a cell at an arbitrary signed address, wrapping modulo size.
    pub fn set(&mut self, addr: i32, instr: Instruction) {
        let idx = self.wrap(addr);
        self.cells[idx] = instr;
    }

    /// Reduce an arbitrary signed address into a valid core index.
    /// Uses Euclidean modulo so negative addresses wrap correctly.
    pub fn wrap(&self, addr: i32) -> usize {
        let size = self.cells.len() as i32;
        ((addr % size + size) % size) as usize
    }
}

/// One warrior — its identity and its FIFO queue of running processes.
/// An empty queue means the warrior is dead.
#[derive(Debug, Clone)]
pub struct Warrior {
    pub id: usize,
    pub processes: VecDeque<usize>,
}

impl Warrior {
    /// Create a new warrior with a single process at `start_pc`.
    pub fn new(id: usize, start_pc: usize) -> Self {
        let mut processes = VecDeque::new();
        processes.push_back(start_pc);
        Self { id, processes }
    }

    pub fn is_alive(&self) -> bool {
        !self.processes.is_empty()
    }
}

/// Full state of an in-progress battle.
#[derive(Debug, Clone)]
pub struct MatchState {
    pub core: Core,
    pub warriors: Vec<Warrior>,
    /// Number of process-steps that have been executed. Note: in classic
    /// Core War a "cycle" is one step *per warrior*; we count individual
    /// process steps here. Equivalent for single-warrior matches.
    pub steps: u64,
    pub max_steps: u64,
    /// Round-robin index — the warrior whose turn it is next.
    next_warrior: usize,
}

impl MatchState {
    pub fn new(core_size: usize, max_steps: u64) -> Self {
        Self {
            core: Core::new(core_size),
            warriors: Vec::new(),
            steps: 0,
            max_steps,
            next_warrior: 0,
        }
    }

    /// Advance the simulation by one process-step (one instruction for one
    /// process of the next live warrior in round-robin order).
    ///
    /// Returns `false` if the match is over (all warriors dead or step
    /// limit reached) and no more progress can be made.
    pub fn step(&mut self) -> bool {
        if self.steps >= self.max_steps {
            return false;
        }

        let n = self.warriors.len();
        if n == 0 {
            return false;
        }

        // Find the next live warrior, starting from `next_warrior`.
        let mut idx = self.next_warrior % n;
        let mut tries = 0;
        while !self.warriors[idx].is_alive() {
            idx = (idx + 1) % n;
            tries += 1;
            if tries >= n {
                return false; // every warrior is dead
            }
        }
        self.next_warrior = (idx + 1) % n;

        // Pop the next process for this warrior, fetch and execute its
        // current instruction.
        let pc = self.warriors[idx]
            .processes
            .pop_front()
            .expect("warrior was alive but had no processes");
        let instr = self.core.get(pc as i32);
        self.execute(idx, pc, instr);

        self.steps += 1;
        true
    }

    fn execute(&mut self, warrior_idx: usize, pc: usize, instr: Instruction) {
        let pc_i = pc as i32;
        let core_size = self.core.size();
        let core_size_i = core_size as i32;
        let next_pc = (pc + 1) % core_size;

        // Resolve effective addresses for both operands. ICWS '94 specifies
        // A is resolved before B, which matters once predec/postinc modes
        // with side effects are added.
        let a_eff = resolve(pc_i, instr.a, &mut self.core);
        let b_eff = resolve(pc_i, instr.b, &mut self.core);

        match instr.opcode {
            Opcode::Dat => {
                // DAT terminates the executing process — nothing is enqueued.
            }

            Opcode::Mov => {
                if instr.modifier == Modifier::I {
                    // Whole-instruction copy: source replaces destination cell.
                    let src = self.core.get(a_eff);
                    self.core.set(b_eff, src);
                } else {
                    // Field-wise copy: only the integer value of selected fields
                    // is copied; addressing modes of the destination are preserved.
                    let src = self.core.get(a_eff);
                    let mut dest = self.core.get(b_eff);
                    for &(sf, df) in modifier_field_pairs(instr.modifier) {
                        dest.set_field(df, src.field(sf));
                    }
                    self.core.set(b_eff, dest);
                }
                self.warriors[warrior_idx].processes.push_back(next_pc);
            }

            Opcode::Add => {
                // Field-wise addition. .I is treated identically to .F here,
                // per the ICWS '94 spec for arithmetic opcodes.
                let src = self.core.get(a_eff);
                let mut dest = self.core.get(b_eff);
                for &(sf, df) in modifier_field_pairs(instr.modifier) {
                    let sum = (dest.field(df) + src.field(sf)).rem_euclid(core_size_i);
                    dest.set_field(df, sum);
                }
                self.core.set(b_eff, dest);
                self.warriors[warrior_idx].processes.push_back(next_pc);
            }

            Opcode::Jmp => {
                let target = self.core.wrap(a_eff);
                self.warriors[warrior_idx].processes.push_back(target);
            }

            Opcode::Spl => {
                // Continue at next instruction AND spawn a new process at A.
                self.warriors[warrior_idx].processes.push_back(next_pc);
                let target = self.core.wrap(a_eff);
                self.warriors[warrior_idx].processes.push_back(target);
            }

            Opcode::Djn => {
                // Decrement the destination's selected field, then jump to A
                // if the *new* value is non-zero. Modifiers .F/.X/.I would
                // decrement both fields and have a slightly different jump
                // condition; not yet implemented.
                let mut dest = self.core.get(b_eff);
                let target = self.core.wrap(a_eff);
                let field = match instr.modifier {
                    Modifier::A | Modifier::BA => Field::A,
                    Modifier::B | Modifier::AB => Field::B,
                    other => {
                        unimplemented!("DJN modifier {:?} is not yet implemented", other)
                    }
                };
                let new_val = (dest.field(field) - 1).rem_euclid(core_size_i);
                dest.set_field(field, new_val);
                self.core.set(b_eff, dest);
                if new_val != 0 {
                    self.warriors[warrior_idx].processes.push_back(target);
                } else {
                    self.warriors[warrior_idx].processes.push_back(next_pc);
                }
            }

            Opcode::Jmz => {
                // Jump to A if the destination's selected field is zero.
                let dest = self.core.get(b_eff);
                let target = self.core.wrap(a_eff);
                let field = match instr.modifier {
                    Modifier::A | Modifier::BA => Field::A,
                    Modifier::B | Modifier::AB => Field::B,
                    other => {
                        unimplemented!("JMZ modifier {:?} is not yet implemented", other)
                    }
                };
                if dest.field(field) == 0 {
                    self.warriors[warrior_idx].processes.push_back(target);
                } else {
                    self.warriors[warrior_idx].processes.push_back(next_pc);
                }
            }

            // Every other opcode: panic loudly so we know exactly what to
            // implement next when a warrior reaches it. Silent no-ops were
            // catching real bugs as "the warrior just keeps running fine".
            other => unimplemented!("opcode {:?} is not yet implemented", other),
        }
    }
}

/// For arithmetic and field-wise MOV operations, the (source_field, dest_field)
/// pairs that the modifier expands to.
///
/// `.I` is treated as `.F` here — for arithmetic ops the spec says they're
/// equivalent, and for `MOV` the whole-instruction copy is handled separately
/// in the opcode body, so this fallback is only ever consulted for the
/// arithmetic case.
fn modifier_field_pairs(m: Modifier) -> &'static [(Field, Field)] {
    use Field::{A, B};
    match m {
        Modifier::A => &[(A, A)],
        Modifier::B => &[(B, B)],
        Modifier::AB => &[(A, B)],
        Modifier::BA => &[(B, A)],
        Modifier::F => &[(A, A), (B, B)],
        Modifier::X => &[(A, B), (B, A)],
        Modifier::I => &[(A, A), (B, B)],
    }
}

/// Resolve an operand to an effective core address relative to the executing PC.
///
/// Takes `&mut Core` so that future predecrement / postincrement modes can
/// mutate the intermediate cell as part of resolution. Direct, Immediate,
/// AIndirect, and BIndirect do not mutate.
fn resolve(pc: i32, op: Operand, core: &mut Core) -> i32 {
    match op.mode {
        AddressMode::Direct => pc + op.value,

        // Per ICWS '94, an immediate operand "points to" the cell containing
        // the executing instruction itself.
        AddressMode::Immediate => pc,

        // *N — read the intermediate cell at PC+N, then offset by its A-field.
        AddressMode::AIndirect => {
            let intermediate = core.get(pc + op.value);
            pc + op.value + intermediate.a.value
        }

        // @N — read the intermediate cell at PC+N, then offset by its B-field.
        AddressMode::BIndirect => {
            let intermediate = core.get(pc + op.value);
            pc + op.value + intermediate.b.value
        }

        // <N — predecrement B-indirect: read the intermediate cell at PC+N,
        // decrement its B-field IN PLACE (writing back to core), then offset
        // by the new B-field value. The mutation is observable to subsequent
        // reads — this is the whole reason resolve() takes &mut Core.
        AddressMode::BPredecrement => {
            let intermediate_addr = pc + op.value;
            let mut intermediate = core.get(intermediate_addr);
            let core_size_i = core.size() as i32;
            let new_b = (intermediate.b.value - 1).rem_euclid(core_size_i);
            intermediate.b.value = new_b;
            core.set(intermediate_addr, intermediate);
            pc + op.value + new_b
        }

        other => unimplemented!("addressing mode {:?} is not yet implemented", other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruction::{AddressMode, Instruction, Modifier, Opcode, Operand};

    /// Convenience for building an `Instruction` in a test without rendering
    /// the full struct literal six lines tall every time.
    fn instr(opcode: Opcode, modifier: Modifier, a: Operand, b: Operand) -> Instruction {
        Instruction {
            opcode,
            modifier,
            a,
            b,
        }
    }

    fn imm(v: i32) -> Operand {
        Operand {
            mode: AddressMode::Immediate,
            value: v,
        }
    }

    fn dir(v: i32) -> Operand {
        Operand {
            mode: AddressMode::Direct,
            value: v,
        }
    }

    fn b_ind(v: i32) -> Operand {
        Operand {
            mode: AddressMode::BIndirect,
            value: v,
        }
    }

    fn b_predec(v: i32) -> Operand {
        Operand {
            mode: AddressMode::BPredecrement,
            value: v,
        }
    }

    /// The canonical Imp: `MOV.I $0, $1`. Copies itself one cell forward
    /// every step, walking through core forever.
    fn imp() -> Instruction {
        instr(Opcode::Mov, Modifier::I, dir(0), dir(1))
    }

    #[test]
    fn imp_propagates_one_cell_per_step() {
        let mut state = MatchState::new(8000, 100);
        state.warriors.push(Warrior::new(0, 0));
        state.core.set(0, imp());

        // After N steps, cells [0..=N] should all contain the imp:
        // step 1 writes cell 1, step 2 writes cell 2, etc.
        for n in 1..=5 {
            state.step();
            for cell in 0..=n {
                assert_eq!(
                    state.core.get(cell as i32),
                    imp(),
                    "after {n} steps, cell {cell} should be the imp",
                );
            }
        }

        assert!(state.warriors[0].is_alive(), "imp should still be running");
        assert_eq!(state.steps, 5);
    }

    #[test]
    fn imp_wraps_around_core() {
        // Tiny core to make the wrap fast.
        let mut state = MatchState::new(4, 100);
        state.warriors.push(Warrior::new(0, 0));
        state.core.set(0, imp());

        // Step enough times to walk past the end of the core. The imp should
        // still be alive (wraparound semantics) and every cell should be imp.
        for _ in 0..10 {
            state.step();
        }

        for cell in 0..4 {
            assert_eq!(state.core.get(cell), imp(), "cell {cell} should be the imp");
        }
        assert!(state.warriors[0].is_alive());
    }

    #[test]
    fn dat_kills_process() {
        let mut state = MatchState::new(8, 10);
        state.warriors.push(Warrior::new(0, 0));
        // Cell 0 is already DAT.F #0, #0 from Core::new — execute it.

        state.step();

        assert!(
            !state.warriors[0].is_alive(),
            "executing DAT should have killed the only process",
        );
    }

    #[test]
    fn match_ends_when_all_warriors_dead() {
        let mut state = MatchState::new(8, 10);
        state.warriors.push(Warrior::new(0, 0));

        // First step executes DAT and kills the warrior.
        assert!(state.step());
        // Second step finds no live warriors and reports the match is over.
        assert!(!state.step());
    }

    #[test]
    fn add_ab_adds_source_a_to_dest_b() {
        // ADD.AB #7, $1   — add 7 (the source's A-field, which for an
        //                   immediate is just the literal value) to the
        //                   destination cell's B-field.
        let mut state = MatchState::new(16, 10);
        state.warriors.push(Warrior::new(0, 0));

        state.core.set(0, instr(Opcode::Add, Modifier::AB, imm(7), dir(1)));
        // Cell 1 starts as DAT.F #0, #5 — we'll watch its B-field grow to 12.
        state
            .core
            .set(1, instr(Opcode::Dat, Modifier::F, imm(0), imm(5)));

        state.step();

        let cell1 = state.core.get(1);
        assert_eq!(cell1.b.value, 12, "5 + 7 should be 12");
        assert_eq!(cell1.a.value, 0, ".AB must not touch the destination's A field");
    }

    #[test]
    fn b_indirect_resolves_through_intermediate_cell() {
        // MOV.I $2, @1
        //   - source: $2 — direct, effective = PC+2 = 2
        //   - dest:   @1 — B-indirect: intermediate = PC+1 = cell 1, then
        //                  add cell 1's B-field (5) to that, giving target 6.
        let mut state = MatchState::new(16, 10);
        state.warriors.push(Warrior::new(0, 0));

        state
            .core
            .set(0, instr(Opcode::Mov, Modifier::I, dir(2), b_ind(1)));
        // Cell 1 — the "pointer". Its B-field of 5 is what makes @1 land on cell 6.
        state
            .core
            .set(1, instr(Opcode::Dat, Modifier::F, imm(0), imm(5)));
        // Cell 2 — a recognizable source instruction we expect to see at cell 6.
        let marker = instr(Opcode::Jmp, Modifier::B, dir(99), dir(0));
        state.core.set(2, marker);

        state.step();

        assert_eq!(
            state.core.get(6),
            marker,
            "B-indirect destination should have landed on cell 1+5=6",
        );
    }

    /// Dwarf — A. K. Dewdney's classic stone warrior. Bombs core at intervals
    /// of 4, marching ever-further from its starting position. Per iteration:
    ///
    ///   ADD.AB #4, $3   ; cell 3's B-field += 4 (advance the bomb pointer)
    ///   MOV.I  $2, @2   ; copy cell 3 (the DAT bomb) to wherever cell 3's
    ///                   ;   B-field now points, relative to cell 3
    ///   JMP    -2       ; loop back to start
    ///   DAT.F  #0, #0   ; the "bomb" — also the pointer state
    ///
    /// After N iterations, cell 3's B-field is 4*N and there are N DAT bombs
    /// at addresses 3+4, 3+8, ..., 3+4*N. Each bomb is a snapshot of cell 3
    /// at the time it was thrown.
    #[test]
    fn dwarf_bombs_core_at_intervals_of_four() {
        let mut state = MatchState::new(64, 100);
        state.warriors.push(Warrior::new(0, 0));

        state
            .core
            .set(0, instr(Opcode::Add, Modifier::AB, imm(4), dir(3)));
        state
            .core
            .set(1, instr(Opcode::Mov, Modifier::I, dir(2), b_ind(2)));
        state
            .core
            .set(2, instr(Opcode::Jmp, Modifier::B, dir(-2), dir(0)));
        // Cell 3 is already DAT.F #0, #0 from Core::new — that's the bomb.

        // 5 iterations × 3 instructions per iteration = 15 steps.
        for _ in 0..15 {
            assert!(state.step(), "dwarf should never die — it has no DAT in its loop");
        }

        // Bomb pointer (cell 3's B-field) advanced 5 times by 4.
        assert_eq!(state.core.get(3).b.value, 20);
        assert_eq!(state.core.get(3).opcode, Opcode::Dat);

        // One bomb per iteration, at the bomb pointer's value-at-time-of-MOV.
        // Each bomb is a snapshot of cell 3 with the B-field it had then.
        let expected = [(7, 4), (11, 8), (15, 12), (19, 16), (23, 20)];
        for (addr, expected_b) in expected {
            let cell = state.core.get(addr);
            assert_eq!(cell.opcode, Opcode::Dat, "cell {addr} should be a DAT bomb");
            assert_eq!(
                cell.b.value, expected_b,
                "cell {addr}'s b-field should be {expected_b}",
            );
        }

        // The dwarf's program code itself must be untouched.
        assert_eq!(state.core.get(0).opcode, Opcode::Add);
        assert_eq!(state.core.get(1).opcode, Opcode::Mov);
        assert_eq!(state.core.get(2).opcode, Opcode::Jmp);

        // And the dwarf is still going.
        assert!(state.warriors[0].is_alive());
    }

    #[test]
    fn spl_adds_a_second_process_to_warrior_queue() {
        // SPL $5 — the executing process continues at PC+1, AND a new
        // process is spawned at PC+5. Both end up in the queue, with the
        // continuing process ahead of the spawned one (per ICWS '94).
        let mut state = MatchState::new(8, 10);
        state.warriors.push(Warrior::new(0, 0));
        state
            .core
            .set(0, instr(Opcode::Spl, Modifier::B, dir(5), dir(0)));

        assert_eq!(state.warriors[0].processes.len(), 1);

        state.step();

        assert_eq!(state.warriors[0].processes.len(), 2);
        let pcs: Vec<usize> = state.warriors[0].processes.iter().copied().collect();
        assert_eq!(
            pcs,
            vec![1, 5],
            "queue should be [continuing_pc, spawned_pc] after SPL",
        );
    }

    /// Multi-process imp ring. Demonstrates that within a single warrior,
    /// the FIFO process queue interleaves processes one-instruction-at-a-time
    /// — *not* one-process-runs-to-completion-then-the-next.
    ///
    /// Setup:
    ///   cell 0:  SPL  $10        ; spawn a second process at cell 10
    ///   cell 1:  MOV.I $0, $1    ; imp_a — the original process falls into this
    ///   cell 10: MOV.I $0, $1    ; imp_b — the spawned process starts here
    ///
    /// Trace (queue shown as [front, ..., back] after each step):
    ///   step 1:  SPL    queue=[1, 10]
    ///   step 2:  imp_a  copies cell 1→2,    queue=[10, 2]
    ///   step 3:  imp_b  copies cell 10→11,  queue=[2, 11]
    ///   step 4:  imp_a  copies cell 2→3,    queue=[11, 3]
    ///   ... and so on, alternating perfectly between the two processes.
    ///
    /// After 11 steps total (1 SPL + 5 imp_a runs + 5 imp_b runs), each
    /// imp has advanced exactly 5 cells, so cells 1..=6 and 10..=15 are
    /// all imps. Wrong scheduling (one process running ahead, or neither
    /// alternating) would produce trails of unequal length.
    #[test]
    fn spl_creates_two_imps_walking_in_alternation() {
        let mut state = MatchState::new(32, 100);
        state.warriors.push(Warrior::new(0, 0));

        state
            .core
            .set(0, instr(Opcode::Spl, Modifier::B, dir(10), dir(0)));
        state.core.set(1, imp());
        state.core.set(10, imp());

        for _ in 0..11 {
            assert!(state.step(), "neither imp should die");
        }

        for cell in 1..=6 {
            assert_eq!(
                state.core.get(cell),
                imp(),
                "imp_a trail: cell {cell} should be the imp",
            );
        }
        for cell in 10..=15 {
            assert_eq!(
                state.core.get(cell),
                imp(),
                "imp_b trail: cell {cell} should be the imp",
            );
        }

        // The gap between the two trails must be untouched — proves that
        // neither imp ran ahead of the other and stomped past its expected
        // last cell.
        for cell in 7..=9 {
            assert_eq!(
                state.core.get(cell).opcode,
                Opcode::Dat,
                "gap cell {cell} should still be empty",
            );
        }

        // Both processes alive, in the expected positions.
        assert_eq!(state.warriors[0].processes.len(), 2);
        let pcs: Vec<usize> = state.warriors[0].processes.iter().copied().collect();
        assert_eq!(
            pcs,
            vec![6, 15],
            "queue should be [imp_a_pc=6, imp_b_pc=15] after 11 steps",
        );
    }

    #[test]
    fn djn_b_decrements_and_loops_until_zero() {
        // Cell 0: counter, B starts at 3.
        // Cell 1: DJN.B $0, $-1   — A=0 jumps back to itself, B=-1 targets cell 0.
        // Each execution: counter.B--, then jump to self if non-zero.
        let mut state = MatchState::new(8, 20);
        state.warriors.push(Warrior::new(0, 1));

        state
            .core
            .set(0, instr(Opcode::Dat, Modifier::F, imm(0), imm(3)));
        state
            .core
            .set(1, instr(Opcode::Djn, Modifier::B, dir(0), dir(-1)));

        // Step 1: counter 3 → 2, jump to cell 1.
        state.step();
        assert_eq!(state.core.get(0).b.value, 2);
        assert_eq!(state.warriors[0].processes.front(), Some(&1));

        // Step 2: counter 2 → 1, jump.
        state.step();
        assert_eq!(state.core.get(0).b.value, 1);
        assert_eq!(state.warriors[0].processes.front(), Some(&1));

        // Step 3: counter 1 → 0, fall through to cell 2.
        state.step();
        assert_eq!(state.core.get(0).b.value, 0);
        assert_eq!(state.warriors[0].processes.front(), Some(&2));

        // Step 4: cell 2 is the default DAT.F #0, #0 — process dies.
        state.step();
        assert!(!state.warriors[0].is_alive());
    }

    #[test]
    fn jmz_b_jumps_when_destination_b_is_zero() {
        let mut state = MatchState::new(8, 10);
        state.warriors.push(Warrior::new(0, 0));

        // JMZ.B $3, $1  — if cell 1's B == 0, jump to cell 3.
        state
            .core
            .set(0, instr(Opcode::Jmz, Modifier::B, dir(3), dir(1)));
        // Cell 1 is the default DAT.F #0, #0 — its B is already zero.

        state.step();

        assert_eq!(
            state.warriors[0].processes.front(),
            Some(&3),
            "JMZ should have jumped to its A operand because dest.B was zero",
        );
    }

    #[test]
    fn jmz_b_falls_through_when_destination_b_is_nonzero() {
        let mut state = MatchState::new(8, 10);
        state.warriors.push(Warrior::new(0, 0));

        state
            .core
            .set(0, instr(Opcode::Jmz, Modifier::B, dir(3), dir(1)));
        // Cell 1: DAT.F #0, #5 — B-field is non-zero so JMZ should NOT jump.
        state
            .core
            .set(1, instr(Opcode::Dat, Modifier::F, imm(0), imm(5)));

        state.step();

        assert_eq!(
            state.warriors[0].processes.front(),
            Some(&1),
            "JMZ should have fallen through to PC+1 because dest.B was non-zero",
        );
    }

    #[test]
    fn b_predecrement_decrements_intermediate_b_then_resolves() {
        let mut state = MatchState::new(16, 10);
        state.warriors.push(Warrior::new(0, 0));

        // MOV.I $5, <1
        //   - source: $5     — direct, effective = PC+5 = 5
        //   - dest:   <1     — predec B-indirect: intermediate at PC+1 = cell 1,
        //                      decrement its B (4 → 3), target = 1 + 3 = 4.
        state
            .core
            .set(0, instr(Opcode::Mov, Modifier::I, dir(5), b_predec(1)));
        // Cell 1 — the pointer cell. Its B field starts at 4 and gets decremented.
        state
            .core
            .set(1, instr(Opcode::Dat, Modifier::F, imm(0), imm(4)));
        // Cell 5 — recognizable marker we expect to land at cell 4.
        let marker = instr(Opcode::Jmp, Modifier::B, dir(99), dir(0));
        state.core.set(5, marker);

        state.step();

        // The intermediate cell's B was decremented in place.
        assert_eq!(
            state.core.get(1).b.value,
            3,
            "predecrement should have written 3 back to cell 1",
        );
        // And the resolved destination (1 + 3 = 4) received the source.
        assert_eq!(
            state.core.get(4),
            marker,
            "MOV destination should have used the post-decrement B value",
        );
    }

    /// "Mice-lite" — a simplified version of Chip Wendell's 1986 Mice that
    /// uses the same primitives (counter + predecrement-B copy loop) but
    /// without the SPL/JMZ tail. Demonstrates DJN, predec-B, and MOV.I
    /// working together as a real replicator instead of just a contrived
    /// micro-test. Three iterations copy the imp template into three
    /// consecutive cells, walking *backwards* as the dest pointer
    /// predecrements each iteration.
    ///
    /// Layout (process starts at cell 3, the loop body):
    ///
    ///   cell 0: counter   DAT.F #0, #3       ; loop count
    ///   cell 1: dest      DAT.F #0, #8       ; copy pointer (B = 8 initially)
    ///   cell 2: template  MOV.I $0, $1       ; the marker we replicate
    ///   cell 3: loop      MOV.I $-1, <-2     ; copy template to predec(dest)
    ///   cell 4:           DJN.B $-1, $-4     ; counter--, loop if non-zero
    ///   cell 5:           DAT.F #0, #0       ; landing pad — process dies here
    ///
    /// Trace of one full iteration starting at cell 3:
    ///   step:  exec cell 3:  copy cell 2 → predec dest (8 → 7), target = 3-2+7 = 8
    ///   step:  exec cell 4:  counter 3 → 2, jump back to cell 3
    ///   step:  exec cell 3:  copy cell 2 → predec dest (7 → 6), target = 3-2+6 = 7
    ///   step:  exec cell 4:  counter 2 → 1, jump to cell 3
    ///   step:  exec cell 3:  copy cell 2 → predec dest (6 → 5), target = 3-2+5 = 6
    ///   step:  exec cell 4:  counter 1 → 0, fall through to cell 5
    ///   step:  exec cell 5:  default DAT — process dies.
    ///
    /// After 7 steps: imp template lives at cells 6, 7, 8; counter == 0;
    /// dest.B == 5; process is dead.
    #[test]
    fn mice_lite_replicator_copies_marker_three_times() {
        let mut state = MatchState::new(32, 50);
        state.warriors.push(Warrior::new(0, 3));

        let template = imp();

        state
            .core
            .set(0, instr(Opcode::Dat, Modifier::F, imm(0), imm(3)));
        state
            .core
            .set(1, instr(Opcode::Dat, Modifier::F, imm(0), imm(8)));
        state.core.set(2, template);
        state
            .core
            .set(3, instr(Opcode::Mov, Modifier::I, dir(-1), b_predec(-2)));
        state
            .core
            .set(4, instr(Opcode::Djn, Modifier::B, dir(-1), dir(-4)));
        // Cell 5 stays as default DAT — the post-loop landing pad.

        for _ in 0..7 {
            state.step();
        }

        // Three copies of the imp template, walking backwards from cell 8.
        for cell in [6, 7, 8] {
            assert_eq!(
                state.core.get(cell),
                template,
                "cell {cell} should hold a copy of the marker",
            );
        }

        // The counter and dest pointer ended in their expected exhausted state.
        assert_eq!(state.core.get(0).b.value, 0, "counter should be exhausted");
        assert_eq!(
            state.core.get(1).b.value,
            5,
            "dest pointer should have decremented 8 → 5",
        );

        // Program code untouched.
        assert_eq!(state.core.get(2), template, "template cell intact");
        assert_eq!(state.core.get(3).opcode, Opcode::Mov, "loop body intact");
        assert_eq!(state.core.get(4).opcode, Opcode::Djn, "DJN body intact");

        // Process fell into the DAT landing pad and died.
        assert!(
            !state.warriors[0].is_alive(),
            "process should have died on the cell-5 DAT",
        );
    }
}
