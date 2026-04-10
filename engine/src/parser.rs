//! Redcode parser — converts text source into a `ParsedWarrior`.
//!
//! Implements enough of ICWS '94 to load the canonical warriors:
//!   - All 16 opcodes (case-insensitive; `CMP` is accepted as the ICWS '88
//!     alias for `SEQ`)
//!   - All 7 modifiers (case-insensitive). Modifiers can be omitted —
//!     `default_modifier` infers them per the ICWS '94 rules.
//!   - All 8 addressing modes (`# $ * @ { } < >`); the `$` prefix can be
//!     omitted, and a bare number or label means direct addressing.
//!   - Labels with both forward and backward references via two-pass parsing
//!   - Numeric operand values (signed integers)
//!   - Comments (`;` to end of line)
//!   - Metadata comments: `;name <name>` and `;author <author>`
//!   - Pseudo-ops: `ORG <label>` (sets start offset), `END [<label>]`
//!     (terminates source, optionally with a start label)
//!   - Default operand handling: single-operand `DAT`/`NOP` becomes
//!     `(#0, #operand)`; single-operand jumps become `(operand, $0)`.
//!
//! Not yet supported (deferred until a real warrior needs it):
//!   - `EQU` constants
//!   - Arithmetic expressions in operand values (`label + 1` etc.)
//!   - Multiple warriors per source file (`FOR` loops)

use std::collections::HashMap;

use crate::instruction::{AddressMode, Instruction, Modifier, Opcode, Operand};

/// A warrior loaded from text source — instructions, the offset within
/// `instructions` where execution begins, and any metadata extracted from
/// `;name` / `;author` comments.
///
/// Construct via `parse_warrior`. Load into a battle via
/// `MatchState::load_warrior` (defined in `vm.rs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedWarrior {
    instructions: Vec<Instruction>,
    start_offset: usize,
    name: Option<String>,
    author: Option<String>,
}

impl ParsedWarrior {
    pub fn instructions(&self) -> &[Instruction] {
        &self.instructions
    }

    pub fn start_offset(&self) -> usize {
        self.start_offset
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn author(&self) -> Option<&str> {
        self.author.as_deref()
    }
}

/// Errors raised while parsing Redcode source. Every variant carries the
/// 1-indexed line number where the problem was found so error messages
/// can point at the right place in the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// The source contained no instructions.
    EmptyWarrior,
    /// An opcode token wasn't recognized.
    UnknownOpcode { line: usize, text: String },
    /// A modifier token wasn't recognized.
    UnknownModifier { line: usize, text: String },
    /// An operand value referenced a label that wasn't defined.
    UnknownLabel { line: usize, label: String },
    /// A numeric operand value couldn't be parsed.
    InvalidNumber { line: usize, text: String },
    /// The same label was defined twice.
    DuplicateLabel { line: usize, label: String },
    /// A line had structural problems (missing operand, malformed pseudo-op).
    SyntaxError { line: usize, message: String },
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::EmptyWarrior => write!(f, "warrior source has no instructions"),
            ParseError::UnknownOpcode { line, text } => {
                write!(f, "line {line}: unknown opcode {text:?}")
            }
            ParseError::UnknownModifier { line, text } => {
                write!(f, "line {line}: unknown modifier {text:?}")
            }
            ParseError::UnknownLabel { line, label } => {
                write!(f, "line {line}: undefined label {label:?}")
            }
            ParseError::InvalidNumber { line, text } => {
                write!(f, "line {line}: invalid number {text:?}")
            }
            ParseError::DuplicateLabel { line, label } => {
                write!(f, "line {line}: duplicate label {label:?}")
            }
            ParseError::SyntaxError { line, message } => {
                write!(f, "line {line}: syntax error: {message}")
            }
        }
    }
}

impl std::error::Error for ParseError {}

/// Parse a Redcode warrior from text source.
pub fn parse_warrior(source: &str) -> Result<ParsedWarrior, ParseError> {
    // ─── Pass 1: classify lines, extract metadata, find labels ───
    let mut name: Option<String> = None;
    let mut author: Option<String> = None;
    let mut start_label: Option<String> = None;
    let mut equ_table: HashMap<String, String> = HashMap::new();
    let mut instr_lines: Vec<InstructionLine> = Vec::new();

    'lines: for (idx, raw) in source.lines().enumerate() {
        let line_no = idx + 1; // human-friendly 1-indexed line numbers
        let code = strip_comment_extracting_metadata(raw, &mut name, &mut author);
        let code = code.trim();
        if code.is_empty() {
            continue;
        }

        // Pseudo-ops are detected by their leading keyword (case-insensitive,
        // followed by whitespace or end-of-line).
        if let Some(rest) = strip_keyword_ci(code, "ORG") {
            let target = rest.trim();
            if target.is_empty() {
                return Err(ParseError::SyntaxError {
                    line: line_no,
                    message: "ORG requires a target label".to_string(),
                });
            }
            start_label = Some(target.to_string());
            continue;
        }
        if let Some(rest) = strip_keyword_ci(code, "END") {
            let target = rest.trim();
            if !target.is_empty() {
                start_label = Some(target.to_string());
            }
            break 'lines;
        }

        // EQU pseudo-op: "name EQU value". Must be checked before
        // parse_label_and_body because EQU lines are not instructions —
        // they define text substitutions consumed during operand parsing.
        {
            let first_end = code
                .char_indices()
                .find(|(_, c)| c.is_whitespace())
                .map(|(i, _)| i)
                .unwrap_or(code.len());
            let after_first = code[first_end..].trim_start();
            if let Some(rest) = strip_keyword_ci(after_first, "EQU") {
                let equ_name = code[..first_end].trim_end_matches(':').to_string();
                let equ_value = rest.trim().to_string();
                if equ_value.is_empty() {
                    return Err(ParseError::SyntaxError {
                        line: line_no,
                        message: "EQU requires a value".to_string(),
                    });
                }
                if equ_table.insert(equ_name.clone(), equ_value).is_some() {
                    return Err(ParseError::DuplicateLabel {
                        line: line_no,
                        label: equ_name,
                    });
                }
                continue 'lines;
            }
        }

        // Otherwise, it's a (possibly labeled) instruction line.
        instr_lines.push(parse_label_and_body(code, line_no)?);
    }

    if instr_lines.is_empty() {
        return Err(ParseError::EmptyWarrior);
    }

    // Build the label → offset table. Done in its own pass so forward
    // references work — an instruction at offset 0 can refer to a label
    // at offset 5.
    let mut label_table: HashMap<String, usize> = HashMap::new();
    for (offset, il) in instr_lines.iter().enumerate() {
        if let Some(label) = &il.label {
            if label_table.insert(label.clone(), offset).is_some() {
                return Err(ParseError::DuplicateLabel {
                    line: il.line_no,
                    label: label.clone(),
                });
            }
        }
    }

    // ─── Pass 2: parse instruction bodies, resolving labels ───
    let mut instructions = Vec::with_capacity(instr_lines.len());
    for (offset, il) in instr_lines.iter().enumerate() {
        let instr = parse_instruction_body(&il.body, offset, il.line_no, &label_table, &equ_table)?;
        instructions.push(instr);
    }

    // Resolve start_offset from the (optional) ORG / END label.
    let start_offset = match start_label {
        Some(label) => *label_table
            .get(&label)
            .ok_or_else(|| ParseError::UnknownLabel { line: 0, label })?,
        None => 0,
    };

    Ok(ParsedWarrior {
        instructions,
        start_offset,
        name,
        author,
    })
}

#[derive(Debug, Clone)]
struct InstructionLine {
    label: Option<String>,
    body: String,
    line_no: usize,
}

/// Strip the trailing `;`-comment from `line`, populating `name` / `author`
/// from `;name <value>` and `;author <value>` metadata comments.
fn strip_comment_extracting_metadata(
    line: &str,
    name: &mut Option<String>,
    author: &mut Option<String>,
) -> String {
    let Some(idx) = line.find(';') else {
        return line.to_string();
    };

    let comment = line[idx + 1..].trim();
    if let Some(rest) = strip_keyword_ci(comment, "name") {
        *name = Some(rest.trim().to_string());
    } else if let Some(rest) = strip_keyword_ci(comment, "author") {
        *author = Some(rest.trim().to_string());
    }

    line[..idx].to_string()
}

/// If `text` starts with `keyword` (case-insensitive) followed by whitespace
/// or end-of-string, returns the remainder. Otherwise returns `None`.
fn strip_keyword_ci<'a>(text: &'a str, keyword: &str) -> Option<&'a str> {
    if text.len() < keyword.len() {
        return None;
    }
    let (head, rest) = text.split_at(keyword.len());
    if !head.eq_ignore_ascii_case(keyword) {
        return None;
    }
    if rest.is_empty() || rest.starts_with(char::is_whitespace) {
        Some(rest)
    } else {
        None
    }
}

/// Pull the optional leading label off an instruction line. The first
/// whitespace-separated token is a label if it does NOT parse as a known
/// opcode (with optional `.modifier`); otherwise the line has no label.
fn parse_label_and_body(code: &str, line_no: usize) -> Result<InstructionLine, ParseError> {
    let first_end = code
        .char_indices()
        .find(|(_, c)| c.is_whitespace())
        .map(|(i, _)| i)
        .unwrap_or(code.len());

    let first_token = &code[..first_end];
    let rest = code[first_end..].trim_start();

    // The opcode-or-label decision: try the first token (minus any modifier)
    // as a known opcode. If it parses, there's no label.
    let first_no_modifier = first_token.split('.').next().unwrap_or(first_token);
    if parse_opcode_name(first_no_modifier).is_some() {
        Ok(InstructionLine {
            label: None,
            body: code.to_string(),
            line_no,
        })
    } else {
        let label = first_token.trim_end_matches(':').to_string();
        if rest.is_empty() {
            return Err(ParseError::SyntaxError {
                line: line_no,
                message: format!("label {label:?} has no instruction"),
            });
        }
        Ok(InstructionLine {
            label: Some(label),
            body: rest.to_string(),
            line_no,
        })
    }
}

fn parse_opcode_name(s: &str) -> Option<Opcode> {
    Some(match s.to_ascii_uppercase().as_str() {
        "DAT" => Opcode::Dat,
        "MOV" => Opcode::Mov,
        "ADD" => Opcode::Add,
        "SUB" => Opcode::Sub,
        "MUL" => Opcode::Mul,
        "DIV" => Opcode::Div,
        "MOD" => Opcode::Mod,
        "JMP" => Opcode::Jmp,
        "JMZ" => Opcode::Jmz,
        "JMN" => Opcode::Jmn,
        "DJN" => Opcode::Djn,
        "SPL" => Opcode::Spl,
        // SEQ is the ICWS '94 name; CMP is the older ICWS '88 alias.
        "SEQ" | "CMP" => Opcode::Seq,
        "SNE" => Opcode::Sne,
        "SLT" => Opcode::Slt,
        "NOP" => Opcode::Nop,
        _ => return None,
    })
}

fn parse_modifier_name(s: &str) -> Option<Modifier> {
    Some(match s.to_ascii_uppercase().as_str() {
        "A" => Modifier::A,
        "B" => Modifier::B,
        "AB" => Modifier::AB,
        "BA" => Modifier::BA,
        "F" => Modifier::F,
        "X" => Modifier::X,
        "I" => Modifier::I,
        _ => return None,
    })
}

fn parse_instruction_body(
    body: &str,
    offset: usize,
    line_no: usize,
    labels: &HashMap<String, usize>,
    equ_table: &HashMap<String, String>,
) -> Result<Instruction, ParseError> {
    // Split into the opcode-token (with optional .modifier) and the rest.
    let mut parts = body.splitn(2, char::is_whitespace);
    let opcode_token = parts.next().unwrap_or("").trim();
    let operands_text = parts.next().unwrap_or("").trim();

    let (opcode, explicit_modifier) = parse_opcode_token(opcode_token, line_no)?;

    // Operands are comma-separated. Empty entries (from `DAT` with no
    // operands at all) are filtered out so the slice patterns below are clean.
    let operand_parts: Vec<&str> = operands_text
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();

    // Operand defaults differ by opcode:
    //   - DAT and NOP can take 0 or 1 operands; missing operands default to #0
    //     and a single operand goes in the B field (per ICWS '94 single-DAT).
    //   - Jump-style opcodes (JMP, JMZ, JMN, DJN, SPL) often take just an A;
    //     the missing B defaults to $0.
    //   - Everything else needs both operands; one is treated as A and B
    //     defaults to $0.
    let (a, b) = match (opcode, operand_parts.as_slice()) {
        (Opcode::Dat | Opcode::Nop, []) => (
            Operand {
                mode: AddressMode::Immediate,
                value: 0,
            },
            Operand {
                mode: AddressMode::Immediate,
                value: 0,
            },
        ),
        (Opcode::Dat | Opcode::Nop, [single]) => {
            let b = parse_operand(single, offset, line_no, labels, equ_table)?;
            let a = Operand {
                mode: AddressMode::Immediate,
                value: 0,
            };
            (a, b)
        }
        (_, []) => {
            return Err(ParseError::SyntaxError {
                line: line_no,
                message: format!("opcode {opcode:?} requires at least one operand"),
            });
        }
        (_, [a_str]) => {
            let a = parse_operand(a_str, offset, line_no, labels, equ_table)?;
            let b = Operand {
                mode: AddressMode::Direct,
                value: 0,
            };
            (a, b)
        }
        (_, [a_str, b_str]) => {
            let a = parse_operand(a_str, offset, line_no, labels, equ_table)?;
            let b = parse_operand(b_str, offset, line_no, labels, equ_table)?;
            (a, b)
        }
        _ => {
            return Err(ParseError::SyntaxError {
                line: line_no,
                message: "instruction has more than two operands".to_string(),
            });
        }
    };

    let modifier = explicit_modifier.unwrap_or_else(|| default_modifier(opcode, a.mode, b.mode));

    Ok(Instruction {
        opcode,
        modifier,
        a,
        b,
    })
}

fn parse_opcode_token(token: &str, line_no: usize) -> Result<(Opcode, Option<Modifier>), ParseError> {
    let mut parts = token.splitn(2, '.');
    let opcode_str = parts.next().unwrap_or("");
    let modifier_str = parts.next();

    let opcode = parse_opcode_name(opcode_str).ok_or_else(|| ParseError::UnknownOpcode {
        line: line_no,
        text: opcode_str.to_string(),
    })?;

    let modifier = if let Some(m_str) = modifier_str {
        Some(
            parse_modifier_name(m_str).ok_or_else(|| ParseError::UnknownModifier {
                line: line_no,
                text: m_str.to_string(),
            })?,
        )
    } else {
        None
    };

    Ok((opcode, modifier))
}

fn parse_operand(
    text: &str,
    offset: usize,
    line_no: usize,
    labels: &HashMap<String, usize>,
    equ_table: &HashMap<String, String>,
) -> Result<Operand, ParseError> {
    let text = text.trim();
    let (mode, value_text) = match text.chars().next() {
        Some('#') => (AddressMode::Immediate, &text[1..]),
        Some('$') => (AddressMode::Direct, &text[1..]),
        Some('*') => (AddressMode::AIndirect, &text[1..]),
        Some('@') => (AddressMode::BIndirect, &text[1..]),
        Some('{') => (AddressMode::APredecrement, &text[1..]),
        Some('}') => (AddressMode::APostincrement, &text[1..]),
        Some('<') => (AddressMode::BPredecrement, &text[1..]),
        Some('>') => (AddressMode::BPostincrement, &text[1..]),
        // Bare number or label — direct addressing is the default.
        _ => (AddressMode::Direct, text),
    };

    let value = parse_value(value_text.trim(), offset, line_no, labels, equ_table)?;
    Ok(Operand { mode, value })
}

/// Parse an operand value: a signed integer literal, an EQU constant,
/// or a label resolved to its offset relative to the executing instruction.
fn parse_value(
    text: &str,
    offset: usize,
    line_no: usize,
    labels: &HashMap<String, usize>,
    equ_table: &HashMap<String, String>,
) -> Result<i32, ParseError> {
    if text.is_empty() {
        return Err(ParseError::SyntaxError {
            line: line_no,
            message: "operand has no value".to_string(),
        });
    }

    // Try as a numeric literal first.
    if let Ok(n) = text.parse::<i32>() {
        return Ok(n);
    }

    // Try as an EQU constant (text substitution, then re-parse).
    if let Some(replacement) = equ_table.get(text) {
        return parse_value(replacement, offset, line_no, labels, equ_table);
    }

    // Try as a label reference (resolved to relative offset).
    if let Some(&label_offset) = labels.get(text) {
        return Ok(label_offset as i32 - offset as i32);
    }

    // Last possibility: maybe it looked like a number but failed to parse
    // (e.g. overflow, leading zeros, malformed). Distinguish that from a
    // genuinely unknown label by whether it starts with a digit or sign.
    let starts_numeric = text
        .chars()
        .next()
        .map(|c| c == '-' || c == '+' || c.is_ascii_digit())
        .unwrap_or(false);
    if starts_numeric {
        Err(ParseError::InvalidNumber {
            line: line_no,
            text: text.to_string(),
        })
    } else {
        Err(ParseError::UnknownLabel {
            line: line_no,
            label: text.to_string(),
        })
    }
}

/// Default-modifier inference per ICWS '94 §A.2.1.
///
///   DAT, NOP                          → .F always
///   MOV, SEQ, SNE                     → .AB if A immediate
///                                      .B  if B immediate
///                                      .I  otherwise
///   ADD, SUB, MUL, DIV, MOD           → .AB if A immediate
///                                      .B  if B immediate
///                                      .F  otherwise
///   SLT                                → .AB if A immediate
///                                      .B  otherwise
///   JMP, JMZ, JMN, DJN, SPL            → .B always
fn default_modifier(opcode: Opcode, a_mode: AddressMode, b_mode: AddressMode) -> Modifier {
    use AddressMode::Immediate;
    use Opcode::*;

    let a_imm = a_mode == Immediate;
    let b_imm = b_mode == Immediate;

    match opcode {
        Dat | Nop => Modifier::F,
        Mov | Seq | Sne => {
            if a_imm {
                Modifier::AB
            } else if b_imm {
                Modifier::B
            } else {
                Modifier::I
            }
        }
        Add | Sub | Mul | Div | Mod => {
            if a_imm {
                Modifier::AB
            } else if b_imm {
                Modifier::B
            } else {
                Modifier::F
            }
        }
        Slt => {
            if a_imm {
                Modifier::AB
            } else {
                Modifier::B
            }
        }
        Jmp | Jmz | Jmn | Djn | Spl => Modifier::B,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::{MatchResult, MatchState};

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

    // ── basic single-instruction parsing ────────────────────────────

    #[test]
    fn parses_explicit_modifier_and_explicit_modes() {
        let parsed = parse_warrior("MOV.I $0, $1").unwrap();
        assert_eq!(parsed.instructions().len(), 1);
        assert_eq!(
            parsed.instructions()[0],
            Instruction {
                opcode: Opcode::Mov,
                modifier: Modifier::I,
                a: dir(0),
                b: dir(1),
            }
        );
    }

    #[test]
    fn parses_lowercase_opcode_and_modifier() {
        // Opcodes and modifiers are case-insensitive; labels are not.
        let parsed = parse_warrior("mov.i $0, $1").unwrap();
        assert_eq!(parsed.instructions()[0].opcode, Opcode::Mov);
        assert_eq!(parsed.instructions()[0].modifier, Modifier::I);
    }

    #[test]
    fn parses_cmp_as_seq_alias() {
        // CMP is the ICWS '88 name; the parser accepts it but maps to Seq.
        let parsed = parse_warrior("CMP.I $0, $1").unwrap();
        assert_eq!(parsed.instructions()[0].opcode, Opcode::Seq);
    }

    #[test]
    fn parses_bare_number_as_direct() {
        // Direct addressing is the default — no `$` prefix required.
        let parsed = parse_warrior("MOV.I 5, 7").unwrap();
        assert_eq!(parsed.instructions()[0].a, dir(5));
        assert_eq!(parsed.instructions()[0].b, dir(7));
    }

    #[test]
    fn parses_negative_operand_value() {
        let parsed = parse_warrior("JMP.B $-3, $0").unwrap();
        assert_eq!(parsed.instructions()[0].a.value, -3);
    }

    #[test]
    fn parses_each_addressing_mode() {
        // One MOV.A per addressing mode, all in one source.
        let source = "
            MOV.A #1, #2
            MOV.A $1, $2
            MOV.A *1, *2
            MOV.A @1, @2
            MOV.A {1, {2
            MOV.A }1, }2
            MOV.A <1, <2
            MOV.A >1, >2
        ";
        let parsed = parse_warrior(source).unwrap();
        let modes = [
            AddressMode::Immediate,
            AddressMode::Direct,
            AddressMode::AIndirect,
            AddressMode::BIndirect,
            AddressMode::APredecrement,
            AddressMode::APostincrement,
            AddressMode::BPredecrement,
            AddressMode::BPostincrement,
        ];
        for (i, mode) in modes.iter().enumerate() {
            assert_eq!(parsed.instructions()[i].a.mode, *mode, "instr {i} A mode");
            assert_eq!(parsed.instructions()[i].b.mode, *mode, "instr {i} B mode");
        }
    }

    // ── default modifier inference ──────────────────────────────────

    #[test]
    fn default_modifier_for_dat_and_nop_is_f() {
        let parsed = parse_warrior("DAT #0, #0").unwrap();
        assert_eq!(parsed.instructions()[0].modifier, Modifier::F);
        let parsed = parse_warrior("NOP $0, $0").unwrap();
        assert_eq!(parsed.instructions()[0].modifier, Modifier::F);
    }

    #[test]
    fn default_modifier_for_arithmetic_with_immediate_a_is_ab() {
        let parsed = parse_warrior("ADD #4, $3").unwrap();
        assert_eq!(parsed.instructions()[0].modifier, Modifier::AB);
    }

    #[test]
    fn default_modifier_for_arithmetic_with_immediate_b_is_b() {
        let parsed = parse_warrior("ADD $4, #3").unwrap();
        assert_eq!(parsed.instructions()[0].modifier, Modifier::B);
    }

    #[test]
    fn default_modifier_for_arithmetic_with_no_immediate_is_f() {
        let parsed = parse_warrior("ADD $4, $3").unwrap();
        assert_eq!(parsed.instructions()[0].modifier, Modifier::F);
    }

    #[test]
    fn default_modifier_for_mov_with_no_immediate_is_i() {
        // MOV's no-immediate default is .I (whole-instruction copy), unlike
        // arithmetic's .F (field-wise).
        let parsed = parse_warrior("MOV $0, $1").unwrap();
        assert_eq!(parsed.instructions()[0].modifier, Modifier::I);
    }

    #[test]
    fn default_modifier_for_jumps_is_b() {
        for op in ["JMP", "JMZ", "JMN", "DJN", "SPL"] {
            let parsed = parse_warrior(&format!("{op} $0, $1")).unwrap();
            assert_eq!(
                parsed.instructions()[0].modifier,
                Modifier::B,
                "{op} should default to .B",
            );
        }
    }

    // ── single-operand opcode handling ──────────────────────────────

    #[test]
    fn jmp_with_single_operand_defaults_b_to_zero() {
        let parsed = parse_warrior("JMP $-2").unwrap();
        assert_eq!(parsed.instructions()[0].a, dir(-2));
        assert_eq!(parsed.instructions()[0].b, dir(0));
    }

    #[test]
    fn dat_with_single_operand_treats_it_as_b_field() {
        // ICWS '94: DAT #5 ≡ DAT.F #0, #5
        let parsed = parse_warrior("DAT #5").unwrap();
        assert_eq!(parsed.instructions()[0].a, imm(0));
        assert_eq!(parsed.instructions()[0].b, imm(5));
    }

    #[test]
    fn dat_with_no_operands_is_dat_zero() {
        let parsed = parse_warrior("DAT").unwrap();
        assert_eq!(parsed.instructions()[0], Instruction::dat_zero());
    }

    // ── labels ──────────────────────────────────────────────────────

    #[test]
    fn parses_backward_label_to_relative_offset() {
        // `bomb` is at offset 0; the JMP at offset 1 references it as
        // a backward direct address. Expected operand value: 0 - 1 = -1.
        let source = "
bomb    DAT.F #0, #0
        JMP   bomb
        ";
        let parsed = parse_warrior(source).unwrap();
        assert_eq!(parsed.instructions()[1].a.value, -1);
    }

    #[test]
    fn parses_forward_label_to_relative_offset() {
        // `bomb` is at offset 1; the JMP at offset 0 references it forward.
        // Expected operand value: 1 - 0 = +1.
        let source = "
        JMP   bomb
bomb    DAT.F #0, #0
        ";
        let parsed = parse_warrior(source).unwrap();
        assert_eq!(parsed.instructions()[0].a.value, 1);
    }

    #[test]
    fn parses_label_used_with_addressing_mode_prefix() {
        // `@bomb` should resolve `bomb` to a relative offset and pair it
        // with B-indirect addressing.
        let source = "
start   MOV.I bomb, @bomb
bomb    DAT.F #0, #0
        ";
        let parsed = parse_warrior(source).unwrap();
        // bomb is at offset 1; MOV is at offset 0. Relative = +1.
        assert_eq!(parsed.instructions()[0].a, dir(1));
        assert_eq!(parsed.instructions()[0].b, b_ind(1));
    }

    // ── ORG / END ───────────────────────────────────────────────────

    #[test]
    fn org_pseudo_op_sets_start_offset() {
        let source = "
        ORG start
imp     MOV.I $0, $1
start   MOV.I $0, $2
        ";
        let parsed = parse_warrior(source).unwrap();
        assert_eq!(parsed.start_offset(), 1);
    }

    #[test]
    fn end_with_label_sets_start_offset() {
        let source = "
imp     MOV.I $0, $1
start   MOV.I $0, $2
        END start
        ";
        let parsed = parse_warrior(source).unwrap();
        assert_eq!(parsed.start_offset(), 1);
    }

    #[test]
    fn end_without_label_terminates_source_keeps_default_start() {
        let source = "
        MOV.I $0, $1
        END
        MOV.I $0, $2
        ";
        let parsed = parse_warrior(source).unwrap();
        // The line after END is ignored, so there's only one instruction.
        assert_eq!(parsed.instructions().len(), 1);
        assert_eq!(parsed.start_offset(), 0);
    }

    // ── comments and metadata ───────────────────────────────────────

    #[test]
    fn comments_are_stripped() {
        let source = "
; this is a leading comment
        MOV.I $0, $1   ; trailing comment
        ";
        let parsed = parse_warrior(source).unwrap();
        assert_eq!(parsed.instructions().len(), 1);
    }

    #[test]
    fn metadata_comments_extract_name_and_author() {
        let source = "
;name Dwarf
;author A.K. Dewdney
        MOV.I $0, $1
        ";
        let parsed = parse_warrior(source).unwrap();
        assert_eq!(parsed.name(), Some("Dwarf"));
        assert_eq!(parsed.author(), Some("A.K. Dewdney"));
    }

    // ── error cases ─────────────────────────────────────────────────

    #[test]
    fn unknown_opcode_returns_error() {
        let err = parse_warrior("FOO $0, $1").unwrap_err();
        assert!(matches!(err, ParseError::UnknownOpcode { .. }));
    }

    #[test]
    fn unknown_modifier_returns_error() {
        let err = parse_warrior("MOV.Q $0, $1").unwrap_err();
        assert!(matches!(err, ParseError::UnknownModifier { .. }));
    }

    #[test]
    fn unknown_label_returns_error() {
        let err = parse_warrior("JMP nowhere").unwrap_err();
        assert!(matches!(err, ParseError::UnknownLabel { .. }));
    }

    #[test]
    fn duplicate_label_returns_error() {
        let source = "
start   MOV.I $0, $1
start   MOV.I $0, $1
        ";
        let err = parse_warrior(source).unwrap_err();
        assert!(matches!(err, ParseError::DuplicateLabel { .. }));
    }

    #[test]
    fn empty_source_returns_empty_warrior_error() {
        let err = parse_warrior("   \n  ;just a comment\n  ").unwrap_err();
        assert_eq!(err, ParseError::EmptyWarrior);
    }

    // ── headline parser test: Dwarf source matches hand-built version ─

    /// Parse the canonical Dwarf and assert each parsed instruction matches
    /// the hand-built equivalent from `vm::tests::dwarf_bombs_core_at_intervals_of_four`
    /// exactly. This is the test that proves the parser produces correct
    /// `Instruction` values for a real warrior, not just unit-test fragments.
    #[test]
    fn parses_dwarf_matching_hand_built_version() {
        let source = "
;name Dwarf
;author A.K. Dewdney
        ORG start
start   ADD.AB #4, bomb
        MOV.I  bomb, @bomb
        JMP    start
bomb    DAT.F  #0, #0
        END
        ";
        let parsed = parse_warrior(source).unwrap();

        assert_eq!(parsed.name(), Some("Dwarf"));
        assert_eq!(parsed.author(), Some("A.K. Dewdney"));
        assert_eq!(parsed.start_offset(), 0);
        assert_eq!(parsed.instructions().len(), 4);

        // The expected instructions match the hand-built dwarf in vm.rs:
        //   ADD.AB #4, $3   (bomb is 3 cells away from `start`)
        //   MOV.I  $2, @2   (bomb is 2 cells away from this MOV)
        //   JMP.B  $-2, $0  (start is -2 cells away; default B is $0)
        //   DAT.F  #0, #0
        let expected = [
            Instruction {
                opcode: Opcode::Add,
                modifier: Modifier::AB,
                a: imm(4),
                b: dir(3),
            },
            Instruction {
                opcode: Opcode::Mov,
                modifier: Modifier::I,
                a: dir(2),
                b: b_ind(2),
            },
            Instruction {
                opcode: Opcode::Jmp,
                modifier: Modifier::B,
                a: dir(-2),
                b: dir(0),
            },
            Instruction::dat_zero(),
        ];

        for (i, exp) in expected.iter().enumerate() {
            assert_eq!(
                &parsed.instructions()[i],
                exp,
                "instruction {i} mismatch",
            );
        }
    }

    // ── headline integration test: parsed Dwarf actually runs ────────

    /// The parser is only useful if its output runs correctly through the
    /// engine. Parses Dwarf, loads it via `MatchState::load_warrior`, runs
    /// the same 15-step trace as `dwarf_bombs_core_at_intervals_of_four`,
    /// and asserts the same bomb pattern. This is the test that proves the
    /// parser → load → execute pipeline works end-to-end.
    #[test]
    fn parsed_dwarf_runs_through_match_state_and_bombs_correctly() {
        let source = "
        ORG start
start   ADD.AB #4, bomb
        MOV.I  bomb, @bomb
        JMP    start
bomb    DAT.F  #0, #0
        ";
        let parsed = parse_warrior(source).unwrap();

        let mut state = MatchState::new(64, 100);
        state.load_warrior(0, &parsed, 0);

        // 5 iterations × 3 instructions per iteration = 15 steps.
        for _ in 0..15 {
            assert!(state.step());
        }

        // The bomb pattern should match the hand-built test exactly:
        //   cell 3 (bomb itself):  B = 20 (incremented 5 times by 4)
        //   cells 7, 11, 15, 19, 23: bombs with B-values 4, 8, 12, 16, 20
        assert_eq!(state.core().get(3).b.value, 20);
        for (addr, expected_b) in [(7, 4), (11, 8), (15, 12), (19, 16), (23, 20)] {
            let cell = state.core().get(addr);
            assert_eq!(cell.opcode, Opcode::Dat, "cell {addr} should be a DAT");
            assert_eq!(cell.b.value, expected_b);
        }

        // The dwarf should still be running after 15 steps.
        assert_eq!(state.result(), MatchResult::Victory { winner_id: 0 });
    }

    // ── EQU constant tests ──────────────────────────────────────────

    #[test]
    fn equ_substitutes_constant_in_operand() {
        let source = "
step    EQU 4
        ADD #step, $1
        ";
        let parsed = parse_warrior(source).unwrap();
        // #step should resolve to #4; default modifier for ADD with #A is .AB.
        assert_eq!(parsed.instructions()[0].a, imm(4));
        assert_eq!(parsed.instructions()[0].modifier, Modifier::AB);
    }

    #[test]
    fn equ_works_in_b_operand() {
        let source = "
size    EQU 10
        DAT #0, #size
        ";
        let parsed = parse_warrior(source).unwrap();
        assert_eq!(parsed.instructions()[0].b, imm(10));
    }

    #[test]
    fn equ_does_not_count_as_instruction() {
        let err = parse_warrior("step EQU 4").unwrap_err();
        assert_eq!(err, ParseError::EmptyWarrior);
    }

    #[test]
    fn equ_duplicate_name_errors() {
        let source = "
step    EQU 4
step    EQU 5
        DAT #0, #0
        ";
        let err = parse_warrior(source).unwrap_err();
        assert!(matches!(err, ParseError::DuplicateLabel { .. }));
    }

    #[test]
    fn equ_missing_value_errors() {
        let source = "
step    EQU
        DAT #0, #0
        ";
        let err = parse_warrior(source).unwrap_err();
        assert!(matches!(err, ParseError::SyntaxError { .. }));
    }

    #[test]
    fn equ_used_in_dwarf_with_parameterized_step() {
        // A real-world use: parameterize the Dwarf's bomb interval.
        let source = "
step    EQU 8
        ORG start
start   ADD.AB #step, bomb
        MOV.I  bomb, @bomb
        JMP    start
bomb    DAT.F  #0, #0
        ";
        let parsed = parse_warrior(source).unwrap();
        // The ADD's A-operand should be #8 (from EQU).
        assert_eq!(parsed.instructions()[0].a, imm(8));
    }
}
