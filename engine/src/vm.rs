//! MARS execution: core memory, processes, warriors, and the step function.
//!
//! Implements the full ICWS '94 opcode and addressing mode set:
//!
//!   opcodes (16/16):  DAT, MOV, ADD, SUB, MUL, DIV, MOD,
//!                     JMP, JMZ, JMN, DJN, SPL, SEQ, SNE, SLT, NOP
//!   addressing modes (8/8): Immediate, Direct, AIndirect, BIndirect,
//!                           APredecrement, BPredecrement,
//!                           APostincrement, BPostincrement
//!   modifiers:        all seven for arithmetic / MOV (via modifier_field_pairs);
//!                     .A / .B / .AB / .BA only for DJN / JMZ / JMN / SLT;
//!                     only .I for SEQ / SNE. Multi-field modifier variants
//!                     of the jump and skip opcodes panic — they need
//!                     separate semantics decisions and no current warrior
//!                     needs them.
//!
//! The opcode and addressing-mode matches in `execute` and `resolve` are
//! both *exhaustive*: there is no catch-all arm. If a new variant is added
//! to either enum, the compiler will refuse to build until it's handled.
//!
//! Two opcodes have non-trivial semantics worth calling out:
//!
//!   - SEQ / SNE / SLT introduce the *skip-next-instruction* primitive —
//!     a conditional that advances PC by 2 instead of 1, distinct from a
//!     JMP because there's no target operand. SEQ/SNE compare whole
//!     instructions; SLT compares numeric fields with strict less-than
//!     (it has no .I modifier because there's no defined ordering for
//!     full instructions).
//!
//!   - DIV / MOD kill the executing process on divide-by-zero (same effect
//!     as executing a DAT). This is the only opcode-internal failure mode
//!     that ends a process — every other death comes from running into a
//!     DAT cell directly.
//!
//! The four side-effecting addressing modes (`{ } < >`) all share the
//! same shape: read the intermediate cell, mutate the selected field
//! (decrement before address calc OR increment after), write the
//! intermediate back. This is why `resolve()` takes `&mut Core` rather
//! than `&Core`.
//!
//! Modifier variants that aren't yet implemented panic with a clear
//! "not yet implemented" message rather than silently no-op-ing.

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
///
/// Both fields are private. The process queue is internal scheduling state
/// that must only be mutated via `step()` (and via `MatchState` setup, which
/// lives in the same module and can therefore reach in directly). External
/// code reads queue state through the accessor methods below.
#[derive(Debug, Clone)]
pub struct Warrior {
    id: usize,
    processes: VecDeque<usize>,
}

impl Warrior {
    /// Create a new warrior with a single process at `start_pc`.
    pub fn new(id: usize, start_pc: usize) -> Self {
        let mut processes = VecDeque::new();
        processes.push_back(start_pc);
        Self { id, processes }
    }

    /// The warrior's identifier (assigned at construction).
    pub fn id(&self) -> usize {
        self.id
    }

    /// Whether the warrior still has at least one running process.
    pub fn is_alive(&self) -> bool {
        !self.processes.is_empty()
    }

    /// How many processes this warrior currently has in its queue.
    pub fn process_count(&self) -> usize {
        self.processes.len()
    }

    /// The PC of the next process to run, or `None` if the warrior is dead.
    pub fn next_process_pc(&self) -> Option<usize> {
        self.processes.front().copied()
    }

    /// All process PCs in queue order, front to back. The iterator borrows
    /// from `self`, so it can't outlive the warrior — but it yields owned
    /// `usize`s rather than `&usize`, which is friendlier at call sites.
    pub fn process_pcs(&self) -> impl Iterator<Item = usize> + '_ {
        self.processes.iter().copied()
    }
}

/// The end-state of a battle (or its in-progress state).
///
/// `Tie` and `AllDead` are both "no winner" outcomes but they mean
/// different things diagnostically: `Tie` = "warriors were still trying
/// when the step limit ran out," `AllDead` = "they killed each other (or
/// themselves) before the limit." For scoring purposes neither is a
/// victory, but a frontend or replay viewer may want to display them
/// differently.
///
/// `Victory` is independent of the step limit — if exactly one warrior
/// is alive, that warrior has won regardless of how many steps have
/// elapsed. The limit only matters when there's still more than one
/// warrior alive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchResult {
    /// More than one warrior is alive and the step limit hasn't been reached.
    /// The battle should keep running.
    Ongoing,
    /// Exactly one warrior is still alive — they win.
    Victory { winner_id: usize },
    /// Multiple warriors are still alive but the step limit has been reached.
    /// No clear winner; the match is over.
    Tie,
    /// Every warrior is dead. No survivors, no winner.
    AllDead,
}

/// Full state of an in-progress battle.
///
/// All fields are private. External code reads state via the accessor
/// methods below and mutates state only via `add_warrior` (setup time)
/// or `step` (execution). `core_mut` exists as a deliberate escape hatch
/// for loading initial cells before a battle starts (and for tests
/// constructing scenarios) — it should not be used for anything that
/// resembles "executing instructions."
#[derive(Debug, Clone)]
pub struct MatchState {
    core: Core,
    warriors: Vec<Warrior>,
    /// Number of process-steps that have been executed. Note: in classic
    /// Core War a "cycle" is one step *per warrior*; we count individual
    /// process steps here. Equivalent for single-warrior matches.
    steps: u64,
    max_steps: u64,
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

    /// Add a warrior to the match. Used at setup time, before stepping begins.
    pub fn add_warrior(&mut self, warrior: Warrior) {
        self.warriors.push(warrior);
    }

    /// Load a parsed warrior into core at `base_address` and add it to the
    /// match. Instructions are written sequentially starting at
    /// `base_address`; a new warrior with id `id` is added with its first
    /// process pointing at `base_address + parsed.start_offset()`.
    ///
    /// This is the bridge between the parser and the executor — once a
    /// warrior has been parsed from text via `parse_warrior`, this is how
    /// you get it into a battle.
    pub fn load_warrior(
        &mut self,
        id: usize,
        parsed: &crate::parser::ParsedWarrior,
        base_address: usize,
    ) {
        for (i, &instr) in parsed.instructions().iter().enumerate() {
            self.core_mut().set((base_address + i) as i32, instr);
        }
        let start_pc = base_address + parsed.start_offset();
        self.add_warrior(Warrior::new(id, start_pc));
    }

    /// Read access to the core memory array.
    pub fn core(&self) -> &Core {
        &self.core
    }

    /// Mutable access to the core. Use this to load initial cells before
    /// stepping the simulation; do *not* use it to bypass `step()` once
    /// a battle is in progress.
    pub fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    /// Read access to the warrior list as a slice — external code can
    /// inspect every warrior's state but cannot push, pop, or replace.
    pub fn warriors(&self) -> &[Warrior] {
        &self.warriors
    }

    /// Number of process-steps that have been executed so far.
    pub fn steps(&self) -> u64 {
        self.steps
    }

    /// The configured step limit for this match.
    pub fn max_steps(&self) -> u64 {
        self.max_steps
    }

    /// Classify the current state of the match: ongoing, won by a single
    /// warrior, tied at the step limit, or every warrior dead.
    ///
    /// Note that this is purely a *query* — calling `result()` does not
    /// stop the simulation. `step()` will continue to execute the surviving
    /// warrior even after `result()` reports `Victory`, which is what the
    /// existing tests rely on (`match_continues_for_surviving_warrior_after_other_dies`
    /// runs the imp for eight more steps after the other warrior dies).
    pub fn result(&self) -> MatchResult {
        // Find the first two live warriors. If there are fewer than two,
        // the match outcome is determined by the count alone.
        let mut alive = self.warriors.iter().filter(|w| w.is_alive());
        match (alive.next(), alive.next()) {
            (None, _) => MatchResult::AllDead,
            (Some(only), None) => MatchResult::Victory {
                winner_id: only.id(),
            },
            (Some(_), Some(_)) => {
                if self.steps >= self.max_steps {
                    MatchResult::Tie
                } else {
                    MatchResult::Ongoing
                }
            }
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
                let src = self.core.get(a_eff);
                let dest = self.core.get(b_eff);
                let new_dest =
                    arithmetic_op(src, dest, instr.modifier, core_size_i, |d, s| Some(d + s))
                        .expect("ADD never reports failure");
                self.core.set(b_eff, new_dest);
                self.warriors[warrior_idx].processes.push_back(next_pc);
            }

            Opcode::Sub => {
                let src = self.core.get(a_eff);
                let dest = self.core.get(b_eff);
                let new_dest =
                    arithmetic_op(src, dest, instr.modifier, core_size_i, |d, s| Some(d - s))
                        .expect("SUB never reports failure");
                self.core.set(b_eff, new_dest);
                self.warriors[warrior_idx].processes.push_back(next_pc);
            }

            Opcode::Mul => {
                let src = self.core.get(a_eff);
                let dest = self.core.get(b_eff);
                let new_dest =
                    arithmetic_op(src, dest, instr.modifier, core_size_i, |d, s| Some(d * s))
                        .expect("MUL never reports failure");
                self.core.set(b_eff, new_dest);
                self.warriors[warrior_idx].processes.push_back(next_pc);
            }

            Opcode::Div => {
                // DIV by zero kills the process — same effect as executing a DAT.
                // Returning None from the closure aborts arithmetic_op without
                // writing back any partial results, then we skip enqueueing
                // the next PC, which is exactly the DAT death pattern.
                let src = self.core.get(a_eff);
                let dest = self.core.get(b_eff);
                let new_dest = arithmetic_op(src, dest, instr.modifier, core_size_i, |d, s| {
                    if s == 0 {
                        None
                    } else {
                        Some(d / s)
                    }
                });
                if let Some(new_dest) = new_dest {
                    self.core.set(b_eff, new_dest);
                    self.warriors[warrior_idx].processes.push_back(next_pc);
                }
                // else: divide by zero — process dies, no PC pushed.
            }

            Opcode::Mod => {
                // Same divide-by-zero rule as DIV.
                let src = self.core.get(a_eff);
                let dest = self.core.get(b_eff);
                let new_dest = arithmetic_op(src, dest, instr.modifier, core_size_i, |d, s| {
                    if s == 0 {
                        None
                    } else {
                        Some(d % s)
                    }
                });
                if let Some(new_dest) = new_dest {
                    self.core.set(b_eff, new_dest);
                    self.warriors[warrior_idx].processes.push_back(next_pc);
                }
                // else: mod by zero — process dies, no PC pushed.
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

            Opcode::Seq => {
                // Skip-if-equal: if the source and destination match, the
                // process *skips* the next instruction (PC advances by 2
                // instead of 1). Otherwise it falls through normally.
                //
                // This is the first opcode that introduces the skip-next
                // primitive — strictly different from JMP because there's
                // no target operand, just a conditional advance.
                let src = self.core.get(a_eff);
                let dest = self.core.get(b_eff);
                let equal = match instr.modifier {
                    // Whole-instruction comparison: every field must match.
                    Modifier::I => src == dest,
                    other => {
                        unimplemented!("SEQ modifier {:?} is not yet implemented", other)
                    }
                };
                let resume_pc = if equal {
                    (pc + 2) % core_size
                } else {
                    next_pc
                };
                self.warriors[warrior_idx].processes.push_back(resume_pc);
            }

            Opcode::Sne => {
                // Inverse of SEQ — skip the next instruction when the source
                // and destination *differ*.
                let src = self.core.get(a_eff);
                let dest = self.core.get(b_eff);
                let differ = match instr.modifier {
                    Modifier::I => src != dest,
                    other => {
                        unimplemented!("SNE modifier {:?} is not yet implemented", other)
                    }
                };
                let resume_pc = if differ {
                    (pc + 2) % core_size
                } else {
                    next_pc
                };
                self.warriors[warrior_idx].processes.push_back(resume_pc);
            }

            Opcode::Slt => {
                // Skip-if-less-than: if the source field is strictly less
                // than the destination field, skip the next instruction.
                //
                // Unlike SEQ/SNE, SLT cannot use the .I modifier because
                // there's no defined ordering for whole instructions —
                // less-than only makes sense on numeric fields. The
                // single-field modifiers (.A, .B, .AB, .BA) operate on one
                // (source_field, dest_field) pair each.
                let src = self.core.get(a_eff);
                let dest = self.core.get(b_eff);
                let (sf, df) = match instr.modifier {
                    Modifier::A => (Field::A, Field::A),
                    Modifier::B => (Field::B, Field::B),
                    Modifier::AB => (Field::A, Field::B),
                    Modifier::BA => (Field::B, Field::A),
                    other => {
                        unimplemented!("SLT modifier {:?} is not yet implemented", other)
                    }
                };
                let resume_pc = if src.field(sf) < dest.field(df) {
                    (pc + 2) % core_size
                } else {
                    next_pc
                };
                self.warriors[warrior_idx].processes.push_back(resume_pc);
            }

            Opcode::Jmn => {
                // Inverse of JMZ — jump to A when the destination's selected
                // field is *non-zero*.
                let dest = self.core.get(b_eff);
                let target = self.core.wrap(a_eff);
                let field = match instr.modifier {
                    Modifier::A | Modifier::BA => Field::A,
                    Modifier::B | Modifier::AB => Field::B,
                    other => {
                        unimplemented!("JMN modifier {:?} is not yet implemented", other)
                    }
                };
                if dest.field(field) != 0 {
                    self.warriors[warrior_idx].processes.push_back(target);
                } else {
                    self.warriors[warrior_idx].processes.push_back(next_pc);
                }
            }

            Opcode::Nop => {
                // No-op: do nothing except advance to the next instruction.
                // Operand resolution still happens (which matters for predec/
                // postinc side effects), but no field is read or written.
                self.warriors[warrior_idx].processes.push_back(next_pc);
            }
        }
    }
}

/// Apply a binary arithmetic operation field-wise to a destination instruction,
/// using the modifier to decide which (source_field, dest_field) pairs to
/// operate on. Returns `None` if any iteration of the operation reports
/// failure (used by `DIV` and `MOD` to signal divide-by-zero, which kills
/// the executing process).
///
/// All results are reduced modulo `core_size` so that field values stay in
/// the canonical `[0, core_size)` range.
fn arithmetic_op(
    src: Instruction,
    mut dest: Instruction,
    modifier: Modifier,
    core_size: i32,
    op: impl Fn(i32, i32) -> Option<i32>,
) -> Option<Instruction> {
    for &(sf, df) in modifier_field_pairs(modifier) {
        let result = op(dest.field(df), src.field(sf))?;
        dest.set_field(df, result.rem_euclid(core_size));
    }
    Some(dest)
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

        // {N — predecrement A-indirect. Same pattern as BPredecrement, but
        // operates on the intermediate cell's A-field instead of its B-field.
        AddressMode::APredecrement => {
            let intermediate_addr = pc + op.value;
            let mut intermediate = core.get(intermediate_addr);
            let core_size_i = core.size() as i32;
            let new_a = (intermediate.a.value - 1).rem_euclid(core_size_i);
            intermediate.a.value = new_a;
            core.set(intermediate_addr, intermediate);
            pc + op.value + new_a
        }

        // }N — postincrement A-indirect. Use the *current* A-field value to
        // compute the effective address, THEN increment. The current value
        // is captured before the write-back so the address calculation
        // doesn't see the new value.
        AddressMode::APostincrement => {
            let intermediate_addr = pc + op.value;
            let mut intermediate = core.get(intermediate_addr);
            let current_a = intermediate.a.value;
            let core_size_i = core.size() as i32;
            intermediate.a.value = (current_a + 1).rem_euclid(core_size_i);
            core.set(intermediate_addr, intermediate);
            pc + op.value + current_a
        }

        // >N — postincrement B-indirect. Same pattern as APostincrement on
        // the intermediate's B-field.
        AddressMode::BPostincrement => {
            let intermediate_addr = pc + op.value;
            let mut intermediate = core.get(intermediate_addr);
            let current_b = intermediate.b.value;
            let core_size_i = core.size() as i32;
            intermediate.b.value = (current_b + 1).rem_euclid(core_size_i);
            core.set(intermediate_addr, intermediate);
            pc + op.value + current_b
        }
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

    fn a_predec(v: i32) -> Operand {
        Operand {
            mode: AddressMode::APredecrement,
            value: v,
        }
    }

    fn a_postinc(v: i32) -> Operand {
        Operand {
            mode: AddressMode::APostincrement,
            value: v,
        }
    }

    fn b_postinc(v: i32) -> Operand {
        Operand {
            mode: AddressMode::BPostincrement,
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
        state.add_warrior(Warrior::new(0, 0));
        state.core_mut().set(0, imp());

        // After N steps, cells [0..=N] should all contain the imp:
        // step 1 writes cell 1, step 2 writes cell 2, etc.
        for n in 1..=5 {
            state.step();
            for cell in 0..=n {
                assert_eq!(
                    state.core().get(cell as i32),
                    imp(),
                    "after {n} steps, cell {cell} should be the imp",
                );
            }
        }

        assert!(state.warriors()[0].is_alive(), "imp should still be running");
        assert_eq!(state.steps(), 5);
    }

    #[test]
    fn imp_wraps_around_core() {
        // Tiny core to make the wrap fast.
        let mut state = MatchState::new(4, 100);
        state.add_warrior(Warrior::new(0, 0));
        state.core_mut().set(0, imp());

        // Step enough times to walk past the end of the core. The imp should
        // still be alive (wraparound semantics) and every cell should be imp.
        for _ in 0..10 {
            state.step();
        }

        for cell in 0..4 {
            assert_eq!(state.core().get(cell), imp(), "cell {cell} should be the imp");
        }
        assert!(state.warriors()[0].is_alive());
    }

    #[test]
    fn dat_kills_process() {
        let mut state = MatchState::new(8, 10);
        state.add_warrior(Warrior::new(0, 0));
        // Cell 0 is already DAT.F #0, #0 from Core::new — execute it.

        state.step();

        assert_eq!(
            state.result(),
            MatchResult::AllDead,
            "executing DAT should have killed the only process",
        );
    }

    #[test]
    fn match_ends_when_all_warriors_dead() {
        let mut state = MatchState::new(8, 10);
        state.add_warrior(Warrior::new(0, 0));

        // First step executes DAT and kills the warrior.
        assert!(state.step());
        assert_eq!(state.result(), MatchResult::AllDead);
        // Second step finds no live warriors and reports the match is over.
        assert!(!state.step());
        assert_eq!(state.result(), MatchResult::AllDead);
    }

    #[test]
    fn add_ab_adds_source_a_to_dest_b() {
        // ADD.AB #7, $1   — add 7 (the source's A-field, which for an
        //                   immediate is just the literal value) to the
        //                   destination cell's B-field.
        let mut state = MatchState::new(16, 10);
        state.add_warrior(Warrior::new(0, 0));

        state.core_mut().set(0, instr(Opcode::Add, Modifier::AB, imm(7), dir(1)));
        // Cell 1 starts as DAT.F #0, #5 — we'll watch its B-field grow to 12.
        state
            .core_mut()
            .set(1, instr(Opcode::Dat, Modifier::F, imm(0), imm(5)));

        state.step();

        let cell1 = state.core().get(1);
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
        state.add_warrior(Warrior::new(0, 0));

        state
            .core_mut()
            .set(0, instr(Opcode::Mov, Modifier::I, dir(2), b_ind(1)));
        // Cell 1 — the "pointer". Its B-field of 5 is what makes @1 land on cell 6.
        state
            .core_mut()
            .set(1, instr(Opcode::Dat, Modifier::F, imm(0), imm(5)));
        // Cell 2 — a recognizable source instruction we expect to see at cell 6.
        let marker = instr(Opcode::Jmp, Modifier::B, dir(99), dir(0));
        state.core_mut().set(2, marker);

        state.step();

        assert_eq!(
            state.core().get(6),
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
        state.add_warrior(Warrior::new(0, 0));

        state
            .core_mut()
            .set(0, instr(Opcode::Add, Modifier::AB, imm(4), dir(3)));
        state
            .core_mut()
            .set(1, instr(Opcode::Mov, Modifier::I, dir(2), b_ind(2)));
        state
            .core_mut()
            .set(2, instr(Opcode::Jmp, Modifier::B, dir(-2), dir(0)));
        // Cell 3 is already DAT.F #0, #0 from Core::new — that's the bomb.

        // 5 iterations × 3 instructions per iteration = 15 steps.
        for _ in 0..15 {
            assert!(state.step(), "dwarf should never die — it has no DAT in its loop");
        }

        // Bomb pointer (cell 3's B-field) advanced 5 times by 4.
        assert_eq!(state.core().get(3).b.value, 20);
        assert_eq!(state.core().get(3).opcode, Opcode::Dat);

        // One bomb per iteration, at the bomb pointer's value-at-time-of-MOV.
        // Each bomb is a snapshot of cell 3 with the B-field it had then.
        let expected = [(7, 4), (11, 8), (15, 12), (19, 16), (23, 20)];
        for (addr, expected_b) in expected {
            let cell = state.core().get(addr);
            assert_eq!(cell.opcode, Opcode::Dat, "cell {addr} should be a DAT bomb");
            assert_eq!(
                cell.b.value, expected_b,
                "cell {addr}'s b-field should be {expected_b}",
            );
        }

        // The dwarf's program code itself must be untouched.
        assert_eq!(state.core().get(0).opcode, Opcode::Add);
        assert_eq!(state.core().get(1).opcode, Opcode::Mov);
        assert_eq!(state.core().get(2).opcode, Opcode::Jmp);

        // And the dwarf is still going.
        assert!(state.warriors()[0].is_alive());
    }

    #[test]
    fn spl_adds_a_second_process_to_warrior_queue() {
        // SPL $5 — the executing process continues at PC+1, AND a new
        // process is spawned at PC+5. Both end up in the queue, with the
        // continuing process ahead of the spawned one (per ICWS '94).
        let mut state = MatchState::new(8, 10);
        state.add_warrior(Warrior::new(0, 0));
        state
            .core_mut()
            .set(0, instr(Opcode::Spl, Modifier::B, dir(5), dir(0)));

        assert_eq!(state.warriors()[0].process_count(), 1);

        state.step();

        assert_eq!(state.warriors()[0].process_count(), 2);
        let pcs: Vec<usize> = state.warriors()[0].process_pcs().collect();
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
        state.add_warrior(Warrior::new(0, 0));

        state
            .core_mut()
            .set(0, instr(Opcode::Spl, Modifier::B, dir(10), dir(0)));
        state.core_mut().set(1, imp());
        state.core_mut().set(10, imp());

        for _ in 0..11 {
            assert!(state.step(), "neither imp should die");
        }

        for cell in 1..=6 {
            assert_eq!(
                state.core().get(cell),
                imp(),
                "imp_a trail: cell {cell} should be the imp",
            );
        }
        for cell in 10..=15 {
            assert_eq!(
                state.core().get(cell),
                imp(),
                "imp_b trail: cell {cell} should be the imp",
            );
        }

        // The gap between the two trails must be untouched — proves that
        // neither imp ran ahead of the other and stomped past its expected
        // last cell.
        for cell in 7..=9 {
            assert_eq!(
                state.core().get(cell).opcode,
                Opcode::Dat,
                "gap cell {cell} should still be empty",
            );
        }

        // Both processes alive, in the expected positions.
        assert_eq!(state.warriors()[0].process_count(), 2);
        let pcs: Vec<usize> = state.warriors()[0].process_pcs().collect();
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
        state.add_warrior(Warrior::new(0, 1));

        state
            .core_mut()
            .set(0, instr(Opcode::Dat, Modifier::F, imm(0), imm(3)));
        state
            .core_mut()
            .set(1, instr(Opcode::Djn, Modifier::B, dir(0), dir(-1)));

        // Step 1: counter 3 → 2, jump to cell 1.
        state.step();
        assert_eq!(state.core().get(0).b.value, 2);
        assert_eq!(state.warriors()[0].next_process_pc(), Some(1));

        // Step 2: counter 2 → 1, jump.
        state.step();
        assert_eq!(state.core().get(0).b.value, 1);
        assert_eq!(state.warriors()[0].next_process_pc(), Some(1));

        // Step 3: counter 1 → 0, fall through to cell 2.
        state.step();
        assert_eq!(state.core().get(0).b.value, 0);
        assert_eq!(state.warriors()[0].next_process_pc(), Some(2));

        // Step 4: cell 2 is the default DAT.F #0, #0 — process dies.
        state.step();
        assert!(!state.warriors()[0].is_alive());
    }

    #[test]
    fn jmz_b_jumps_when_destination_b_is_zero() {
        let mut state = MatchState::new(8, 10);
        state.add_warrior(Warrior::new(0, 0));

        // JMZ.B $3, $1  — if cell 1's B == 0, jump to cell 3.
        state
            .core_mut()
            .set(0, instr(Opcode::Jmz, Modifier::B, dir(3), dir(1)));
        // Cell 1 is the default DAT.F #0, #0 — its B is already zero.

        state.step();

        assert_eq!(
            state.warriors()[0].next_process_pc(),
            Some(3),
            "JMZ should have jumped to its A operand because dest.B was zero",
        );
    }

    #[test]
    fn jmz_b_falls_through_when_destination_b_is_nonzero() {
        let mut state = MatchState::new(8, 10);
        state.add_warrior(Warrior::new(0, 0));

        state
            .core_mut()
            .set(0, instr(Opcode::Jmz, Modifier::B, dir(3), dir(1)));
        // Cell 1: DAT.F #0, #5 — B-field is non-zero so JMZ should NOT jump.
        state
            .core_mut()
            .set(1, instr(Opcode::Dat, Modifier::F, imm(0), imm(5)));

        state.step();

        assert_eq!(
            state.warriors()[0].next_process_pc(),
            Some(1),
            "JMZ should have fallen through to PC+1 because dest.B was non-zero",
        );
    }

    #[test]
    fn b_predecrement_decrements_intermediate_b_then_resolves() {
        let mut state = MatchState::new(16, 10);
        state.add_warrior(Warrior::new(0, 0));

        // MOV.I $5, <1
        //   - source: $5     — direct, effective = PC+5 = 5
        //   - dest:   <1     — predec B-indirect: intermediate at PC+1 = cell 1,
        //                      decrement its B (4 → 3), target = 1 + 3 = 4.
        state
            .core_mut()
            .set(0, instr(Opcode::Mov, Modifier::I, dir(5), b_predec(1)));
        // Cell 1 — the pointer cell. Its B field starts at 4 and gets decremented.
        state
            .core_mut()
            .set(1, instr(Opcode::Dat, Modifier::F, imm(0), imm(4)));
        // Cell 5 — recognizable marker we expect to land at cell 4.
        let marker = instr(Opcode::Jmp, Modifier::B, dir(99), dir(0));
        state.core_mut().set(5, marker);

        state.step();

        // The intermediate cell's B was decremented in place.
        assert_eq!(
            state.core().get(1).b.value,
            3,
            "predecrement should have written 3 back to cell 1",
        );
        // And the resolved destination (1 + 3 = 4) received the source.
        assert_eq!(
            state.core().get(4),
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
        state.add_warrior(Warrior::new(0, 3));

        let template = imp();

        state
            .core_mut()
            .set(0, instr(Opcode::Dat, Modifier::F, imm(0), imm(3)));
        state
            .core_mut()
            .set(1, instr(Opcode::Dat, Modifier::F, imm(0), imm(8)));
        state.core_mut().set(2, template);
        state
            .core_mut()
            .set(3, instr(Opcode::Mov, Modifier::I, dir(-1), b_predec(-2)));
        state
            .core_mut()
            .set(4, instr(Opcode::Djn, Modifier::B, dir(-1), dir(-4)));
        // Cell 5 stays as default DAT — the post-loop landing pad.

        for _ in 0..7 {
            state.step();
        }

        // Three copies of the imp template, walking backwards from cell 8.
        for cell in [6, 7, 8] {
            assert_eq!(
                state.core().get(cell),
                template,
                "cell {cell} should hold a copy of the marker",
            );
        }

        // The counter and dest pointer ended in their expected exhausted state.
        assert_eq!(state.core().get(0).b.value, 0, "counter should be exhausted");
        assert_eq!(
            state.core().get(1).b.value,
            5,
            "dest pointer should have decremented 8 → 5",
        );

        // Program code untouched.
        assert_eq!(state.core().get(2), template, "template cell intact");
        assert_eq!(state.core().get(3).opcode, Opcode::Mov, "loop body intact");
        assert_eq!(state.core().get(4).opcode, Opcode::Djn, "DJN body intact");

        // Process fell into the DAT landing pad and died.
        assert_eq!(
            state.result(),
            MatchResult::AllDead,
            "process should have died on the cell-5 DAT",
        );
    }

    #[test]
    fn seq_i_skips_next_when_full_cells_are_equal() {
        let mut state = MatchState::new(8, 10);
        state.add_warrior(Warrior::new(0, 0));

        // SEQ.I $1, $2  — compare cells 1 and 2. Both are default DAT.F #0 #0
        // (untouched by Core::new) so they're equal as full instructions.
        state
            .core_mut()
            .set(0, instr(Opcode::Seq, Modifier::I, dir(1), dir(2)));

        state.step();

        // PC should have advanced by 2 (skipping cell 1) instead of by 1.
        assert_eq!(
            state.warriors()[0].next_process_pc(),
            Some(2),
            "SEQ with equal source/dest should set PC to PC+2",
        );
    }

    #[test]
    fn seq_i_falls_through_when_full_cells_differ() {
        let mut state = MatchState::new(8, 10);
        state.add_warrior(Warrior::new(0, 0));

        // SEQ.I $1, $2 — but plant an imp at cell 1 so it differs from
        // cell 2's default DAT.
        state
            .core_mut()
            .set(0, instr(Opcode::Seq, Modifier::I, dir(1), dir(2)));
        state.core_mut().set(1, imp());

        state.step();

        // PC should have advanced by 1 (no skip).
        assert_eq!(
            state.warriors()[0].next_process_pc(),
            Some(1),
            "SEQ with differing source/dest should set PC to PC+1",
        );
    }

    /// A simple linear scanner. Walks core forward looking for a cell that
    /// differs from a "blank" template; when it finds one, it bombs that
    /// location and dies. This is the third major warrior strategy after
    /// stones (Dwarf) and papers (Mice), and it's the first one that needs
    /// the skip-next-instruction primitive (`SEQ`).
    ///
    /// Layout (process starts at cell 3, the loop body):
    ///
    ///   cell 0: ptr      DAT.F #0, #9       ; scan pointer (B = address being scanned)
    ///   cell 1: blank    DAT.F #0, #0       ; the "empty cell" template
    ///   cell 2: bomb     DAT.F #0, #99      ; the bomb (distinct B for assertion)
    ///   cell 3: loop     ADD.AB #1, $-3     ; ptr.B += 1
    ///   cell 4:          SEQ.I @-4, $-3     ; compare cell-at-ptr to blank
    ///   cell 5:          JMP $2             ; not equal: jump to "found" (cell 7)
    ///   cell 6:          JMP $-3            ; equal:     jump back to loop (cell 3)
    ///   cell 7: found    MOV.I $-5, @-7     ; copy bomb (cell 2) to where ptr points
    ///   cell 8:          DAT.F #0, #0       ; landing pad — process dies here
    ///
    ///   cell 12: marker (a JMP — anything that's not a default DAT)
    ///
    /// The scanner sweeps cells 10, 11, 12 (ptr.B advances 9 → 10 → 11 → 12).
    /// Cells 10 and 11 are empty so SEQ matches and the loop continues.
    /// Cell 12 holds the marker so SEQ falls through and the bomb fires.
    ///
    /// 11 total steps:
    ///   3 scans-that-match × 3 instructions per scan (ADD/SEQ-skip/JMP-back) = 9
    ///   ... wait, only 2 match, then 1 mismatch, then bomb-and-die.
    ///   2 matching iters × 3 = 6
    ///   1 mismatching iter: ADD/SEQ-no-skip/JMP-found/MOV/DAT-death = 5
    ///   Total = 11
    #[test]
    fn simple_scanner_finds_and_bombs_planted_marker() {
        let mut state = MatchState::new(32, 100);
        state.add_warrior(Warrior::new(0, 3));

        state
            .core_mut()
            .set(0, instr(Opcode::Dat, Modifier::F, imm(0), imm(9)));
        state
            .core_mut()
            .set(1, instr(Opcode::Dat, Modifier::F, imm(0), imm(0)));
        state
            .core_mut()
            .set(2, instr(Opcode::Dat, Modifier::F, imm(0), imm(99)));
        state
            .core_mut()
            .set(3, instr(Opcode::Add, Modifier::AB, imm(1), dir(-3)));
        state
            .core_mut()
            .set(4, instr(Opcode::Seq, Modifier::I, b_ind(-4), dir(-3)));
        state
            .core_mut()
            .set(5, instr(Opcode::Jmp, Modifier::B, dir(2), dir(0)));
        state
            .core_mut()
            .set(6, instr(Opcode::Jmp, Modifier::B, dir(-3), dir(0)));
        state
            .core_mut()
            .set(7, instr(Opcode::Mov, Modifier::I, dir(-5), b_ind(-7)));
        // Cell 8 stays as default DAT.F #0 #0 — the landing pad.

        // The marker we expect the scanner to find. Anything that's not a
        // default DAT.F #0 #0 will trigger SEQ to fall through.
        let marker = instr(Opcode::Jmp, Modifier::B, dir(123), dir(456));
        state.core_mut().set(12, marker);

        for _ in 0..11 {
            state.step();
        }

        // The marker at cell 12 should have been replaced by the bomb.
        let bombed = state.core().get(12);
        assert_eq!(
            bombed.opcode,
            Opcode::Dat,
            "cell 12 should now be a DAT (bombed)",
        );
        assert_eq!(
            bombed.b.value, 99,
            "cell 12 should have the bomb's distinctive B value",
        );

        // The scan pointer should have stopped exactly at the marker's address.
        assert_eq!(
            state.core().get(0).b.value,
            12,
            "scan pointer should have stopped at 12",
        );

        // The cells the scanner scanned past must be untouched.
        for cell in [10, 11] {
            assert_eq!(
                state.core().get(cell).opcode,
                Opcode::Dat,
                "scanned-past cell {cell} should still be empty",
            );
            assert_eq!(
                state.core().get(cell).b.value,
                0,
                "scanned-past cell {cell} should still have B=0",
            );
        }

        // The scanner program code itself must be untouched.
        assert_eq!(state.core().get(3).opcode, Opcode::Add);
        assert_eq!(state.core().get(4).opcode, Opcode::Seq);
        assert_eq!(state.core().get(5).opcode, Opcode::Jmp);
        assert_eq!(state.core().get(6).opcode, Opcode::Jmp);
        assert_eq!(state.core().get(7).opcode, Opcode::Mov);

        // Process died on the cell-8 DAT landing pad.
        assert!(
            !state.warriors()[0].is_alive(),
            "scanner should have died on the cell-8 DAT after bombing",
        );
    }

    // ==================================================================
    // Multi-warrior tests — the engine's first battles between two
    // independent warriors sharing a single core.
    // ==================================================================

    /// Two imps at non-overlapping positions, alternating one instruction
    /// per step under the round-robin scheduler. After 10 steps, each
    /// warrior should have executed exactly 5 times.
    ///
    /// This is the multi-warrior version of `imp_propagates_one_cell_per_step`
    /// and is the simplest test that exercises `next_warrior` rotation
    /// across more than one live warrior. Wrong alternation, double-popping
    /// a warrior, or skipping a warrior's turn would all manifest as trails
    /// of unequal length.
    #[test]
    fn two_warriors_alternate_in_round_robin() {
        let mut state = MatchState::new(64, 50);
        state.add_warrior(Warrior::new(0, 0));
        state.add_warrior(Warrior::new(1, 32));
        state.core_mut().set(0, imp());
        state.core_mut().set(32, imp());

        for _ in 0..10 {
            assert!(state.step(), "neither imp should die");
        }

        // Each imp ran 5 times, so each trail spans 6 cells.
        for cell in 0..=5 {
            assert_eq!(
                state.core().get(cell),
                imp(),
                "imp 0 trail: cell {cell} should be the imp",
            );
        }
        for cell in 32..=37 {
            assert_eq!(
                state.core().get(cell),
                imp(),
                "imp 1 trail: cell {cell} should be the imp",
            );
        }

        // Both warriors are still alive with one process each, at the end
        // of their respective trails.
        assert_eq!(state.result(), MatchResult::Ongoing);
        assert_eq!(state.warriors()[0].process_count(), 1);
        assert_eq!(state.warriors()[1].process_count(), 1);
        assert_eq!(state.warriors()[0].next_process_pc(), Some(5));
        assert_eq!(state.warriors()[1].next_process_pc(), Some(37));
    }

    /// When one warrior dies, the surviving warrior should keep running.
    /// This exercises the dead-warrior-skipping path in `step()`: every
    /// subsequent step starts at `next_warrior = 1` (the dead one), fails
    /// the `is_alive` check, falls through to warrior 0, and executes it.
    #[test]
    fn match_continues_for_surviving_warrior_after_other_dies() {
        let mut state = MatchState::new(64, 50);

        // Warrior 0: an imp at cell 0.
        state.add_warrior(Warrior::new(0, 0));
        state.core_mut().set(0, imp());

        // Warrior 1: starts at cell 50, which is the default DAT.F #0 #0.
        // It will die the first time it executes — i.e., on its first turn.
        state.add_warrior(Warrior::new(1, 50));

        // Step 1: imp executes. Both warriors still alive.
        state.step();
        assert_eq!(state.result(), MatchResult::Ongoing);

        // Step 2: warrior 1 executes its DAT and dies — warrior 0 wins.
        state.step();
        assert_eq!(state.result(), MatchResult::Victory { winner_id: 0 });
        assert_eq!(state.warriors()[1].process_count(), 0);

        // Steps 3-10: imp continues, warrior 1 stays dead. After 10 total
        // steps, the imp has executed 9 times (1 in step 1, 8 in steps 3-10),
        // so cells 0..=9 are all imps and the imp's PC is at cell 9.
        for _ in 0..8 {
            assert!(state.step(), "match should still be alive (imp survives)");
        }

        for cell in 0..=9 {
            assert_eq!(
                state.core().get(cell),
                imp(),
                "imp trail: cell {cell} should be the imp",
            );
        }
        assert_eq!(state.result(), MatchResult::Victory { winner_id: 0 });
        assert_eq!(state.warriors()[0].next_process_pc(), Some(9));
    }

    /// **The headline multi-warrior test.** A real Dwarf and a real Scanner
    /// loaded into the same core, fighting it out. The scanner is set up
    /// to find the dwarf on its very first scan iteration (by initializing
    /// its pointer to land on cell 0 after the first `ADD`), bombs cell 0,
    /// and then loops on `JMP $0` at cell 58 instead of dying in a landing
    /// pad — so it survives past the kill as a clean winner.
    ///
    /// Layout:
    ///
    ///   Dwarf at cells 0..=3 (start PC 0):
    ///     0:  ADD.AB #4, $3       ; advance bomb pointer
    ///     1:  MOV.I  $2, @2       ; drop bomb
    ///     2:  JMP    $-2          ; loop
    ///     3:  DAT.F  #0, #0       ; the bomb
    ///
    ///   Scanner at cells 50..=58 (start PC 53):
    ///    50:  DAT.F  #0, #13      ; ptr — first scan after ADD lands at 50+14 = 64 ≡ 0
    ///    51:  DAT.F  #0, #0       ; blank template
    ///    52:  DAT.F  #0, #99      ; bomb (B=99 is the kill signature)
    ///    53:  ADD.AB #1, $-3      ; advance scan ptr
    ///    54:  SEQ.I  @-4, $-3     ; compare cell-at-ptr to blank
    ///    55:  JMP    $2           ; not equal: jump to bomb step
    ///    56:  JMP    $-3          ; equal:     keep scanning
    ///    57:  MOV.I  $-5, @-7     ; bomb the located cell
    ///    58:  JMP    $0           ; survive! loop forever instead of dying
    ///
    /// Hand-traced step-by-step (D = dwarf turn, S = scanner turn):
    ///   1: D ADD     → cell 3.B = 4
    ///   2: S ADD     → ptr.B = 14
    ///   3: D MOV     → cell 7  = DAT.F #0 #4 (dwarf's first bomb)
    ///   4: S SEQ     → cell 0 ≠ blank, no skip, fall through
    ///   5: D JMP     → PC = 0
    ///   6: S JMP $2  → PC = 57
    ///   7: D ADD     → cell 3.B = 8
    ///   8: S MOV     → BOMB cell 0 with DAT.F #0 #99
    ///   9: D MOV     → cell 11 = DAT.F #0 #8 (dwarf's second bomb)
    ///  10: S JMP $0  → loop at 58
    ///  11: D JMP     → PC = 0  (heading to its doom)
    ///  12: S JMP $0  → loop at 58
    ///  13: D DAT     → DWARF EXECUTES THE BOMB AT CELL 0 AND DIES
    ///  14-20: scanner continues looping at cell 58 (dwarf is dead and skipped)
    #[test]
    fn scanner_kills_dwarf_in_head_to_head_battle() {
        let mut state = MatchState::new(64, 200);

        // Dwarf at cells 0..=3, starting PC 0.
        state.add_warrior(Warrior::new(0, 0));
        state
            .core_mut()
            .set(0, instr(Opcode::Add, Modifier::AB, imm(4), dir(3)));
        state
            .core_mut()
            .set(1, instr(Opcode::Mov, Modifier::I, dir(2), b_ind(2)));
        state
            .core_mut()
            .set(2, instr(Opcode::Jmp, Modifier::B, dir(-2), dir(0)));
        // Cell 3 stays as default DAT.F #0 #0 — the bomb.

        // Scanner at cells 50..=58, starting PC 53 (the scan-loop ADD).
        state.add_warrior(Warrior::new(1, 53));
        state
            .core_mut()
            .set(50, instr(Opcode::Dat, Modifier::F, imm(0), imm(13)));
        state
            .core_mut()
            .set(51, instr(Opcode::Dat, Modifier::F, imm(0), imm(0)));
        state
            .core_mut()
            .set(52, instr(Opcode::Dat, Modifier::F, imm(0), imm(99)));
        state
            .core_mut()
            .set(53, instr(Opcode::Add, Modifier::AB, imm(1), dir(-3)));
        state
            .core_mut()
            .set(54, instr(Opcode::Seq, Modifier::I, b_ind(-4), dir(-3)));
        state
            .core_mut()
            .set(55, instr(Opcode::Jmp, Modifier::B, dir(2), dir(0)));
        state
            .core_mut()
            .set(56, instr(Opcode::Jmp, Modifier::B, dir(-3), dir(0)));
        state
            .core_mut()
            .set(57, instr(Opcode::Mov, Modifier::I, dir(-5), b_ind(-7)));
        // Cell 58 is the survival loop: JMP $0 (jump to self).
        state
            .core_mut()
            .set(58, instr(Opcode::Jmp, Modifier::B, dir(0), dir(0)));

        for _ in 0..20 {
            state.step();
        }

        // The scanner bombed cell 0 — the dwarf's main loop entry point.
        let cell0 = state.core().get(0);
        assert_eq!(
            cell0.opcode,
            Opcode::Dat,
            "cell 0 should have been bombed (now a DAT)",
        );
        assert_eq!(
            cell0.b.value, 99,
            "cell 0 should bear the scanner's bomb signature B=99",
        );

        // The scanner won — dwarf is dead, scanner is alive.
        assert_eq!(
            state.result(),
            MatchResult::Victory { winner_id: 1 },
            "scanner (warrior 1) should have won by bombing the dwarf",
        );
        assert_eq!(
            state.warriors()[1].next_process_pc(),
            Some(58),
            "scanner should be looping at its post-kill cell 58",
        );

        // The dwarf got off two bombs before dying — proves it was actually
        // running and not just inert. (If the simulation were broken and
        // the dwarf never executed at all, these cells would still be empty.)
        assert_eq!(state.core().get(7).opcode, Opcode::Dat);
        assert_eq!(state.core().get(7).b.value, 4, "dwarf's first bomb (B=4)");
        assert_eq!(state.core().get(11).opcode, Opcode::Dat);
        assert_eq!(state.core().get(11).b.value, 8, "dwarf's second bomb (B=8)");

        // Scanner program body intact.
        assert_eq!(state.core().get(53).opcode, Opcode::Add);
        assert_eq!(state.core().get(54).opcode, Opcode::Seq);
        assert_eq!(state.core().get(55).opcode, Opcode::Jmp);
        assert_eq!(state.core().get(56).opcode, Opcode::Jmp);
        assert_eq!(state.core().get(57).opcode, Opcode::Mov);
        assert_eq!(state.core().get(58).opcode, Opcode::Jmp);
    }

    // ==================================================================
    // Trivial opcode tests — the SUB / MUL / DIV / MOD / JMN / SNE / NOP
    // additions are all variations on existing opcodes, but each gets a
    // focused unit test to lock in its specific semantics (especially the
    // divide-by-zero death rule for DIV and MOD).
    // ==================================================================

    #[test]
    fn sub_b_subtracts_source_b_from_dest_b() {
        let mut state = MatchState::new(16, 10);
        state.add_warrior(Warrior::new(0, 0));

        // SUB.B $1, $2 — dest.B = dest.B - src.B (cell 2's B - cell 1's B)
        state
            .core_mut()
            .set(0, instr(Opcode::Sub, Modifier::B, dir(1), dir(2)));
        state
            .core_mut()
            .set(1, instr(Opcode::Dat, Modifier::F, imm(0), imm(3)));
        state
            .core_mut()
            .set(2, instr(Opcode::Dat, Modifier::F, imm(0), imm(10)));

        state.step();

        // 10 - 3 = 7
        assert_eq!(state.core().get(2).b.value, 7);
        // .B must not have touched the A field.
        assert_eq!(state.core().get(2).a.value, 0);
    }

    #[test]
    fn mul_b_multiplies_dest_b_by_source_b() {
        let mut state = MatchState::new(64, 10);
        state.add_warrior(Warrior::new(0, 0));

        // MUL.B $1, $2 — dest.B = dest.B * src.B
        state
            .core_mut()
            .set(0, instr(Opcode::Mul, Modifier::B, dir(1), dir(2)));
        state
            .core_mut()
            .set(1, instr(Opcode::Dat, Modifier::F, imm(0), imm(6)));
        state
            .core_mut()
            .set(2, instr(Opcode::Dat, Modifier::F, imm(0), imm(7)));

        state.step();

        // 7 * 6 = 42 (no wrap; well within core size)
        assert_eq!(state.core().get(2).b.value, 42);
    }

    #[test]
    fn div_b_divides_dest_b_by_source_b() {
        let mut state = MatchState::new(64, 10);
        state.add_warrior(Warrior::new(0, 0));

        // DIV.B $1, $2 — dest.B = dest.B / src.B (integer division)
        state
            .core_mut()
            .set(0, instr(Opcode::Div, Modifier::B, dir(1), dir(2)));
        state
            .core_mut()
            .set(1, instr(Opcode::Dat, Modifier::F, imm(0), imm(4)));
        state
            .core_mut()
            .set(2, instr(Opcode::Dat, Modifier::F, imm(0), imm(20)));

        state.step();

        // 20 / 4 = 5
        assert_eq!(state.core().get(2).b.value, 5);
        // Process should still be alive.
        assert_eq!(state.result(), MatchResult::Victory { winner_id: 0 });
    }

    #[test]
    fn div_by_zero_kills_process() {
        let mut state = MatchState::new(8, 10);
        state.add_warrior(Warrior::new(0, 0));

        // DIV.B $1, $2 — but cell 1's B is zero. Divide-by-zero must kill
        // the process, *exactly* as if it had executed a DAT.
        state
            .core_mut()
            .set(0, instr(Opcode::Div, Modifier::B, dir(1), dir(2)));
        state
            .core_mut()
            .set(1, instr(Opcode::Dat, Modifier::F, imm(0), imm(0)));
        state
            .core_mut()
            .set(2, instr(Opcode::Dat, Modifier::F, imm(0), imm(20)));

        state.step();

        // The destination cell must NOT have been modified (the operation
        // aborted before any write).
        assert_eq!(state.core().get(2).b.value, 20, "DIV-by-zero must not write");
        // And the process is dead.
        assert_eq!(state.result(), MatchResult::AllDead);
    }

    #[test]
    fn mod_b_takes_remainder_of_dest_b_over_source_b() {
        let mut state = MatchState::new(64, 10);
        state.add_warrior(Warrior::new(0, 0));

        // MOD.B $1, $2 — dest.B = dest.B mod src.B
        state
            .core_mut()
            .set(0, instr(Opcode::Mod, Modifier::B, dir(1), dir(2)));
        state
            .core_mut()
            .set(1, instr(Opcode::Dat, Modifier::F, imm(0), imm(7)));
        state
            .core_mut()
            .set(2, instr(Opcode::Dat, Modifier::F, imm(0), imm(23)));

        state.step();

        // 23 mod 7 = 2
        assert_eq!(state.core().get(2).b.value, 2);
    }

    #[test]
    fn mod_by_zero_kills_process() {
        let mut state = MatchState::new(8, 10);
        state.add_warrior(Warrior::new(0, 0));

        state
            .core_mut()
            .set(0, instr(Opcode::Mod, Modifier::B, dir(1), dir(2)));
        state
            .core_mut()
            .set(1, instr(Opcode::Dat, Modifier::F, imm(0), imm(0)));
        state
            .core_mut()
            .set(2, instr(Opcode::Dat, Modifier::F, imm(0), imm(23)));

        state.step();

        // No partial write, process dead.
        assert_eq!(state.core().get(2).b.value, 23);
        assert_eq!(state.result(), MatchResult::AllDead);
    }

    #[test]
    fn jmn_b_jumps_when_destination_b_is_nonzero() {
        let mut state = MatchState::new(8, 10);
        state.add_warrior(Warrior::new(0, 0));

        // JMN.B $3, $1 — if cell 1's B != 0, jump to cell 3.
        state
            .core_mut()
            .set(0, instr(Opcode::Jmn, Modifier::B, dir(3), dir(1)));
        state
            .core_mut()
            .set(1, instr(Opcode::Dat, Modifier::F, imm(0), imm(5)));

        state.step();

        assert_eq!(
            state.warriors()[0].next_process_pc(),
            Some(3),
            "JMN should jump because dest.B is non-zero",
        );
    }

    #[test]
    fn jmn_b_falls_through_when_destination_b_is_zero() {
        let mut state = MatchState::new(8, 10);
        state.add_warrior(Warrior::new(0, 0));

        // JMN.B $3, $1 — but cell 1's B is zero (the default), so JMN
        // should NOT jump.
        state
            .core_mut()
            .set(0, instr(Opcode::Jmn, Modifier::B, dir(3), dir(1)));
        // Cell 1 is the default DAT.F #0 #0.

        state.step();

        assert_eq!(
            state.warriors()[0].next_process_pc(),
            Some(1),
            "JMN should fall through because dest.B is zero",
        );
    }

    #[test]
    fn sne_i_skips_next_when_full_cells_differ() {
        let mut state = MatchState::new(8, 10);
        state.add_warrior(Warrior::new(0, 0));

        // SNE.I $1, $2 — cells 1 and 2 differ (imp vs default DAT), so
        // SNE should skip the next instruction.
        state
            .core_mut()
            .set(0, instr(Opcode::Sne, Modifier::I, dir(1), dir(2)));
        state.core_mut().set(1, imp());

        state.step();

        assert_eq!(
            state.warriors()[0].next_process_pc(),
            Some(2),
            "SNE should skip the next instruction when source/dest differ",
        );
    }

    #[test]
    fn sne_i_falls_through_when_full_cells_are_equal() {
        let mut state = MatchState::new(8, 10);
        state.add_warrior(Warrior::new(0, 0));

        // SNE.I $1, $2 — both cells are the default DAT.F #0 #0, so they
        // ARE equal and SNE should NOT skip.
        state
            .core_mut()
            .set(0, instr(Opcode::Sne, Modifier::I, dir(1), dir(2)));

        state.step();

        assert_eq!(
            state.warriors()[0].next_process_pc(),
            Some(1),
            "SNE should fall through when source/dest are equal",
        );
    }

    #[test]
    fn nop_advances_pc_with_no_other_effect() {
        let mut state = MatchState::new(8, 10);
        state.add_warrior(Warrior::new(0, 0));

        // NOP $5, $3 — operands are present but should have no effect on
        // the cells they reference.
        state
            .core_mut()
            .set(0, instr(Opcode::Nop, Modifier::F, dir(5), dir(3)));
        // Snapshot the operand-target cells before stepping.
        let cell3_before = state.core().get(3);
        let cell5_before = state.core().get(5);

        state.step();

        // PC advanced by exactly one.
        assert_eq!(state.warriors()[0].next_process_pc(), Some(1));
        // Operand-target cells are unchanged.
        assert_eq!(state.core().get(3), cell3_before);
        assert_eq!(state.core().get(5), cell5_before);
    }

    /// The `Tie` variant isn't reachable through any of the migrated tests
    /// (none of them run into the step limit). Two imps with a tiny
    /// `max_steps` is the most direct way to force the path: imps don't
    /// die naturally, so when the step counter reaches the limit both
    /// warriors are still alive.
    #[test]
    fn result_is_tie_at_step_limit_with_multiple_warriors_alive() {
        let mut state = MatchState::new(64, 4);
        state.add_warrior(Warrior::new(0, 0));
        state.add_warrior(Warrior::new(1, 32));
        state.core_mut().set(0, imp());
        state.core_mut().set(32, imp());

        // While the match is still progressing, result() reports Ongoing.
        assert_eq!(state.result(), MatchResult::Ongoing);

        // Run exactly to the step limit. After this, step() should refuse
        // to advance further (returns false) and result() should report Tie.
        for _ in 0..4 {
            state.step();
        }

        assert_eq!(
            state.result(),
            MatchResult::Tie,
            "step limit reached with both warriors alive",
        );
        assert_eq!(state.steps(), 4);
        assert_eq!(state.max_steps(), 4);
        assert!(!state.step(), "step() should refuse to advance past max_steps");
    }

    // ==================================================================
    // SLT — skip if less than. Three tests covering the three orderings
    // (less, greater, equal). The equal case matters because SLT is
    // strict less-than: equal values must fall through.
    // ==================================================================

    #[test]
    fn slt_b_skips_when_source_b_less_than_dest_b() {
        let mut state = MatchState::new(8, 10);
        state.add_warrior(Warrior::new(0, 0));

        // SLT.B $1, $2 — if cell 1's B < cell 2's B, skip the next instruction.
        state
            .core_mut()
            .set(0, instr(Opcode::Slt, Modifier::B, dir(1), dir(2)));
        state
            .core_mut()
            .set(1, instr(Opcode::Dat, Modifier::F, imm(0), imm(3)));
        state
            .core_mut()
            .set(2, instr(Opcode::Dat, Modifier::F, imm(0), imm(10)));

        state.step();

        assert_eq!(
            state.warriors()[0].next_process_pc(),
            Some(2),
            "SLT should skip when src.B (3) < dest.B (10)",
        );
    }

    #[test]
    fn slt_b_falls_through_when_source_b_greater_than_dest_b() {
        let mut state = MatchState::new(8, 10);
        state.add_warrior(Warrior::new(0, 0));

        state
            .core_mut()
            .set(0, instr(Opcode::Slt, Modifier::B, dir(1), dir(2)));
        state
            .core_mut()
            .set(1, instr(Opcode::Dat, Modifier::F, imm(0), imm(10)));
        state
            .core_mut()
            .set(2, instr(Opcode::Dat, Modifier::F, imm(0), imm(3)));

        state.step();

        assert_eq!(
            state.warriors()[0].next_process_pc(),
            Some(1),
            "SLT should fall through when src.B (10) > dest.B (3)",
        );
    }

    #[test]
    fn slt_b_falls_through_when_source_b_equals_dest_b() {
        // The strict-less-than edge case: equal values must NOT skip.
        let mut state = MatchState::new(8, 10);
        state.add_warrior(Warrior::new(0, 0));

        state
            .core_mut()
            .set(0, instr(Opcode::Slt, Modifier::B, dir(1), dir(2)));
        state
            .core_mut()
            .set(1, instr(Opcode::Dat, Modifier::F, imm(0), imm(5)));
        state
            .core_mut()
            .set(2, instr(Opcode::Dat, Modifier::F, imm(0), imm(5)));

        state.step();

        assert_eq!(
            state.warriors()[0].next_process_pc(),
            Some(1),
            "SLT is strict less-than — equal values must fall through",
        );
    }

    // ==================================================================
    // The three remaining side-effecting addressing modes. All three
    // mirror the existing BPredecrement test: verify both the side
    // effect on the intermediate cell AND the resolved destination
    // address. The postincrement tests in particular catch any "I used
    // the post-mutated value" bug because the destination is asserted
    // at the *pre-increment* address.
    // ==================================================================

    #[test]
    fn a_predecrement_decrements_intermediate_a_then_resolves() {
        let mut state = MatchState::new(16, 10);
        state.add_warrior(Warrior::new(0, 0));

        // MOV.I $5, {1
        //   - source: $5 — direct, effective = PC+5 = 5
        //   - dest:   {1 — predec A-indirect: intermediate at PC+1 = cell 1,
        //                  decrement its A (4 → 3), target = 1 + 3 = 4.
        state
            .core_mut()
            .set(0, instr(Opcode::Mov, Modifier::I, dir(5), a_predec(1)));
        state
            .core_mut()
            .set(1, instr(Opcode::Dat, Modifier::F, imm(4), imm(0)));
        let marker = instr(Opcode::Jmp, Modifier::B, dir(99), dir(0));
        state.core_mut().set(5, marker);

        state.step();

        assert_eq!(
            state.core().get(1).a.value,
            3,
            "predecrement should have written 3 back to cell 1's A field",
        );
        assert_eq!(
            state.core().get(4),
            marker,
            "MOV destination should have used the post-decrement A value",
        );
    }

    #[test]
    fn a_postincrement_uses_current_a_then_increments() {
        let mut state = MatchState::new(16, 10);
        state.add_warrior(Warrior::new(0, 0));

        // MOV.I $5, }1
        //   - source: $5 — direct, effective = PC+5 = 5
        //   - dest:   }1 — postinc A-indirect: intermediate at PC+1 = cell 1,
        //                  use current A (3), target = 1 + 3 = 4,
        //                  THEN increment cell 1's A to 4.
        state
            .core_mut()
            .set(0, instr(Opcode::Mov, Modifier::I, dir(5), a_postinc(1)));
        state
            .core_mut()
            .set(1, instr(Opcode::Dat, Modifier::F, imm(3), imm(0)));
        let marker = instr(Opcode::Jmp, Modifier::B, dir(99), dir(0));
        state.core_mut().set(5, marker);

        state.step();

        assert_eq!(
            state.core().get(4),
            marker,
            "MOV destination should have used the PRE-increment A value (3)",
        );
        assert_eq!(
            state.core().get(1).a.value,
            4,
            "intermediate A should have been incremented from 3 to 4 after the address calc",
        );
    }

    #[test]
    fn b_postincrement_uses_current_b_then_increments() {
        let mut state = MatchState::new(16, 10);
        state.add_warrior(Warrior::new(0, 0));

        // MOV.I $5, >1
        //   - source: $5 — direct, effective = PC+5 = 5
        //   - dest:   >1 — postinc B-indirect: intermediate at PC+1 = cell 1,
        //                  use current B (3), target = 1 + 3 = 4,
        //                  THEN increment cell 1's B to 4.
        state
            .core_mut()
            .set(0, instr(Opcode::Mov, Modifier::I, dir(5), b_postinc(1)));
        state
            .core_mut()
            .set(1, instr(Opcode::Dat, Modifier::F, imm(0), imm(3)));
        let marker = instr(Opcode::Jmp, Modifier::B, dir(99), dir(0));
        state.core_mut().set(5, marker);

        state.step();

        assert_eq!(
            state.core().get(4),
            marker,
            "MOV destination should have used the PRE-increment B value (3)",
        );
        assert_eq!(
            state.core().get(1).b.value,
            4,
            "intermediate B should have been incremented from 3 to 4 after the address calc",
        );
    }
}
