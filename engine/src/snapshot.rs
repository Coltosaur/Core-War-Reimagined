//! Packing for `MatchState.coreSnapshot()` (see `wasm.rs` for the wire
//! format). Kept out of `wasm.rs`, which only compiles on wasm32, so native
//! tests can pin the encoding the frontend decodes.

use crate::vm::Core;
use crate::Instruction;

/// Two `u32` words per cell, in address order.
pub(crate) fn core_snapshot(core: &Core) -> Vec<u32> {
    (0..core.size())
        .flat_map(|i| pack_cell(&core.get(i as i32)))
        .collect()
}

/// Word 0: opcode | modifier | A mode | B mode. Word 1: A value (i16) in the
/// upper half, B value (i16) in the lower half.
fn pack_cell(instr: &Instruction) -> [u32; 2] {
    let w0 = (instr.opcode as u32) << 9
        | (instr.modifier as u32) << 6
        | (instr.a.mode as u32) << 3
        | instr.b.mode as u32;
    let w1 = ((instr.a.value as i16 as u16) as u32) << 16 | (instr.b.value as i16 as u16) as u32;
    [w0, w1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AddressMode, Modifier, Opcode, Operand};

    fn instr(opcode: Opcode, modifier: Modifier, a: Operand, b: Operand) -> Instruction {
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

    /// The decoding from the doc comment on `coreSnapshot` in wasm.rs, as
    /// JS runs it (`>>` on int32, so the upper half sign-extends).
    fn js_decode(w: [u32; 2]) -> (u32, u32, u32, u32, i32, i32) {
        let (w0, w1) = (w[0], w[1] as i32);
        (
            (w0 >> 9) & 0xF,
            (w0 >> 6) & 0x7,
            (w0 >> 3) & 0x7,
            w0 & 0x7,
            w1 >> 16,
            (w1 << 16) >> 16,
        )
    }

    #[test]
    fn packs_dat_zero_as_two_zero_words() {
        let dat = instr(
            Opcode::Dat,
            Modifier::A,
            op(AddressMode::Immediate, 0),
            op(AddressMode::Immediate, 0),
        );
        assert_eq!(pack_cell(&dat), [0, 0]);
    }

    #[test]
    fn packs_every_field_at_its_bit_position() {
        // Highest discriminant of each field, so an off-by-one shift or a
        // too-narrow mask shows up.
        let i = instr(
            Opcode::Nop,
            Modifier::I,
            op(AddressMode::BPostincrement, 32767),
            op(AddressMode::BPostincrement, 1),
        );
        assert_eq!(pack_cell(&i), [15 << 9 | 6 << 6 | 7 << 3 | 7, 0x7FFF_0001]);
    }

    #[test]
    fn js_decoding_recovers_signed_values_and_modes() {
        let i = instr(
            Opcode::Djn,
            Modifier::BA,
            op(AddressMode::APredecrement, -1),
            op(AddressMode::BIndirect, -32768),
        );
        assert_eq!(
            js_decode(pack_cell(&i)),
            (
                Opcode::Djn as u32,
                Modifier::BA as u32,
                AddressMode::APredecrement as u32,
                AddressMode::BIndirect as u32,
                -1,
                -32768,
            )
        );
    }

    #[test]
    fn snapshot_has_two_words_per_cell_in_address_order() {
        let mut core = Core::new(3);
        let mov = instr(
            Opcode::Mov,
            Modifier::I,
            op(AddressMode::Direct, 0),
            op(AddressMode::Direct, 1),
        );
        core.set(1, mov);
        let snap = core_snapshot(&core);
        assert_eq!(snap.len(), 6);
        assert_eq!(snap[2..4], pack_cell(&mov));
        assert_eq!(snap[..2], pack_cell(&core.get(0)));
        assert_eq!(snap[4..], pack_cell(&core.get(2)));
    }
}
