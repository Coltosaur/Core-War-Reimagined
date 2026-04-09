//! MARS execution: core memory, processes, warriors, and the step function.
//!
//! Currently implements the subset of ICWS '94 needed to run the canonical
//! Imp and Dwarf warriors:
//!
//!   opcodes:          DAT, MOV, ADD, JMP, SPL
//!   modifiers:        all seven (A, B, AB, BA, F, X, I)
//!   addressing modes: Immediate, Direct, AIndirect, BIndirect
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
}
