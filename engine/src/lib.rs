//! Core War engine — MARS (Memory Array Redcode Simulator).
//!
//! This crate is the simulation kernel: parses Redcode warriors, executes
//! battles in a circular memory array, and emits state for visualization.
//!
//! It is designed to compile to two targets from a single source:
//!   - **wasm32-unknown-unknown** via `wasm-pack` — the `wasm` module
//!     (conditionally compiled only on this target) provides
//!     `#[wasm_bindgen]` wrappers consumed by the frontend visualizer.
//!   - **native** — consumed directly as a Rust crate dependency by the
//!     `core-war-backend` workspace for server-side ranked-match validation.
//!
//! The native public API is re-exported at the crate root below. The wasm
//! public API lives in `wasm.rs` and is only compiled when
//! `target_arch = "wasm32"`.

pub mod instruction;
pub mod parser;
pub mod vm;

#[cfg(target_arch = "wasm32")]
pub mod wasm;

pub use instruction::{AddressMode, Instruction, Modifier, Opcode, Operand};
pub use parser::{parse_warrior, ParseError, ParsedWarrior};
pub use vm::{Core, MatchResult, MatchState, Warrior};
