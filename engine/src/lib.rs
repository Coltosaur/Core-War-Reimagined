//! Core War engine — MARS (Memory Array Redcode Simulator).
//!
//! This crate is the simulation kernel: parses Redcode warriors, executes
//! battles in a circular memory array, and emits state for visualization.
//!
//! It is designed to compile to two targets from a single source:
//!   - **wasm32-unknown-unknown** via `wasm-pack` — consumed by the frontend
//!     visualizer running in the browser.
//!   - **native** — consumed directly as a Rust crate dependency by the
//!     `core-war-backend` workspace for server-side ranked-match validation.
//!
//! Implementation status: the type model is complete; the executor currently
//! supports only the opcodes/addressing modes needed to run an Imp warrior.
//! New opcodes are intended to be added one at a time, each with a unit test.

pub mod instruction;
pub mod vm;

pub use instruction::{AddressMode, Instruction, Modifier, Opcode, Operand};
pub use vm::{Core, MatchState, Warrior};

use wasm_bindgen::prelude::*;

/// Stub wasm-bindgen export so `wasm-pack build` continues to produce a
/// usable JS module while the real public API is still being designed.
/// Returns the engine crate version string.
#[wasm_bindgen]
pub fn engine_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
