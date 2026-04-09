//! Redcode instruction model.
//!
//! Follows the ICWS '94 draft (the "Core War '94" standard) — the most
//! widely implemented variant. Each cell of MARS memory holds one
//! `Instruction`, which is `Opcode` + `Modifier` + two `Operand`s.

/// Every Redcode opcode. Not all of these are executed yet — see `vm::execute`
/// for the currently-implemented subset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opcode {
    /// Data — kills the executing process when reached.
    Dat,
    /// Move — copy A to B.
    Mov,
    /// Add — B = B + A.
    Add,
    /// Subtract — B = B - A.
    Sub,
    /// Multiply — B = B * A.
    Mul,
    /// Divide — B = B / A. Divide-by-zero kills the process.
    Div,
    /// Modulo — B = B mod A. Mod-by-zero kills the process.
    Mod,
    /// Unconditional jump to A.
    Jmp,
    /// Jump to A if B is zero.
    Jmz,
    /// Jump to A if B is non-zero.
    Jmn,
    /// Decrement B, then jump to A if B is non-zero.
    Djn,
    /// Split — continue at next instruction *and* spawn a new process at A.
    Spl,
    /// Skip next instruction if A is less than B.
    Slt,
    /// Compare (== Seq in '94) — skip next if A == B.
    Cmp,
    /// Skip next instruction if A != B.
    Sne,
    /// No-op.
    Nop,
}

/// Instruction modifier — controls *which* fields the opcode operates on.
/// `.I` ("instruction") is the most common for `Mov` and means "the whole cell".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modifier {
    A,
    B,
    AB,
    BA,
    F,
    X,
    I,
}

/// How an operand's value is interpreted to find an effective core address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressMode {
    /// `#N` — the literal value N. Operand "points at" the executing instruction.
    Immediate,
    /// `$N` (the default mode) — the cell at PC + N.
    Direct,
    /// `*N` — A-indirect: read PC+N, then offset by *that cell's A-field*.
    AIndirect,
    /// `@N` — B-indirect: read PC+N, then offset by *that cell's B-field*.
    BIndirect,
    /// `{N` — predecrement A-indirect.
    APredecrement,
    /// `}N` — postincrement A-indirect.
    APostincrement,
    /// `<N` — predecrement B-indirect.
    BPredecrement,
    /// `>N` — postincrement B-indirect.
    BPostincrement,
}

/// One operand: an addressing mode plus a signed offset value.
///
/// Values are stored signed (`i32`) so we can represent the negative offsets
/// programmers actually write in source code (e.g. `JMP -2`). They are reduced
/// modulo core size at resolution time, not at construction time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Operand {
    pub mode: AddressMode,
    pub value: i32,
}

/// One cell of MARS memory. Every cell — even "empty" ones — is a valid
/// instruction. Empty cells are conventionally `DAT.F #0, #0`, which kills
/// any process unfortunate enough to execute one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Instruction {
    pub opcode: Opcode,
    pub modifier: Modifier,
    pub a: Operand,
    pub b: Operand,
}

impl Instruction {
    /// `DAT.F #0, #0` — the canonical "empty cell" / dead instruction.
    pub const fn dat_zero() -> Self {
        Self {
            opcode: Opcode::Dat,
            modifier: Modifier::F,
            a: Operand {
                mode: AddressMode::Immediate,
                value: 0,
            },
            b: Operand {
                mode: AddressMode::Immediate,
                value: 0,
            },
        }
    }
}
