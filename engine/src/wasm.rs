//! wasm-bindgen public API for the Core War engine.
//!
//! This module is only compiled on the `wasm32` target (`#[cfg(target_arch =
//! "wasm32")]` in `lib.rs`). It provides thin wrapper types that adapt the
//! native Rust API (`MatchState`, `ParsedWarrior`, etc.) for consumption from
//! JavaScript / TypeScript via wasm-pack.
//!
//! ## Design principles
//!
//! - **Thin wrappers, no duplicated logic.** Every method delegates to the
//!   inner Rust type. The wasm layer's job is type adaptation (Rust enums →
//!   integer discriminants, `Result<T, ParseError>` → `Result<T, JsValue>`,
//!   slices → `Vec<u8>` / `Vec<u32>` for typed-array transfer), not
//!   business logic.
//!
//! - **JS-friendly naming.** Rust methods are `snake_case`; the JS-facing
//!   exports use `camelCase` via `#[wasm_bindgen(js_name = "...")]`.
//!
//! - **Bulk reads for the visualizer.** Individual cell reads across the
//!   wasm boundary (8000 calls per frame at 60fps) would be too slow.
//!   `core_opcodes()` returns a `Uint8Array` of opcode bytes (one per cell)
//!   for the fast render path; `core_snapshot()` returns a `Uint32Array`
//!   with two words per cell (packed metadata + field values) for the
//!   detail/inspector view.
//!
//! ## JS usage example
//!
//! ```js
//! import init, { parseWarrior, MatchState } from 'core-war-engine';
//!
//! await init();
//!
//! const w1 = parseWarrior(source1);
//! const w2 = parseWarrior(source2);
//!
//! const match = new MatchState(8000, 80000);
//! match.loadWarrior(0, w1, 0);
//! match.loadWarrior(1, w2, 4000);
//!
//! // Game loop
//! function tick() {
//!     match.stepN(100);
//!     if (match.resultCode() !== 0) { /* handle end */ return; }
//!     const opcodes = match.coreOpcodes(); // Uint8Array(8000)
//!     renderGrid(opcodes);
//!     requestAnimationFrame(tick);
//! }
//! tick();
//! ```

use wasm_bindgen::prelude::*;

use crate::parser::ParsedWarrior;
use crate::vm::{MatchResult, MatchState};

// ─── Free functions ─────────────────────────────────────────────────

/// Returns the engine crate version string.
#[wasm_bindgen(js_name = "engineVersion")]
pub fn engine_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Parse a Redcode warrior from text source. Throws a JS error string on
/// parse failure (the error message includes the line number and a
/// description of the problem).
#[wasm_bindgen(js_name = "parseWarrior")]
pub fn parse_warrior(source: &str) -> Result<WasmParsedWarrior, JsValue> {
    crate::parse_warrior(source)
        .map(|pw| WasmParsedWarrior { inner: pw })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

// ─── ParsedWarrior wrapper ──────────────────────────────────────────

/// A warrior loaded from text source. Exposes metadata and instruction
/// count; the instructions themselves are not individually accessible
/// from JS — they're consumed in bulk via `MatchState.loadWarrior`.
#[wasm_bindgen(js_name = "ParsedWarrior")]
pub struct WasmParsedWarrior {
    inner: ParsedWarrior,
}

#[wasm_bindgen(js_class = "ParsedWarrior")]
impl WasmParsedWarrior {
    /// The warrior's name from a `;name` metadata comment, or `undefined`.
    pub fn name(&self) -> Option<String> {
        self.inner.name().map(|s| s.to_string())
    }

    /// The warrior's author from an `;author` metadata comment, or `undefined`.
    pub fn author(&self) -> Option<String> {
        self.inner.author().map(|s| s.to_string())
    }

    /// The offset within the warrior's instruction array where execution
    /// begins (set by `ORG` or `END` in the source).
    #[wasm_bindgen(js_name = "startOffset")]
    pub fn start_offset(&self) -> usize {
        self.inner.start_offset()
    }

    /// How many instructions this warrior contains.
    #[wasm_bindgen(js_name = "instructionCount")]
    pub fn instruction_count(&self) -> usize {
        self.inner.instructions().len()
    }
}

// ─── MatchState wrapper ─────────────────────────────────────────────

/// Full state of an in-progress battle, exposed to JavaScript.
///
/// Construct with `new MatchState(coreSize, maxSteps)`, load warriors with
/// `loadWarrior`, advance with `step` or `stepN`, read results with
/// `resultCode` / `resultWinnerId`, and read core state with `coreOpcodes`
/// or `coreSnapshot`.
#[wasm_bindgen(js_name = "MatchState")]
pub struct WasmMatchState {
    inner: MatchState,
}

#[wasm_bindgen(js_class = "MatchState")]
impl WasmMatchState {
    /// Create a new match with the given core size and step limit.
    #[wasm_bindgen(constructor)]
    pub fn new(core_size: usize, max_steps: u64) -> Self {
        Self {
            inner: MatchState::new(core_size, max_steps),
        }
    }

    /// Load a parsed warrior into core at `baseAddress` and register it
    /// in the match with the given `id`. The warrior's instructions are
    /// written sequentially starting at `baseAddress`, and its first
    /// process starts at `baseAddress + warrior.startOffset()`.
    #[wasm_bindgen(js_name = "loadWarrior")]
    pub fn load_warrior(
        &mut self,
        id: usize,
        warrior: &WasmParsedWarrior,
        base_address: usize,
    ) {
        self.inner.load_warrior(id, &warrior.inner, base_address);
    }

    /// Advance the simulation by one process-step. Returns `true` if the
    /// match is still in progress, `false` if it's over (all warriors dead
    /// or step limit reached).
    pub fn step(&mut self) -> bool {
        self.inner.step()
    }

    /// Advance the simulation by up to `n` process-steps. Returns the
    /// number of steps actually executed (may be less than `n` if the
    /// match ends before `n` is reached). Use this instead of calling
    /// `step()` in a JS loop to avoid N individual wasm boundary crossings.
    #[wasm_bindgen(js_name = "stepN")]
    pub fn step_n(&mut self, n: u32) -> u32 {
        let mut executed = 0u32;
        for _ in 0..n {
            if !self.inner.step() {
                break;
            }
            executed += 1;
        }
        executed
    }

    /// Number of process-steps that have been executed so far.
    pub fn steps(&self) -> u64 {
        self.inner.steps()
    }

    /// The configured step limit for this match.
    #[wasm_bindgen(js_name = "maxSteps")]
    pub fn max_steps(&self) -> u64 {
        self.inner.max_steps()
    }

    /// The size of the core memory array (number of cells).
    #[wasm_bindgen(js_name = "coreSize")]
    pub fn core_size(&self) -> usize {
        self.inner.core().size()
    }

    // ── Match result ────────────────────────────────────────────────

    /// The match result as an integer discriminant:
    ///   0 = Ongoing, 1 = Victory, 2 = Tie, 3 = AllDead.
    ///
    /// Use `resultWinnerId()` to get the winner when the code is 1.
    #[wasm_bindgen(js_name = "resultCode")]
    pub fn result_code(&self) -> u8 {
        match self.inner.result() {
            MatchResult::Ongoing => 0,
            MatchResult::Victory { .. } => 1,
            MatchResult::Tie => 2,
            MatchResult::AllDead => 3,
        }
    }

    /// The id of the winning warrior, or -1 if there is no winner.
    #[wasm_bindgen(js_name = "resultWinnerId")]
    pub fn result_winner_id(&self) -> i32 {
        match self.inner.result() {
            MatchResult::Victory { winner_id } => winner_id as i32,
            _ => -1,
        }
    }

    // ── Warrior queries ─────────────────────────────────────────────

    /// How many warriors are loaded in this match.
    #[wasm_bindgen(js_name = "warriorCount")]
    pub fn warrior_count(&self) -> usize {
        self.inner.warriors().len()
    }

    /// Whether the warrior at the given index is still alive.
    #[wasm_bindgen(js_name = "warriorIsAlive")]
    pub fn warrior_is_alive(&self, idx: usize) -> bool {
        self.inner
            .warriors()
            .get(idx)
            .map_or(false, |w| w.is_alive())
    }

    /// How many processes the warrior at the given index has.
    #[wasm_bindgen(js_name = "warriorProcessCount")]
    pub fn warrior_process_count(&self, idx: usize) -> usize {
        self.inner
            .warriors()
            .get(idx)
            .map_or(0, |w| w.process_count())
    }

    /// The id of the warrior at the given index.
    #[wasm_bindgen(js_name = "warriorId")]
    pub fn warrior_id(&self, idx: usize) -> usize {
        self.inner.warriors().get(idx).map_or(0, |w| w.id())
    }

    /// The PCs of all processes for the warrior at the given index, as a
    /// `Uint32Array`. Returns an empty array if the index is out of bounds
    /// or the warrior is dead.
    #[wasm_bindgen(js_name = "warriorProcessPcs")]
    pub fn warrior_process_pcs(&self, idx: usize) -> Vec<u32> {
        self.inner
            .warriors()
            .get(idx)
            .map(|w| w.process_pcs().map(|pc| pc as u32).collect())
            .unwrap_or_default()
    }

    // ── Single cell read (for tooltip / inspector) ──────────────────

    /// The opcode of the cell at the given address, as a `u8` discriminant.
    /// See the Opcode enum docs for the mapping (Dat=0, Mov=1, ..., Nop=15).
    #[wasm_bindgen(js_name = "cellOpcode")]
    pub fn cell_opcode(&self, addr: usize) -> u8 {
        self.inner.core().get(addr as i32).opcode as u8
    }

    /// The modifier of the cell at the given address, as a `u8` (A=0 .. I=6).
    #[wasm_bindgen(js_name = "cellModifier")]
    pub fn cell_modifier(&self, addr: usize) -> u8 {
        self.inner.core().get(addr as i32).modifier as u8
    }

    /// The A-operand addressing mode, as a `u8` (Immediate=0 .. BPostincrement=7).
    #[wasm_bindgen(js_name = "cellAMode")]
    pub fn cell_a_mode(&self, addr: usize) -> u8 {
        self.inner.core().get(addr as i32).a.mode as u8
    }

    /// The A-operand value (signed integer).
    #[wasm_bindgen(js_name = "cellAValue")]
    pub fn cell_a_value(&self, addr: usize) -> i32 {
        self.inner.core().get(addr as i32).a.value
    }

    /// The B-operand addressing mode, as a `u8`.
    #[wasm_bindgen(js_name = "cellBMode")]
    pub fn cell_b_mode(&self, addr: usize) -> u8 {
        self.inner.core().get(addr as i32).b.mode as u8
    }

    /// The B-operand value (signed integer).
    #[wasm_bindgen(js_name = "cellBValue")]
    pub fn cell_b_value(&self, addr: usize) -> i32 {
        self.inner.core().get(addr as i32).b.value
    }

    // ── Bulk core reads for the visualizer ──────────────────────────

    /// A `Uint8Array` of opcode bytes, one per cell, in address order.
    /// This is the fast path for coloring the PixiJS grid — one byte per
    /// cell, no decoding needed on the JS side.
    ///
    /// Opcode encoding: Dat=0, Mov=1, Add=2, Sub=3, Mul=4, Div=5, Mod=6,
    /// Jmp=7, Jmz=8, Jmn=9, Djn=10, Spl=11, Slt=12, Seq=13, Sne=14, Nop=15.
    #[wasm_bindgen(js_name = "coreOpcodes")]
    pub fn core_opcodes(&self) -> Vec<u8> {
        let core = self.inner.core();
        let size = core.size();
        (0..size).map(|i| core.get(i as i32).opcode as u8).collect()
    }

    /// Full core snapshot as a `Uint32Array` with **two words per cell**.
    /// This is the detail path for the inspector / tooltip view.
    ///
    /// Encoding per cell (2 consecutive `u32` entries):
    ///
    /// **Word 0** — packed metadata (13 bits used, 19 spare):
    ///   - bits 12..9: opcode (4 bits, 0–15)
    ///   - bits  8..6: modifier (3 bits, 0–6)
    ///   - bits  5..3: A addressing mode (3 bits, 0–7)
    ///   - bits  2..0: B addressing mode (3 bits, 0–7)
    ///
    /// **Word 1** — field values:
    ///   - bits 31..16: A-operand value as signed i16 (upper half)
    ///   - bits 15.. 0: B-operand value as signed i16 (lower half)
    ///
    /// JS decoding example:
    /// ```js
    /// const snap = match.coreSnapshot();
    /// for (let i = 0; i < coreSize; i++) {
    ///     const w0 = snap[i * 2], w1 = snap[i * 2 + 1];
    ///     const opcode = (w0 >> 9) & 0xF;
    ///     const aValue = w1 >> 16;            // sign-extends naturally
    ///     const bValue = (w1 << 16) >> 16;    // sign-extend lower half
    /// }
    /// ```
    ///
    /// Note: field values are stored as i16 (signed 16-bit), which
    /// accommodates core sizes up to 32767. Standard Core War uses 8000.
    #[wasm_bindgen(js_name = "coreSnapshot")]
    pub fn core_snapshot(&self) -> Vec<u32> {
        let core = self.inner.core();
        let size = core.size();
        let mut buf = Vec::with_capacity(size * 2);
        for i in 0..size {
            let instr = core.get(i as i32);
            // Word 0: packed opcode | modifier | a_mode | b_mode
            let w0 = (instr.opcode as u32) << 9
                | (instr.modifier as u32) << 6
                | (instr.a.mode as u32) << 3
                | instr.b.mode as u32;
            // Word 1: a_value(i16) in upper half, b_value(i16) in lower half
            let w1 = ((instr.a.value as i16 as u16) as u32) << 16
                | (instr.b.value as i16 as u16) as u32;
            buf.push(w0);
            buf.push(w1);
        }
        buf
    }
}
