//! Builders and assertions shared by the unit-test modules in `vm.rs` and
//! `parser.rs`.

use crate::instruction::{AddressMode, Instruction, Modifier, Opcode, Operand};
use crate::vm::Core;

/// Convenience for building an `Instruction` in a test without rendering
/// the full struct literal six lines tall every time.
pub fn instr(opcode: Opcode, modifier: Modifier, a: Operand, b: Operand) -> Instruction {
    Instruction {
        opcode,
        modifier,
        a,
        b,
    }
}

fn op(mode: AddressMode, value: i32) -> Operand {
    Operand { mode, value }
}

pub fn imm(v: i32) -> Operand {
    op(AddressMode::Immediate, v)
}

pub fn dir(v: i32) -> Operand {
    op(AddressMode::Direct, v)
}

pub fn b_ind(v: i32) -> Operand {
    op(AddressMode::BIndirect, v)
}

pub fn b_predec(v: i32) -> Operand {
    op(AddressMode::BPredecrement, v)
}

pub fn a_predec(v: i32) -> Operand {
    op(AddressMode::APredecrement, v)
}

pub fn a_postinc(v: i32) -> Operand {
    op(AddressMode::APostincrement, v)
}

pub fn b_postinc(v: i32) -> Operand {
    op(AddressMode::BPostincrement, v)
}

/// The canonical Dwarf, hand-built:
///   0: ADD.AB #4, $3
///   1: MOV.I  $2, @2
///   2: JMP.B  $-2, $0
///   3: DAT.F  #0, #0   (the bomb / bomb pointer)
pub fn dwarf() -> [Instruction; 4] {
    [
        instr(Opcode::Add, Modifier::AB, imm(4), dir(3)),
        instr(Opcode::Mov, Modifier::I, dir(2), b_ind(2)),
        instr(Opcode::Jmp, Modifier::B, dir(-2), dir(0)),
        Instruction::dat_zero(),
    ]
}

/// After 15 steps (5 loop iterations) of a Dwarf loaded at cell 0: the bomb
/// pointer (cell 3's B-field) is 20, and cells 7, 11, 15, 19, 23 hold DAT
/// bombs with B-values 4, 8, 12, 16, 20 — each a snapshot of cell 3 at the
/// time it was thrown.
pub fn assert_dwarf_bomb_pattern(core: &Core) {
    assert_eq!(core.get(3).b.value, 20);
    for (addr, expected_b) in [(7, 4), (11, 8), (15, 12), (19, 16), (23, 20)] {
        let cell = core.get(addr);
        assert_eq!(cell.opcode, Opcode::Dat, "cell {addr} should be a DAT bomb");
        assert_eq!(
            cell.b.value, expected_b,
            "cell {addr}'s b-field should be {expected_b}",
        );
    }
}
