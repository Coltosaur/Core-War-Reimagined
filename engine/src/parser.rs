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
//!   - `EQU` text substitution constants with recursive expression evaluation
//!   - Full arithmetic expressions in operand values (`label + 1` etc.)
//!   - `FOR count` / `ROF` preprocessor loops with label renaming and nesting

use std::collections::{HashMap, HashSet};

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
    /// A `ROF` was found without a matching `FOR`.
    UnmatchedRof { line: usize },
    /// A `FOR` was found without a matching `ROF`.
    UnmatchedFor { line: usize },
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
            ParseError::UnmatchedRof { line } => {
                write!(f, "line {line}: ROF without matching FOR")
            }
            ParseError::UnmatchedFor { line } => {
                write!(f, "line {line}: FOR without matching ROF")
            }
        }
    }
}

impl std::error::Error for ParseError {}

/// Preprocess FOR/ROF loops in Redcode source text.
///
/// `FOR count` ... `ROF` blocks repeat the enclosed lines `count` times.
/// The count can be any expression that resolves using only EQU constants
/// and numeric literals (labels are not available at preprocessing time).
/// Nested FOR/ROF is supported. Labels inside a FOR/ROF block are suffixed
/// with `_N` (where N is the 0-based iteration index) to avoid collisions.
///
/// EQU definitions that appear before or inside FOR/ROF blocks are collected
/// so they can be used in FOR count expressions. EQU definitions inside a
/// loop body are expanded with the same iteration suffix as labels.
///
/// This function returns the expanded source text, ready for the two-pass
/// parse.
fn preprocess_for_rof(source: &str) -> Result<String, ParseError> {
    // First pass: collect EQU definitions that appear at the top level
    // (before any FOR) so they're available for FOR count expressions.
    let mut equ_table: HashMap<String, String> = HashMap::new();
    // Scan for top-level EQUs — we need these before expansion.
    for raw in source.lines() {
        let code = strip_comment(raw);
        let code = code.trim();
        if code.is_empty() {
            continue;
        }
        // Check for EQU pseudo-op.
        let first_end = code
            .char_indices()
            .find(|(_, c)| c.is_whitespace())
            .map(|(i, _)| i)
            .unwrap_or(code.len());
        let after_first = code[first_end..].trim_start();
        if let Some(rest) = strip_keyword_ci(after_first, "EQU") {
            let equ_name = code[..first_end].trim_end_matches(':').to_string();
            let equ_value = rest.trim().to_string();
            if !equ_value.is_empty() {
                // Don't error on duplicates here — let the main parser catch that.
                equ_table.entry(equ_name).or_insert(equ_value);
            }
        }
    }

    let lines: Vec<&str> = source.lines().collect();
    let expanded = expand_for_rof_block(&lines, 0, lines.len(), &equ_table, "")?;
    Ok(expanded.join("\n"))
}

/// Recursively expand FOR/ROF blocks in `lines[start..end]`.
///
/// Returns the expanded lines. `suffix` is the current label-rename suffix
/// inherited from enclosing FOR loops (empty at the top level).
fn expand_for_rof_block(
    lines: &[&str],
    start: usize,
    end: usize,
    equ_table: &HashMap<String, String>,
    suffix: &str,
) -> Result<Vec<String>, ParseError> {
    let mut expanded = Vec::new();
    let mut i = start;

    while i < end {
        let line_no = i + 1; // 1-indexed for error messages
        let code = strip_comment(lines[i]);
        let trimmed = code.trim();

        // Check for ROF — if we hit one at this level, it's unmatched.
        if is_rof_directive(trimmed) {
            return Err(ParseError::UnmatchedRof { line: line_no });
        }

        // Check for bare `FOR` with no count — produce a clear error.
        if strip_keyword_ci(trimmed, "FOR").is_some()
            && strip_keyword_ci(trimmed, "FOR")
                .unwrap()
                .trim()
                .is_empty()
        {
            return Err(ParseError::SyntaxError {
                line: line_no,
                message: "FOR requires a count expression".to_string(),
            });
        }

        // Check for FOR directive (possibly labeled: "label FOR count").
        if let Some(for_count_expr) = detect_for_directive(trimmed) {
            let count = evaluate_for_count(&for_count_expr, line_no, equ_table)?;

            // Find the matching ROF, handling nesting.
            let body_start = i + 1;
            let rof_line = find_matching_rof(lines, body_start, end)?;

            // Collect labels defined in the body (non-recursively, just this
            // level's labels) for renaming.
            let body_labels = collect_labels_in_range(lines, body_start, rof_line);

            // Expand the body `count` times.
            for iter in 0..count {
                let iter_suffix = format!("{suffix}_{iter}");

                // Rename labels in each line of the body, then recursively
                // expand any nested FOR/ROF blocks.
                let renamed_body: Vec<String> = (body_start..rof_line)
                    .map(|j| rename_labels_in_line(lines[j], &body_labels, &iter_suffix))
                    .collect();
                let renamed_refs: Vec<&str> = renamed_body.iter().map(|s| s.as_str()).collect();

                let inner = expand_for_rof_block(
                    &renamed_refs,
                    0,
                    renamed_refs.len(),
                    equ_table,
                    "", // suffix already applied via renaming
                )?;
                expanded.extend(inner);
            }

            i = rof_line + 1; // skip past the ROF
        } else {
            // Regular line — pass through.
            expanded.push(lines[i].to_string());
            i += 1;
        }
    }

    Ok(expanded)
}

/// Detect a FOR directive in a trimmed line. Returns the count expression
/// string if found. Handles both bare `FOR expr` and labeled `label FOR expr`.
fn detect_for_directive(trimmed: &str) -> Option<String> {
    // Case 1: line starts with FOR keyword.
    if let Some(rest) = strip_keyword_ci(trimmed, "FOR") {
        let expr = rest.trim().to_string();
        if expr.is_empty() {
            return None; // FOR with no count — will be caught as syntax error
        }
        return Some(expr);
    }

    // Case 2: "label FOR expr" — first token is a label (not an opcode).
    if trimmed.is_empty() {
        return None;
    }
    let first_end = trimmed
        .char_indices()
        .find(|(_, c)| c.is_whitespace())
        .map(|(i, _)| i)
        .unwrap_or(trimmed.len());
    let first_token = &trimmed[..first_end];
    let after_first = trimmed[first_end..].trim_start();

    // The first token must NOT be a known opcode (otherwise it's an instruction).
    let first_no_modifier = first_token.split('.').next().unwrap_or(first_token);
    if parse_opcode_name(first_no_modifier).is_some() {
        return None;
    }

    if let Some(rest) = strip_keyword_ci(after_first, "FOR") {
        let expr = rest.trim().to_string();
        if expr.is_empty() {
            return None;
        }
        return Some(expr);
    }

    None
}

/// Evaluate a FOR count expression using only EQU constants and numeric
/// literals (labels are not available at preprocessing time).
fn evaluate_for_count(
    expr: &str,
    line_no: usize,
    equ_table: &HashMap<String, String>,
) -> Result<usize, ParseError> {
    let empty_labels: HashMap<String, usize> = HashMap::new();
    let tokens = tokenize_expr(expr, line_no)?;
    let mut cursor = ExprCursor {
        tokens: &tokens,
        pos: 0,
    };
    let mut visiting = HashSet::new();
    let value = cursor.parse_expr(0, line_no, &empty_labels, equ_table, &mut visiting)?;
    if cursor.pos < tokens.len() {
        return Err(ParseError::SyntaxError {
            line: line_no,
            message: format!("trailing tokens in FOR count expression: {expr:?}"),
        });
    }
    if value < 0 {
        return Err(ParseError::SyntaxError {
            line: line_no,
            message: format!("FOR count must be non-negative, got {value}"),
        });
    }
    Ok(value as usize)
}

/// Find the line index of the ROF that matches a FOR at `body_start - 1`.
/// Handles nesting by tracking depth.
fn find_matching_rof(lines: &[&str], body_start: usize, end: usize) -> Result<usize, ParseError> {
    let mut depth = 0usize;
    for (i, line) in lines.iter().enumerate().take(end).skip(body_start) {
        let code = strip_comment(line);
        let trimmed = code.trim();

        if is_for_directive_any(trimmed) {
            depth += 1;
        } else if is_rof_directive(trimmed) {
            if depth == 0 {
                return Ok(i);
            }
            depth -= 1;
        }
    }
    // No matching ROF found.
    Err(ParseError::UnmatchedFor {
        line: body_start, // 0-indexed here, but close enough — the FOR line is body_start - 1
    })
}

/// Check if a trimmed line is any FOR directive (with or without count).
/// Used for depth tracking in `find_matching_rof`.
fn is_for_directive_any(trimmed: &str) -> bool {
    // Bare `FOR` or `FOR expr`.
    if strip_keyword_ci(trimmed, "FOR").is_some() {
        return true;
    }
    // `label FOR ...` where label is not an opcode.
    if trimmed.is_empty() {
        return false;
    }
    let first_end = trimmed
        .char_indices()
        .find(|(_, c)| c.is_whitespace())
        .map(|(i, _)| i)
        .unwrap_or(trimmed.len());
    let first_token = &trimmed[..first_end];
    let after_first = trimmed[first_end..].trim_start();
    let first_no_modifier = first_token.split('.').next().unwrap_or(first_token);
    if parse_opcode_name(first_no_modifier).is_some() {
        return false;
    }
    strip_keyword_ci(after_first, "FOR").is_some()
}

/// Check if a trimmed line is a ROF directive.
fn is_rof_directive(trimmed: &str) -> bool {
    strip_keyword_ci(trimmed, "ROF").is_some()
}

/// Collect all label names defined in `lines[start..end]`.
/// A label is the first whitespace-separated token on a line if it doesn't
/// parse as a known opcode and isn't a FOR/ROF directive. EQU names (the
/// token before `EQU`) are also collected as labels for renaming.
fn collect_labels_in_range(lines: &[&str], start: usize, end: usize) -> HashSet<String> {
    let mut labels = HashSet::new();
    for line in lines.iter().take(end).skip(start) {
        let code = strip_comment(line);
        let trimmed = code.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Skip FOR and ROF directives.
        if detect_for_directive(trimmed).is_some() || is_rof_directive(trimmed) {
            continue;
        }
        let first_end = trimmed
            .char_indices()
            .find(|(_, c)| c.is_whitespace())
            .map(|(idx, _)| idx)
            .unwrap_or(trimmed.len());
        let first_token = &trimmed[..first_end];
        let first_no_modifier = first_token.split('.').next().unwrap_or(first_token);

        if parse_opcode_name(first_no_modifier).is_none() {
            let label = first_token.trim_end_matches(':').to_string();
            labels.insert(label);
        }
    }
    labels
}

/// Rename all occurrences of `labels` in `line` by appending `suffix`.
/// This handles:
///   - Label definitions (first token, possibly with trailing colon)
///   - Label references in operand values
///   - EQU names (the name before EQU)
fn rename_labels_in_line(line: &str, labels: &HashSet<String>, suffix: &str) -> String {
    // Split the line into code and comment parts to avoid renaming inside comments.
    let (code_part, comment_part) = match line.find(';') {
        Some(idx) => (&line[..idx], Some(&line[idx..])),
        None => (line, None),
    };

    // Rename identifiers in the code part. We need to be careful to only
    // rename whole-word occurrences of known labels, not substrings.
    let renamed_code = rename_identifiers_in_text(code_part, labels, suffix);

    match comment_part {
        Some(comment) => format!("{renamed_code}{comment}"),
        None => renamed_code,
    }
}

/// Replace all whole-word occurrences of identifiers from `labels` with
/// the suffixed version. A "whole word" boundary is a transition between
/// an identifier character (alphanumeric or `_`) and a non-identifier
/// character. When a matched identifier is immediately followed by `:`,
/// the suffix is inserted before the colon (label definition syntax).
fn rename_identifiers_in_text(text: &str, labels: &HashSet<String>, suffix: &str) -> String {
    if labels.is_empty() || text.is_empty() {
        return text.to_string();
    }

    let bytes = text.as_bytes();
    let mut result = String::with_capacity(text.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'_' || bytes[i].is_ascii_alphabetic() {
            let start = i;
            while i < bytes.len() && (bytes[i] == b'_' || bytes[i].is_ascii_alphanumeric()) {
                i += 1;
            }
            let ident = &text[start..i];

            if labels.contains(ident) {
                result.push_str(ident);
                result.push_str(suffix);
                // Consume a trailing colon (label definition syntax).
                if i < bytes.len() && bytes[i] == b':' {
                    result.push(':');
                    i += 1;
                }
            } else {
                result.push_str(ident);
            }
        } else {
            result.push(bytes[i] as char);
            i += 1;
        }
    }

    result
}

/// Strip the `;`-comment from a line (without extracting metadata).
/// Used during preprocessing where we only need the code portion.
fn strip_comment(line: &str) -> String {
    match line.find(';') {
        Some(idx) => line[..idx].to_string(),
        None => line.to_string(),
    }
}

/// Parse a Redcode warrior from text source.
pub fn parse_warrior(source: &str) -> Result<ParsedWarrior, ParseError> {
    // ─── Preprocessor: expand FOR/ROF loops ───
    let expanded = preprocess_for_rof(source)?;
    let source = &expanded;

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

    // Resolve start_offset from the (optional) ORG / END target. The target
    // can be a label name, an EQU constant (resolved to a number), or a
    // direct numeric literal.
    let start_offset = match start_label {
        Some(target) => {
            if let Some(&offset) = label_table.get(&target) {
                offset
            } else if let Some(value) = equ_table.get(&target) {
                value
                    .parse::<usize>()
                    .map_err(|_| ParseError::UnknownLabel {
                        line: 0,
                        label: target.clone(),
                    })?
            } else if let Ok(n) = target.parse::<usize>() {
                n
            } else {
                return Err(ParseError::UnknownLabel {
                    line: 0,
                    label: target,
                });
            }
        }
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

fn parse_opcode_token(
    token: &str,
    line_no: usize,
) -> Result<(Opcode, Option<Modifier>), ParseError> {
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

/// Parse an operand value into a signed integer. Accepts full arithmetic
/// expressions over integer literals, labels (resolved to their offset
/// relative to the executing instruction), and EQU constants (evaluated
/// recursively with cycle detection). Operators: `+ - * / %` with standard
/// precedence, unary `+ -`, and parenthesized sub-expressions.
fn parse_value(
    text: &str,
    offset: usize,
    line_no: usize,
    labels: &HashMap<String, usize>,
    equ_table: &HashMap<String, String>,
) -> Result<i32, ParseError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(ParseError::SyntaxError {
            line: line_no,
            message: "operand has no value".to_string(),
        });
    }

    let tokens = tokenize_expr(trimmed, line_no)?;
    let mut cursor = ExprCursor {
        tokens: &tokens,
        pos: 0,
    };
    let mut visiting = HashSet::new();
    let value = cursor.parse_expr(offset, line_no, labels, equ_table, &mut visiting)?;
    if cursor.pos < tokens.len() {
        return Err(ParseError::SyntaxError {
            line: line_no,
            message: format!("trailing tokens in operand expression: {trimmed:?}"),
        });
    }
    Ok(value)
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(i32),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    LParen,
    RParen,
}

fn tokenize_expr(text: &str, line_no: usize) -> Result<Vec<Tok>, ParseError> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if b.is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            let slice = &text[start..i];
            let n = slice
                .parse::<i32>()
                .map_err(|_| ParseError::InvalidNumber {
                    line: line_no,
                    text: slice.to_string(),
                })?;
            tokens.push(Tok::Num(n));
            continue;
        }
        if b == b'_' || b.is_ascii_alphabetic() {
            let start = i;
            while i < bytes.len() && (bytes[i] == b'_' || bytes[i].is_ascii_alphanumeric()) {
                i += 1;
            }
            tokens.push(Tok::Ident(text[start..i].to_string()));
            continue;
        }
        let tok = match b {
            b'+' => Tok::Plus,
            b'-' => Tok::Minus,
            b'*' => Tok::Star,
            b'/' => Tok::Slash,
            b'%' => Tok::Percent,
            b'(' => Tok::LParen,
            b')' => Tok::RParen,
            _ => {
                return Err(ParseError::SyntaxError {
                    line: line_no,
                    message: format!("unexpected character {:?} in operand value", b as char),
                });
            }
        };
        tokens.push(tok);
        i += 1;
    }
    Ok(tokens)
}

struct ExprCursor<'a> {
    tokens: &'a [Tok],
    pos: usize,
}

impl<'a> ExprCursor<'a> {
    fn peek(&self) -> Option<&Tok> {
        self.tokens.get(self.pos)
    }

    fn parse_expr(
        &mut self,
        offset: usize,
        line_no: usize,
        labels: &HashMap<String, usize>,
        equ_table: &HashMap<String, String>,
        visiting: &mut HashSet<String>,
    ) -> Result<i32, ParseError> {
        let mut lhs = self.parse_term(offset, line_no, labels, equ_table, visiting)?;
        loop {
            let add = match self.peek() {
                Some(Tok::Plus) => true,
                Some(Tok::Minus) => false,
                _ => break,
            };
            self.pos += 1;
            let rhs = self.parse_term(offset, line_no, labels, equ_table, visiting)?;
            lhs = if add {
                lhs.wrapping_add(rhs)
            } else {
                lhs.wrapping_sub(rhs)
            };
        }
        Ok(lhs)
    }

    fn parse_term(
        &mut self,
        offset: usize,
        line_no: usize,
        labels: &HashMap<String, usize>,
        equ_table: &HashMap<String, String>,
        visiting: &mut HashSet<String>,
    ) -> Result<i32, ParseError> {
        let mut lhs = self.parse_factor(offset, line_no, labels, equ_table, visiting)?;
        loop {
            enum MulOp {
                Mul,
                Div,
                Rem,
            }
            let op = match self.peek() {
                Some(Tok::Star) => MulOp::Mul,
                Some(Tok::Slash) => MulOp::Div,
                Some(Tok::Percent) => MulOp::Rem,
                _ => break,
            };
            self.pos += 1;
            let rhs = self.parse_factor(offset, line_no, labels, equ_table, visiting)?;
            lhs = match op {
                MulOp::Mul => lhs.wrapping_mul(rhs),
                MulOp::Div => {
                    if rhs == 0 {
                        return Err(ParseError::SyntaxError {
                            line: line_no,
                            message: "division by zero in operand expression".to_string(),
                        });
                    }
                    lhs.wrapping_div(rhs)
                }
                MulOp::Rem => {
                    if rhs == 0 {
                        return Err(ParseError::SyntaxError {
                            line: line_no,
                            message: "modulo by zero in operand expression".to_string(),
                        });
                    }
                    lhs.wrapping_rem(rhs)
                }
            };
        }
        Ok(lhs)
    }

    fn parse_factor(
        &mut self,
        offset: usize,
        line_no: usize,
        labels: &HashMap<String, usize>,
        equ_table: &HashMap<String, String>,
        visiting: &mut HashSet<String>,
    ) -> Result<i32, ParseError> {
        match self.peek() {
            Some(Tok::Plus) => {
                self.pos += 1;
                self.parse_factor(offset, line_no, labels, equ_table, visiting)
            }
            Some(Tok::Minus) => {
                self.pos += 1;
                let v = self.parse_factor(offset, line_no, labels, equ_table, visiting)?;
                Ok(v.wrapping_neg())
            }
            Some(Tok::LParen) => {
                self.pos += 1;
                let v = self.parse_expr(offset, line_no, labels, equ_table, visiting)?;
                match self.tokens.get(self.pos) {
                    Some(Tok::RParen) => {
                        self.pos += 1;
                        Ok(v)
                    }
                    _ => Err(ParseError::SyntaxError {
                        line: line_no,
                        message: "unmatched '(' in operand expression".to_string(),
                    }),
                }
            }
            Some(Tok::Num(n)) => {
                let n = *n;
                self.pos += 1;
                Ok(n)
            }
            Some(Tok::Ident(name)) => {
                let name = name.clone();
                self.pos += 1;
                self.resolve_ident(&name, offset, line_no, labels, equ_table, visiting)
            }
            _ => Err(ParseError::SyntaxError {
                line: line_no,
                message: "expected value in operand expression".to_string(),
            }),
        }
    }

    fn resolve_ident(
        &mut self,
        name: &str,
        offset: usize,
        line_no: usize,
        labels: &HashMap<String, usize>,
        equ_table: &HashMap<String, String>,
        visiting: &mut HashSet<String>,
    ) -> Result<i32, ParseError> {
        // EQU takes precedence over labels. Recursively evaluate the
        // substitution text as its own expression so EQU values can
        // themselves be expressions (e.g. `foo EQU bar + 1`).
        if let Some(replacement) = equ_table.get(name) {
            if !visiting.insert(name.to_string()) {
                return Err(ParseError::SyntaxError {
                    line: line_no,
                    message: format!("circular EQU reference involving {name:?}"),
                });
            }
            let sub_tokens = tokenize_expr(replacement.trim(), line_no)?;
            let mut sub = ExprCursor {
                tokens: &sub_tokens,
                pos: 0,
            };
            let value = sub.parse_expr(offset, line_no, labels, equ_table, visiting)?;
            visiting.remove(name);
            if sub.pos < sub_tokens.len() {
                return Err(ParseError::SyntaxError {
                    line: line_no,
                    message: format!("malformed EQU value for {name:?}: {replacement:?}"),
                });
            }
            return Ok(value);
        }

        if let Some(&label_offset) = labels.get(name) {
            return Ok(label_offset as i32 - offset as i32);
        }

        Err(ParseError::UnknownLabel {
            line: line_no,
            label: name.to_string(),
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
            assert_eq!(&parsed.instructions()[i], exp, "instruction {i} mismatch",);
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

    // ── Label-with-colon syntax ─────────────────────────────────────

    #[test]
    fn parses_label_with_colon_suffix() {
        let source = "
start:  ADD.AB #4, bomb
        JMP    start
bomb:   DAT.F  #0, #0
        ";
        let parsed = parse_warrior(source).unwrap();
        assert_eq!(parsed.instructions().len(), 3);
        // JMP at offset 1 references `start` at offset 0 → relative -1.
        assert_eq!(parsed.instructions()[1].a.value, -1);
    }

    // ── ORG with numeric literal and EQU constant ───────────────────

    #[test]
    fn org_with_numeric_literal() {
        let source = "
        ORG 1
        MOV.I $0, $1
        MOV.I $0, $2
        ";
        let parsed = parse_warrior(source).unwrap();
        assert_eq!(parsed.start_offset(), 1);
    }

    #[test]
    fn org_with_equ_constant() {
        let source = "
entry   EQU 1
        ORG entry
        MOV.I $0, $1
        MOV.I $0, $2
        ";
        let parsed = parse_warrior(source).unwrap();
        assert_eq!(parsed.start_offset(), 1);
    }

    // ── Malformed input error coverage ──────────────────────────────

    #[test]
    fn missing_operand_for_non_dat_opcode_errors() {
        let err = parse_warrior("ADD").unwrap_err();
        assert!(matches!(err, ParseError::SyntaxError { .. }));
    }

    #[test]
    fn too_many_operands_errors() {
        let err = parse_warrior("MOV.I $0, $1, $2").unwrap_err();
        assert!(matches!(err, ParseError::SyntaxError { .. }));
    }

    // ── Arithmetic expression tests ─────────────────────────────────

    #[test]
    fn expr_adds_constants_in_operand() {
        let parsed = parse_warrior("DAT #0, #5 + 3").unwrap();
        assert_eq!(parsed.instructions()[0].b, imm(8));
    }

    #[test]
    fn expr_subtracts_and_negates() {
        let parsed = parse_warrior("DAT #0, #10-3").unwrap();
        assert_eq!(parsed.instructions()[0].b, imm(7));
        let parsed = parse_warrior("DAT #0, #-5").unwrap();
        assert_eq!(parsed.instructions()[0].b, imm(-5));
    }

    #[test]
    fn expr_respects_precedence_and_parens() {
        let parsed = parse_warrior("DAT #0, #2 + 3 * 4").unwrap();
        assert_eq!(parsed.instructions()[0].b, imm(14));
        let parsed = parse_warrior("DAT #0, #(2 + 3) * 4").unwrap();
        assert_eq!(parsed.instructions()[0].b, imm(20));
    }

    #[test]
    fn expr_handles_div_and_mod() {
        let parsed = parse_warrior("DAT #0, #20 / 3").unwrap();
        assert_eq!(parsed.instructions()[0].b, imm(6));
        let parsed = parse_warrior("DAT #0, #20 % 3").unwrap();
        assert_eq!(parsed.instructions()[0].b, imm(2));
    }

    #[test]
    fn expr_div_by_zero_errors() {
        let err = parse_warrior("DAT #0, #5 / 0").unwrap_err();
        assert!(matches!(err, ParseError::SyntaxError { .. }));
        let err = parse_warrior("DAT #0, #5 % 0").unwrap_err();
        assert!(matches!(err, ParseError::SyntaxError { .. }));
    }

    #[test]
    fn expr_label_plus_offset() {
        // `target + 2` at offset 0 where target is at offset 3 → 3 - 0 + 2 = 5.
        let source = "
        DAT #0, target + 2
        DAT #0, #0
        DAT #0, #0
target  DAT #0, #99
        ";
        let parsed = parse_warrior(source).unwrap();
        assert_eq!(parsed.instructions()[0].b.value, 5);
    }

    #[test]
    fn expr_label_minus_label_is_distance() {
        // tail - head should resolve to the constant distance (2),
        // independent of the executing PC. (Avoiding the label name `end`
        // which collides with the END pseudo-op.)
        let source = "
head    DAT #0, #0
        DAT #0, #0
tail    DAT #0, tail - head
        ";
        let parsed = parse_warrior(source).unwrap();
        assert_eq!(parsed.instructions()[2].b.value, 2);
    }

    #[test]
    fn expr_equ_with_expression_value() {
        let source = "
base    EQU 4
step    EQU base + 1
        DAT #0, #step * 2
        ";
        let parsed = parse_warrior(source).unwrap();
        assert_eq!(parsed.instructions()[0].b, imm(10));
    }

    #[test]
    fn expr_circular_equ_errors() {
        let source = "
a       EQU b + 1
b       EQU a + 1
        DAT #0, #a
        ";
        let err = parse_warrior(source).unwrap_err();
        assert!(matches!(err, ParseError::SyntaxError { .. }));
    }

    #[test]
    fn expr_unmatched_paren_errors() {
        let err = parse_warrior("DAT #0, #(5 + 3").unwrap_err();
        assert!(matches!(err, ParseError::SyntaxError { .. }));
    }

    #[test]
    fn expr_unknown_ident_errors() {
        let err = parse_warrior("DAT #0, #5 + nope").unwrap_err();
        assert!(matches!(err, ParseError::UnknownLabel { .. }));
    }

    #[test]
    fn numeric_overflow_in_operand_errors() {
        let err = parse_warrior("MOV.I $99999999999999999, $0").unwrap_err();
        assert!(matches!(err, ParseError::InvalidNumber { .. }));
    }

    // ── FOR/ROF preprocessor loop tests ─────────────────────────────

    #[test]
    fn for_rof_basic_expansion() {
        // FOR 3 / ROF should repeat the body 3 times.
        let source = "
        FOR 3
        DAT #0, #1
        ROF
        ";
        let parsed = parse_warrior(source).unwrap();
        assert_eq!(parsed.instructions().len(), 3);
        for i in 0..3 {
            assert_eq!(parsed.instructions()[i].b, imm(1), "instruction {i}");
        }
    }

    #[test]
    fn for_rof_with_labels_renames_per_iteration() {
        // Labels inside FOR/ROF should be suffixed with _0, _1, etc.
        // Each iteration's label should resolve independently.
        let source = "
        FOR 2
target  DAT #0, #0
        JMP target
        ROF
        ";
        let parsed = parse_warrior(source).unwrap();
        // 4 instructions: DAT, JMP, DAT, JMP
        assert_eq!(parsed.instructions().len(), 4);
        // First JMP (offset 1) should point to target_0 (offset 0): relative -1.
        assert_eq!(parsed.instructions()[1].a.value, -1);
        // Second JMP (offset 3) should point to target_1 (offset 2): relative -1.
        assert_eq!(parsed.instructions()[3].a.value, -1);
    }

    #[test]
    fn for_rof_count_zero_produces_no_output() {
        let source = "
        DAT #0, #99
        FOR 0
        DAT #0, #1
        ROF
        DAT #0, #42
        ";
        let parsed = parse_warrior(source).unwrap();
        assert_eq!(parsed.instructions().len(), 2);
        assert_eq!(parsed.instructions()[0].b, imm(99));
        assert_eq!(parsed.instructions()[1].b, imm(42));
    }

    #[test]
    fn for_rof_nested() {
        // Outer loop 2 times, inner loop 3 times = 6 DATs total.
        let source = "
        FOR 2
        FOR 3
        DAT #0, #7
        ROF
        ROF
        ";
        let parsed = parse_warrior(source).unwrap();
        assert_eq!(parsed.instructions().len(), 6);
        for i in 0..6 {
            assert_eq!(parsed.instructions()[i].b, imm(7), "instruction {i}");
        }
    }

    #[test]
    fn for_rof_nested_with_labels() {
        // Nested loops with labels should all get unique suffixes.
        let source = "
        FOR 2
        FOR 2
lbl     DAT #0, #0
        JMP lbl
        ROF
        ROF
        ";
        let parsed = parse_warrior(source).unwrap();
        // 2 outer * 2 inner * 2 instructions = 8 instructions.
        assert_eq!(parsed.instructions().len(), 8);
        // Each JMP should reference its local DAT (relative -1).
        for i in (1..8).step_by(2) {
            assert_eq!(
                parsed.instructions()[i].a.value,
                -1,
                "JMP at offset {i} should reference its local DAT"
            );
        }
    }

    #[test]
    fn for_rof_with_equ_count() {
        // FOR count should accept EQU-defined constants.
        let source = "
count   EQU 3
        FOR count
        DAT #0, #5
        ROF
        ";
        let parsed = parse_warrior(source).unwrap();
        assert_eq!(parsed.instructions().len(), 3);
    }

    #[test]
    fn for_rof_with_expression_count() {
        // FOR count should accept arithmetic expressions.
        let source = "
        FOR 2 + 1
        DAT #0, #1
        ROF
        ";
        let parsed = parse_warrior(source).unwrap();
        assert_eq!(parsed.instructions().len(), 3);
    }

    #[test]
    fn for_rof_with_equ_expression_count() {
        // FOR count that uses an EQU referencing another EQU.
        let source = "
base    EQU 2
count   EQU base * 2
        FOR count
        DAT #0, #1
        ROF
        ";
        let parsed = parse_warrior(source).unwrap();
        assert_eq!(parsed.instructions().len(), 4);
    }

    #[test]
    fn for_rof_preserves_surrounding_code() {
        // Instructions before and after FOR/ROF should be preserved.
        let source = "
        MOV.I $0, $1
        FOR 2
        DAT #0, #1
        ROF
        ADD #1, $2
        ";
        let parsed = parse_warrior(source).unwrap();
        assert_eq!(parsed.instructions().len(), 4);
        assert_eq!(parsed.instructions()[0].opcode, Opcode::Mov);
        assert_eq!(parsed.instructions()[1].opcode, Opcode::Dat);
        assert_eq!(parsed.instructions()[2].opcode, Opcode::Dat);
        assert_eq!(parsed.instructions()[3].opcode, Opcode::Add);
    }

    #[test]
    fn for_rof_multiple_sequential_loops() {
        let source = "
        FOR 2
        DAT #0, #1
        ROF
        FOR 3
        DAT #0, #2
        ROF
        ";
        let parsed = parse_warrior(source).unwrap();
        assert_eq!(parsed.instructions().len(), 5);
        assert_eq!(parsed.instructions()[0].b, imm(1));
        assert_eq!(parsed.instructions()[1].b, imm(1));
        assert_eq!(parsed.instructions()[2].b, imm(2));
        assert_eq!(parsed.instructions()[3].b, imm(2));
        assert_eq!(parsed.instructions()[4].b, imm(2));
    }

    #[test]
    fn for_rof_case_insensitive() {
        let source = "
        for 2
        DAT #0, #1
        rof
        ";
        let parsed = parse_warrior(source).unwrap();
        assert_eq!(parsed.instructions().len(), 2);
    }

    #[test]
    fn for_rof_labels_across_iterations_dont_collide() {
        // Two iterations each define a label. The cross-iteration
        // references should be to their own iteration's label.
        let source = "
        FOR 3
entry   MOV.I $0, $1
        JMP   entry
        ROF
        ";
        let parsed = parse_warrior(source).unwrap();
        assert_eq!(parsed.instructions().len(), 6);
        // Each JMP targets its own iteration's MOV (relative -1).
        assert_eq!(parsed.instructions()[1].a.value, -1);
        assert_eq!(parsed.instructions()[3].a.value, -1);
        assert_eq!(parsed.instructions()[5].a.value, -1);
    }

    // ── FOR/ROF error cases ─────────────────────────────────────────

    #[test]
    fn for_rof_unmatched_rof_errors() {
        let err = parse_warrior("ROF\nDAT #0, #0").unwrap_err();
        assert!(matches!(err, ParseError::UnmatchedRof { .. }));
    }

    #[test]
    fn for_rof_unmatched_for_errors() {
        let err = parse_warrior("FOR 3\nDAT #0, #0").unwrap_err();
        assert!(matches!(err, ParseError::UnmatchedFor { .. }));
    }

    #[test]
    fn for_rof_negative_count_errors() {
        let source = "
        FOR -1
        DAT #0, #0
        ROF
        ";
        let err = parse_warrior(source).unwrap_err();
        assert!(matches!(err, ParseError::SyntaxError { .. }));
    }

    #[test]
    fn for_rof_bare_for_no_count_errors() {
        let source = "
        FOR
        DAT #0, #0
        ROF
        ";
        let err = parse_warrior(source).unwrap_err();
        assert!(matches!(err, ParseError::SyntaxError { .. }));
    }

    #[test]
    fn for_rof_equ_inside_loop_is_renamed() {
        // EQU defined inside a FOR/ROF should have its name suffixed
        // to avoid collisions across iterations.
        let source = "
        FOR 2
val     EQU 5
        DAT #0, #val
        ROF
        ";
        let parsed = parse_warrior(source).unwrap();
        assert_eq!(parsed.instructions().len(), 2);
        assert_eq!(parsed.instructions()[0].b, imm(5));
        assert_eq!(parsed.instructions()[1].b, imm(5));
    }

    #[test]
    fn for_rof_with_label_and_colon() {
        let source = "
        FOR 2
target: DAT #0, #0
        JMP target
        ROF
        ";
        let parsed = parse_warrior(source).unwrap();
        assert_eq!(parsed.instructions().len(), 4);
        // Each JMP references its own DAT.
        assert_eq!(parsed.instructions()[1].a.value, -1);
        assert_eq!(parsed.instructions()[3].a.value, -1);
    }

    #[test]
    fn for_rof_does_not_rename_outside_labels() {
        // Labels defined outside FOR/ROF should not be renamed.
        let source = "
target  DAT #0, #0
        FOR 2
        JMP target
        ROF
        ";
        let parsed = parse_warrior(source).unwrap();
        assert_eq!(parsed.instructions().len(), 3);
        // JMP at offset 1 → target at offset 0: relative -1.
        assert_eq!(parsed.instructions()[1].a.value, -1);
        // JMP at offset 2 → target at offset 0: relative -2.
        assert_eq!(parsed.instructions()[2].a.value, -2);
    }

    #[test]
    fn for_rof_multi_line_body() {
        // FOR/ROF with multiple lines in the body.
        let source = "
        FOR 2
        ADD #1, $2
        SUB #2, $3
        MOV.I $0, $1
        ROF
        ";
        let parsed = parse_warrior(source).unwrap();
        assert_eq!(parsed.instructions().len(), 6);
        assert_eq!(parsed.instructions()[0].opcode, Opcode::Add);
        assert_eq!(parsed.instructions()[1].opcode, Opcode::Sub);
        assert_eq!(parsed.instructions()[2].opcode, Opcode::Mov);
        assert_eq!(parsed.instructions()[3].opcode, Opcode::Add);
        assert_eq!(parsed.instructions()[4].opcode, Opcode::Sub);
        assert_eq!(parsed.instructions()[5].opcode, Opcode::Mov);
    }
}
