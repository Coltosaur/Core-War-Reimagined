//! MARS execution: core memory, processes, warriors, and the step function.
//!
//! Currently implements only the minimum needed to run an Imp:
//!   - addressing modes: Direct, Immediate
//!   - opcodes:          Dat, Mov.I, Jmp, Spl
//!
//! Other opcodes/modes/modifiers fall through as no-ops (process keeps running)
//! so the simulator stays usable while it's incrementally fleshed out. Each
//! addition is a localized change in `execute` and `resolve` plus a new test.

use std::collections::VecDeque;

use crate::instruction::{AddressMode, Instruction, Modifier, Opcode, Operand};

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
        let next_pc = (pc + 1) % core_size;

        // Resolve effective addresses for both operands.
        let a_eff = resolve(pc_i, instr.a);
        let b_eff = resolve(pc_i, instr.b);

        match instr.opcode {
            Opcode::Dat => {
                // DAT terminates the executing process — nothing is enqueued.
            }

            Opcode::Mov => match instr.modifier {
                Modifier::I => {
                    // MOV.I copies the entire source instruction to the destination.
                    let src = self.core.get(a_eff);
                    self.core.set(b_eff, src);
                    self.warriors[warrior_idx].processes.push_back(next_pc);
                }
                _ => {
                    // Field-wise MOV variants not yet implemented; treat as no-op
                    // so the simulator stays usable. To be filled in incrementally.
                    self.warriors[warrior_idx].processes.push_back(next_pc);
                }
            },

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

            // Every other opcode is a no-op for now — keep the process alive
            // so the simulation continues. Real semantics get added per-opcode.
            _ => {
                self.warriors[warrior_idx].processes.push_back(next_pc);
            }
        }
    }
}

/// Resolve an operand to an effective core address relative to the executing PC.
///
/// Currently supports `Direct` and `Immediate` only. Indirect / pre-dec /
/// post-inc modes panic — they need to be implemented before any non-trivial
/// warrior (e.g. Dwarf, which uses `@`) can run.
fn resolve(pc: i32, op: Operand) -> i32 {
    match op.mode {
        AddressMode::Direct => pc + op.value,
        // Per ICWS '94, an immediate operand "points to" the cell containing
        // the executing instruction itself.
        AddressMode::Immediate => pc,
        other => panic!("addressing mode {:?} not yet implemented", other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruction::{AddressMode, Instruction, Modifier, Opcode, Operand};

    /// The canonical Imp: `MOV.I $0, $1`. Copies itself one cell forward
    /// every step, walking through core forever.
    fn imp() -> Instruction {
        Instruction {
            opcode: Opcode::Mov,
            modifier: Modifier::I,
            a: Operand {
                mode: AddressMode::Direct,
                value: 0,
            },
            b: Operand {
                mode: AddressMode::Direct,
                value: 1,
            },
        }
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
}
