//! Parser for WebSequenceDiagrams-compatible sequence diagram syntax

use nom::{
    branch::alt,
    bytes::complete::{tag, tag_no_case, take_until, take_while, take_while1},
    character::complete::{char, digit1, space0, space1},
    combinator::{map, opt, value},
    multi::separated_list1,
    sequence::{delimited, pair, preceded},
    IResult, Parser,
};

use crate::ast::*;

/// Parse error
#[derive(Debug, Clone, thiserror::Error)]
pub enum ParseError {
    #[error("Parse error at line {line}: {message}")]
    SyntaxError { line: usize, message: String },
}

/// Parse a complete diagram
pub fn parse(input: &str) -> Result<Diagram, ParseError> {
    let mut items = Vec::new();
    let mut title = None;
    let lines: Vec<&str> = input.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        // Skip empty lines
        if trimmed.is_empty() {
            i += 1;
            continue;
        }

        // Task 5: Skip comment lines (# ...)
        if trimmed.starts_with('#') {
            i += 1;
            continue;
        }

        // Task 7: Extended text description (lines starting with space but not empty)
        if line.starts_with(' ') && !trimmed.is_empty() && !line.starts_with("  ") {
            // Single space indent is description
            items.push(Item::Description {
                text: trimmed.to_string(),
            });
            i += 1;
            continue;
        }

        // Try parsing title first
        if let Ok((_, t)) = parse_title(trimmed) {
            title = Some(t);
            i += 1;
            continue;
        }

        // Task 1: Check for multiline note (note without colon)
        if let Some((position, participants)) = parse_multiline_note_start(trimmed) {
            let mut note_lines = Vec::new();
            i += 1;
            while i < lines.len() {
                let note_line = lines[i].trim();
                if note_line.eq_ignore_ascii_case("end note") {
                    break;
                }
                note_lines.push(note_line);
                i += 1;
            }
            let text = note_lines.join("\\n");
            items.push(Item::Note {
                position,
                participants,
                text,
            });
            i += 1;
            continue;
        }

        // Task 3: Check for multiline ref (ref over ... without colon on same line ending with text)
        // Also handles A->ref over B: input ... end ref-->A: output
        if let Some(ref_start) = parse_multiline_ref_start(trimmed) {
            let mut ref_lines = Vec::new();
            let mut output_to: Option<String> = None;
            let mut output_label: Option<String> = None;
            i += 1;
            while i < lines.len() {
                let ref_line = lines[i].trim();
                // Check for end ref with optional output signal
                if let Some((out_to, out_label)) = parse_ref_end(ref_line) {
                    output_to = out_to;
                    output_label = out_label;
                    break;
                }
                ref_lines.push(ref_line);
                i += 1;
            }
            let text = ref_lines.join("\\n");
            items.push(Item::Ref {
                participants: ref_start.participants,
                text,
                input_from: ref_start.input_from,
                input_label: ref_start.input_label,
                output_to,
                output_label,
            });
            i += 1;
            continue;
        }

        // Task 8: Check for parallel { or serial { brace syntax
        if let Some((kind, remaining)) = parse_brace_block_start(trimmed) {
            let mut block_items = Vec::new();
            let mut brace_depth = 1;

            // Check if there's content after the opening brace on the same line
            let after_brace = remaining.trim();
            if !after_brace.is_empty() && after_brace != "{" {
                // Parse content after brace if any
            }

            i += 1;
            while i < lines.len() && brace_depth > 0 {
                let block_line = lines[i].trim();

                if block_line == "}" {
                    brace_depth -= 1;
                    if brace_depth == 0 {
                        break;
                    }
                    i += 1;
                    continue;
                }

                if !block_line.is_empty() && !block_line.starts_with('#') {
                    // Recursively parse nested content
                    if let Some((nested_kind, _)) = parse_brace_block_start(block_line) {
                        // Handle nested parallel/serial blocks
                        let mut nested_items = Vec::new();
                        let mut nested_depth = 1;
                        i += 1;

                        while i < lines.len() && nested_depth > 0 {
                            let nested_line = lines[i].trim();
                            if nested_line == "}" {
                                nested_depth -= 1;
                                if nested_depth == 0 {
                                    break;
                                }
                            } else if nested_line.ends_with('{') {
                                nested_depth += 1;
                            }

                            if nested_depth > 0
                                && !nested_line.is_empty()
                                && !nested_line.starts_with('#')
                            {
                                if let Ok((_, item)) = parse_line(nested_line) {
                                    nested_items.push(item);
                                }
                            }
                            i += 1;
                        }

                        block_items.push(Item::Block {
                            kind: nested_kind,
                            label: String::new(),
                            items: nested_items,
                            else_sections: vec![],
                        });
                    } else if let Ok((_, item)) = parse_line(block_line) {
                        block_items.push(item);
                    }
                }
                i += 1;
            }

            items.push(Item::Block {
                kind,
                label: String::new(),
                items: block_items,
                else_sections: vec![],
            });
            i += 1;
            continue;
        }

        // Regular line parsing
        match parse_line(trimmed) {
            Ok((_, item)) => {
                items.push(item);
            }
            Err(e) => {
                return Err(ParseError::SyntaxError {
                    line: i + 1,
                    message: format!("Failed to parse: {:?}", e),
                });
            }
        }
        i += 1;
    }

    // Second pass: handle blocks (alt/opt/loop/par/end/else)
    let items = build_blocks(items)?;

    // Extract options from items
    let mut options = DiagramOptions::default();
    for item in &items {
        if let Item::DiagramOption { key, value } = item {
            if key.eq_ignore_ascii_case("footer") {
                options.footer = match value.to_lowercase().as_str() {
                    "none" => FooterStyle::None,
                    "bar" => FooterStyle::Bar,
                    "box" => FooterStyle::Box,
                    _ => FooterStyle::Box,
                };
            }
        }
    }

    Ok(Diagram {
        title,
        items,
        options,
    })
}

/// Check if line starts a multiline note (note without colon)
fn parse_multiline_note_start(input: &str) -> Option<(NotePosition, Vec<String>)> {
    let input_lower = input.to_lowercase();

    // Must start with "note" but not have a colon
    if !input_lower.starts_with("note ") || input.contains(':') {
        return None;
    }

    let rest = &input[5..].trim();

    // Determine position
    let (position, after_pos) = if rest.to_lowercase().starts_with("left of ") {
        (NotePosition::Left, &rest[8..])
    } else if rest.to_lowercase().starts_with("right of ") {
        (NotePosition::Right, &rest[9..])
    } else if rest.to_lowercase().starts_with("over ") {
        (NotePosition::Over, &rest[5..])
    } else {
        return None;
    };

    // Parse participants
    let participants: Vec<String> = after_pos
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if participants.is_empty() {
        return None;
    }

    Some((position, participants))
}

/// Result of parsing a multiline ref start
struct RefStartResult {
    participants: Vec<String>,
    input_from: Option<String>,
    input_label: Option<String>,
}

/// Check if line starts a multiline ref (ref over ... without ending text)
/// Also handles A->ref over B: label syntax for input signal
fn parse_multiline_ref_start(input: &str) -> Option<RefStartResult> {
    let mut input_from: Option<String> = None;
    let mut input_label: Option<String> = None;
    let mut rest_str = input.to_string();

    // Check for "A->ref over" pattern (input signal)
    if let Some(arrow_pos) = input.to_lowercase().find("->") {
        let after_arrow = input[arrow_pos + 2..].trim_start();
        if after_arrow.to_lowercase().starts_with("ref over") {
            input_from = Some(input[..arrow_pos].trim().to_string());
            rest_str = after_arrow.to_string(); // Keep "ref over ..."
        }
    }

    let rest_lower = rest_str.to_lowercase();

    // Must start with "ref over"
    if !rest_lower.starts_with("ref over ") && !rest_lower.starts_with("ref over") {
        return None;
    }

    // Extract part after "ref over "
    let after_ref_over = if rest_lower.starts_with("ref over ") {
        &rest_str[9..]
    } else {
        &rest_str[8..]
    };
    let after_ref_over = after_ref_over.trim();

    // Check for colon (input label for single-line or multiline with label)
    let (participants_str, label) = if let Some(colon_pos) = after_ref_over.find(':') {
        let parts = after_ref_over.split_at(colon_pos);
        (parts.0.trim(), Some(parts.1[1..].trim()))
    } else {
        (after_ref_over, None)
    };

    // Parse participants
    let participants: Vec<String> = participants_str
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if participants.is_empty() {
        return None;
    }

    // If there's a label with input_from, this is "A->ref over B: label" format
    if input_from.is_some() && label.is_some() {
        input_label = label.map(|s| s.to_string());
    }

    // For multiline ref, we expect no colon (or the colon case is handled differently)
    // But if input signal is present with a colon, it's still a valid multiline ref start
    if label.is_some() && input_from.is_none() {
        // This is a single-line ref like "ref over A, B: text" - not a multiline start
        return None;
    }

    Some(RefStartResult {
        participants,
        input_from,
        input_label,
    })
}

/// Parse end ref line with optional output signal
/// Returns (output_to, output_label)
fn parse_ref_end(line: &str) -> Option<(Option<String>, Option<String>)> {
    let trimmed = line.trim();
    let lower = trimmed.to_lowercase();

    if !lower.starts_with("end ref") {
        return None;
    }

    let rest = &trimmed[7..]; // After "end ref"

    // Check for output signal "-->A: label"
    if let Some(arrow_pos) = rest.find("-->") {
        let after_arrow = &rest[arrow_pos + 3..];
        // Parse "A: label" or just "A"
        if let Some(colon_pos) = after_arrow.find(':') {
            let to = after_arrow[..colon_pos].trim().to_string();
            let label = after_arrow[colon_pos + 1..].trim().to_string();
            return Some((Some(to), Some(label)));
        } else {
            let to = after_arrow.trim().to_string();
            return Some((Some(to), None));
        }
    }

    // Simple "end ref"
    Some((None, None))
}

/// Check if line starts a brace block (parallel { or serial {)
fn parse_brace_block_start(input: &str) -> Option<(BlockKind, &str)> {
    let trimmed = input.trim();

    // Check for "parallel {" or "parallel{"
    if let Some(rest) = trimmed.strip_prefix("parallel") {
        let rest = rest.trim();
        if rest.starts_with('{') {
            return Some((BlockKind::Parallel, &rest[1..]));
        }
    }

    // Check for "serial {" or "serial{"
    if let Some(rest) = trimmed.strip_prefix("serial") {
        let rest = rest.trim();
        if rest.starts_with('{') {
            return Some((BlockKind::Serial, &rest[1..]));
        }
    }

    None
}

/// Parse a single line
fn parse_line(input: &str) -> IResult<&str, Item> {
    alt((
        parse_state,
        parse_ref_single_line,
        parse_option,
        parse_participant_decl,
        parse_note,
        parse_activate,
        parse_deactivate,
        parse_destroy,
        parse_autonumber,
        parse_block_keyword,
        parse_message,
    ))
    .parse(input)
}

/// Parse title
fn parse_title(input: &str) -> IResult<&str, String> {
    let (input, _) = tag_no_case("title").parse(input)?;
    let (input, _) = space1.parse(input)?;
    let title = input.trim().to_string();
    Ok(("", title))
}

/// Parse participant declaration: `participant Name` or `actor Name` or `participant "Long Name" as L`
fn parse_participant_decl(input: &str) -> IResult<&str, Item> {
    let (input, kind) = alt((
        value(ParticipantKind::Participant, tag_no_case("participant")),
        value(ParticipantKind::Actor, tag_no_case("actor")),
    ))
    .parse(input)?;

    let (input, _) = space1.parse(input)?;

    // Parse name (possibly quoted)
    let (input, name) = parse_name(input)?;

    // Check for alias
    let (input, alias) = opt(preceded(
        (space1, tag_no_case("as"), space1),
        parse_identifier,
    ))
    .parse(input)?;

    Ok((
        input,
        Item::ParticipantDecl {
            name: name.to_string(),
            alias: alias.map(|s| s.to_string()),
            kind,
        },
    ))
}

/// Parse a name (quoted or unquoted) - Task 6: supports colon in quoted names
fn parse_name(input: &str) -> IResult<&str, &str> {
    alt((
        // Boundary markers for gate/found/lost messages
        tag("["),
        tag("]"),
        // Quoted name (can contain colons, spaces, etc.)
        delimited(char('"'), take_until("\""), char('"')),
        // Unquoted identifier
        parse_identifier,
    ))
    .parse(input)
}

/// Parse an identifier (alphanumeric + underscore)
fn parse_identifier(input: &str) -> IResult<&str, &str> {
    take_while1(|c: char| c.is_alphanumeric() || c == '_').parse(input)
}

/// Parse a message: `A->B: text` or `A->>B: text` etc.
/// Task 6: Now supports quoted names with colons
fn parse_message(input: &str) -> IResult<&str, Item> {
    let (input, from) = parse_name(input)?;
    let (input, arrow) = parse_arrow(input)?;
    let (input, modifiers) = parse_arrow_modifiers(input)?;
    let (input, to) = parse_name(input)?;
    let (input, _) = opt(char(':')).parse(input)?;
    let (input, _) = space0.parse(input)?;
    let text = input.trim().to_string();

    Ok((
        "",
        Item::Message {
            from: from.to_string(),
            to: to.to_string(),
            text,
            arrow,
            activate: modifiers.0,
            deactivate: modifiers.1,
            create: modifiers.2,
        },
    ))
}

/// Parse arrow: `->`, `->>`, `-->`, `-->>`, `->(n)`, `<->`, `<-->`
/// Task 9: Added bidirectional arrow support (though WSD may not use it)
fn parse_arrow(input: &str) -> IResult<&str, Arrow> {
    alt((
        // <--> bidirectional dashed (if needed)
        value(Arrow::RESPONSE, tag("<-->")),
        // <-> bidirectional solid (if needed)
        value(Arrow::SYNC, tag("<->")),
        // -->> dashed open
        value(Arrow::RESPONSE_OPEN, tag("-->>")),
        // --> dashed filled
        value(Arrow::RESPONSE, tag("-->")),
        // ->> solid open
        value(Arrow::SYNC_OPEN, tag("->>")),
        // ->(n) delayed
        map(delimited(tag("->("), digit1, char(')')), |n: &str| Arrow {
            line: LineStyle::Solid,
            head: ArrowHead::Filled,
            delay: n.parse().ok(),
        }),
        // -> solid filled
        value(Arrow::SYNC, tag("->")),
    ))
    .parse(input)
}

/// Parse arrow modifiers: `+` (activate), `-` (deactivate), `*` (create)
fn parse_arrow_modifiers(input: &str) -> IResult<&str, (bool, bool, bool)> {
    let (input, mods) = take_while(|c| c == '+' || c == '-' || c == '*').parse(input)?;
    let activate = mods.contains('+');
    let deactivate = mods.contains('-');
    let create = mods.contains('*');
    Ok((input, (activate, deactivate, create)))
}

/// Parse note: `note left of A: text`, `note right of A: text`, `note over A: text`, `note over A,B: text`
fn parse_note(input: &str) -> IResult<&str, Item> {
    let (input, _) = tag_no_case("note").parse(input)?;
    let (input, _) = space1.parse(input)?;

    let (input, position) = alt((
        value(NotePosition::Left, pair(tag_no_case("left"), space1)),
        value(NotePosition::Right, pair(tag_no_case("right"), space1)),
        value(NotePosition::Over, tag_no_case("")),
    ))
    .parse(input)?;

    let (input, position) = if position == NotePosition::Over {
        let (input, _) = tag_no_case("over").parse(input)?;
        (input, NotePosition::Over)
    } else {
        let (input, _) = tag_no_case("of").parse(input)?;
        (input, position)
    };

    let (input, _) = space1.parse(input)?;

    // Parse participants (comma-separated) - support quoted names
    let (input, participants) =
        separated_list1((space0, char(','), space0), parse_name).parse(input)?;

    let (input, _) = opt(char(':')).parse(input)?;
    let (input, _) = space0.parse(input)?;
    let text = input.trim().to_string();

    Ok((
        "",
        Item::Note {
            position,
            participants: participants.into_iter().map(|s| s.to_string()).collect(),
            text,
        },
    ))
}

/// Task 2: Parse state: `state over A: text` or `state over A,B: text`
fn parse_state(input: &str) -> IResult<&str, Item> {
    let (input, _) = tag_no_case("state").parse(input)?;
    let (input, _) = space1.parse(input)?;
    let (input, _) = tag_no_case("over").parse(input)?;
    let (input, _) = space1.parse(input)?;

    // Parse participants (comma-separated)
    let (input, participants) =
        separated_list1((space0, char(','), space0), parse_name).parse(input)?;

    let (input, _) = opt(char(':')).parse(input)?;
    let (input, _) = space0.parse(input)?;
    let text = input.trim().to_string();

    Ok((
        "",
        Item::State {
            participants: participants.into_iter().map(|s| s.to_string()).collect(),
            text,
        },
    ))
}

/// Task 3: Parse single-line ref: `ref over A,B: text`
fn parse_ref_single_line(input: &str) -> IResult<&str, Item> {
    let (input, _) = tag_no_case("ref").parse(input)?;
    let (input, _) = space1.parse(input)?;
    let (input, _) = tag_no_case("over").parse(input)?;
    let (input, _) = space1.parse(input)?;

    // Parse participants (comma-separated)
    let (input, participants) =
        separated_list1((space0, char(','), space0), parse_name).parse(input)?;

    let (input, _) = char(':').parse(input)?;
    let (input, _) = space0.parse(input)?;
    let text = input.trim().to_string();

    Ok((
        "",
        Item::Ref {
            participants: participants.into_iter().map(|s| s.to_string()).collect(),
            text,
            input_from: None,
            input_label: None,
            output_to: None,
            output_label: None,
        },
    ))
}

/// Task 4: Parse option: `option key=value`
fn parse_option(input: &str) -> IResult<&str, Item> {
    let (input, _) = tag_no_case("option").parse(input)?;
    let (input, _) = space1.parse(input)?;
    let (input, key) = take_while1(|c: char| c.is_alphanumeric() || c == '_').parse(input)?;
    let (input, _) = char('=').parse(input)?;
    let (_input, value) = take_while1(|c: char| !c.is_whitespace()).parse(input)?;

    Ok((
        "",
        Item::DiagramOption {
            key: key.to_string(),
            value: value.to_string(),
        },
    ))
}

/// Parse activate: `activate A`
fn parse_activate(input: &str) -> IResult<&str, Item> {
    let (input, _) = tag_no_case("activate").parse(input)?;
    let (input, _) = space1.parse(input)?;
    let (_input, participant) = parse_name(input)?;
    Ok((
        "",
        Item::Activate {
            participant: participant.to_string(),
        },
    ))
}

/// Parse deactivate: `deactivate A`
fn parse_deactivate(input: &str) -> IResult<&str, Item> {
    let (input, _) = tag_no_case("deactivate").parse(input)?;
    let (input, _) = space1.parse(input)?;
    let (_input, participant) = parse_name(input)?;
    Ok((
        "",
        Item::Deactivate {
            participant: participant.to_string(),
        },
    ))
}

/// Parse destroy: `destroy A`
fn parse_destroy(input: &str) -> IResult<&str, Item> {
    let (input, _) = tag_no_case("destroy").parse(input)?;
    let (input, _) = space1.parse(input)?;
    let (_input, participant) = parse_name(input)?;
    Ok((
        "",
        Item::Destroy {
            participant: participant.to_string(),
        },
    ))
}

/// Parse autonumber: `autonumber` or `autonumber off` or `autonumber 5`
fn parse_autonumber(input: &str) -> IResult<&str, Item> {
    let (input, _) = tag_no_case("autonumber").parse(input)?;

    let (_input, rest) =
        opt(preceded(space1, take_while1(|c: char| !c.is_whitespace()))).parse(input)?;

    let (enabled, start) = match rest {
        Some("off") => (false, None),
        Some(n) => (true, n.parse().ok()),
        None => (true, None),
    };

    Ok(("", Item::Autonumber { enabled, start }))
}

/// Parse block keywords: alt, opt, loop, par, else, end
fn parse_block_keyword(input: &str) -> IResult<&str, Item> {
    alt((parse_block_start, parse_else, parse_end)).parse(input)
}

/// Parse block start: `alt condition`, `opt condition`, `loop condition`, `par`, `seq`
fn parse_block_start(input: &str) -> IResult<&str, Item> {
    let (input, kind) = alt((
        value(BlockKind::Alt, tag_no_case("alt")),
        value(BlockKind::Opt, tag_no_case("opt")),
        value(BlockKind::Loop, tag_no_case("loop")),
        value(BlockKind::Par, tag_no_case("par")),
        value(BlockKind::Seq, tag_no_case("seq")),
    ))
    .parse(input)?;

    let (input, _) = space0.parse(input)?;
    let label = input.trim().to_string();

    // Return a marker block that will be processed later
    Ok((
        "",
        Item::Block {
            kind,
            label,
            items: vec![],
            else_sections: vec![],
        },
    ))
}

/// Parse else: `else condition`
fn parse_else(input: &str) -> IResult<&str, Item> {
    let (input, _) = tag_no_case("else").parse(input)?;
    let (input, _) = space0.parse(input)?;
    let label = input.trim().to_string();

    // Return a marker that will be processed during block building
    Ok((
        "",
        Item::Block {
            kind: BlockKind::Alt, // marker
            label: format!("__ELSE__{}", label),
            items: vec![],
            else_sections: vec![],
        },
    ))
}

/// Parse end (but not "end note" or "end ref")
fn parse_end(input: &str) -> IResult<&str, Item> {
    let trimmed = input.trim().to_lowercase();
    // Don't match "end note" or "end ref" - those are handled separately
    if trimmed.starts_with("end note") || trimmed.starts_with("end ref") {
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Tag,
        )));
    }
    let (_input, _) = tag_no_case("end").parse(input)?;
    Ok((
        "",
        Item::Block {
            kind: BlockKind::Alt, // marker
            label: "__END__".to_string(),
            items: vec![],
            else_sections: vec![],
        },
    ))
}

/// Build block structure from flat list of items
fn build_blocks(items: Vec<Item>) -> Result<Vec<Item>, ParseError> {
    use crate::ast::ElseSection;

    let mut result = Vec::new();
    // Stack entry: (kind, label, items, else_sections, current_else_items, current_else_label, in_else_branch)
    struct StackEntry {
        kind: BlockKind,
        label: String,
        items: Vec<Item>,
        else_sections: Vec<ElseSection>,
        current_else_items: Vec<Item>,
        current_else_label: Option<String>,
        in_else_branch: bool,
    }
    let mut stack: Vec<StackEntry> = Vec::new();

    for item in items {
        match &item {
            Item::Block { label, .. } if label == "__END__" => {
                // End of block
                if let Some(mut entry) = stack.pop() {
                    // If we were in an else branch, finalize it
                    if entry.in_else_branch && !entry.current_else_items.is_empty() {
                        entry.else_sections.push(ElseSection {
                            label: entry.current_else_label.take(),
                            items: std::mem::take(&mut entry.current_else_items),
                        });
                    }
                    let block = Item::Block {
                        kind: entry.kind,
                        label: entry.label,
                        items: entry.items,
                        else_sections: entry.else_sections,
                    };
                    if let Some(parent) = stack.last_mut() {
                        if parent.in_else_branch {
                            parent.current_else_items.push(block);
                        } else {
                            parent.items.push(block);
                        }
                    } else {
                        result.push(block);
                    }
                }
            }
            Item::Block { label, .. } if label.starts_with("__ELSE__") => {
                // Else marker - extract the else label
                let else_label_text = label.strip_prefix("__ELSE__").unwrap_or("").to_string();
                if let Some(entry) = stack.last_mut() {
                    // If we were already in an else branch, save the current one
                    if entry.in_else_branch && !entry.current_else_items.is_empty() {
                        entry.else_sections.push(ElseSection {
                            label: entry.current_else_label.take(),
                            items: std::mem::take(&mut entry.current_else_items),
                        });
                    }
                    // Start new else branch
                    entry.in_else_branch = true;
                    entry.current_else_items = Vec::new();
                    entry.current_else_label = if else_label_text.is_empty() {
                        None
                    } else {
                        Some(else_label_text)
                    };
                }
            }
            Item::Block {
                kind,
                label,
                items,
                else_sections,
                ..
            } if !label.starts_with("__") => {
                // Check if this is a completed block (parallel/serial with items already)
                if matches!(kind, BlockKind::Parallel | BlockKind::Serial) || !items.is_empty() {
                    // Already a complete block, add directly
                    let block = Item::Block {
                        kind: *kind,
                        label: label.clone(),
                        items: items.clone(),
                        else_sections: else_sections.clone(),
                    };
                    if let Some(parent) = stack.last_mut() {
                        if parent.in_else_branch {
                            parent.current_else_items.push(block);
                        } else {
                            parent.items.push(block);
                        }
                    } else {
                        result.push(block);
                    }
                } else {
                    // Block start marker
                    stack.push(StackEntry {
                        kind: *kind,
                        label: label.clone(),
                        items: Vec::new(),
                        else_sections: Vec::new(),
                        current_else_items: Vec::new(),
                        current_else_label: None,
                        in_else_branch: false,
                    });
                }
            }
            _ => {
                // Regular item
                if let Some(parent) = stack.last_mut() {
                    if parent.in_else_branch {
                        parent.current_else_items.push(item);
                    } else {
                        parent.items.push(item);
                    }
                } else {
                    result.push(item);
                }
            }
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_message() {
        let result = parse("Alice->Bob: Hello").unwrap();
        assert_eq!(result.items.len(), 1);
        match &result.items[0] {
            Item::Message { from, to, text, .. } => {
                assert_eq!(from, "Alice");
                assert_eq!(to, "Bob");
                assert_eq!(text, "Hello");
            }
            _ => panic!("Expected Message"),
        }
    }

    #[test]
    fn test_participant_decl() {
        let result = parse("participant Alice\nactor Bob").unwrap();
        assert_eq!(result.items.len(), 2);
    }

    #[test]
    fn test_note() {
        let result = parse("note over Alice: Hello").unwrap();
        assert_eq!(result.items.len(), 1);
        match &result.items[0] {
            Item::Note {
                position,
                participants,
                text,
            } => {
                assert_eq!(*position, NotePosition::Over);
                assert_eq!(participants, &["Alice"]);
                assert_eq!(text, "Hello");
            }
            _ => panic!("Expected Note"),
        }
    }

    #[test]
    fn test_opt_block() {
        let result = parse("opt condition\nAlice->Bob: Hello\nend").unwrap();
        assert_eq!(result.items.len(), 1);
        match &result.items[0] {
            Item::Block {
                kind, label, items, ..
            } => {
                assert_eq!(*kind, BlockKind::Opt);
                assert_eq!(label, "condition");
                assert_eq!(items.len(), 1);
            }
            _ => panic!("Expected Block"),
        }
    }

    #[test]
    fn test_alt_else_block() {
        let result =
            parse("alt success\nAlice->Bob: OK\nelse failure\nAlice->Bob: Error\nend").unwrap();
        assert_eq!(result.items.len(), 1);
        match &result.items[0] {
            Item::Block {
                kind,
                label,
                items,
                else_sections,
                ..
            } => {
                assert_eq!(*kind, BlockKind::Alt);
                assert_eq!(label, "success");
                assert_eq!(items.len(), 1);
                assert_eq!(else_sections.len(), 1);
                assert_eq!(else_sections[0].items.len(), 1);
            }
            _ => panic!("Expected Block"),
        }
    }

    // Task 5: Comment test
    #[test]
    fn test_comment() {
        let result = parse("# This is a comment\nAlice->Bob: Hello").unwrap();
        assert_eq!(result.items.len(), 1);
        match &result.items[0] {
            Item::Message { from, to, text, .. } => {
                assert_eq!(from, "Alice");
                assert_eq!(to, "Bob");
                assert_eq!(text, "Hello");
            }
            _ => panic!("Expected Message"),
        }
    }

    // Task 1: Multiline note test
    #[test]
    fn test_multiline_note() {
        let input = r#"note left of Alice
Line 1
Line 2
end note"#;
        let result = parse(input).unwrap();
        assert_eq!(result.items.len(), 1);
        match &result.items[0] {
            Item::Note {
                position,
                participants,
                text,
            } => {
                assert_eq!(*position, NotePosition::Left);
                assert_eq!(participants, &["Alice"]);
                assert_eq!(text, "Line 1\\nLine 2");
            }
            _ => panic!("Expected Note"),
        }
    }

    // Task 2: State test
    #[test]
    fn test_state() {
        let result = parse("state over Server: LISTEN").unwrap();
        assert_eq!(result.items.len(), 1);
        match &result.items[0] {
            Item::State { participants, text } => {
                assert_eq!(participants, &["Server"]);
                assert_eq!(text, "LISTEN");
            }
            _ => panic!("Expected State"),
        }
    }

    // Task 3: Ref test
    #[test]
    fn test_ref() {
        let result = parse("ref over Alice, Bob: See other diagram").unwrap();
        assert_eq!(result.items.len(), 1);
        match &result.items[0] {
            Item::Ref {
                participants, text, ..
            } => {
                assert_eq!(participants, &["Alice", "Bob"]);
                assert_eq!(text, "See other diagram");
            }
            _ => panic!("Expected Ref"),
        }
    }

    #[test]
    fn test_ref_input_signal_multiline() {
        let input = r#"Alice->ref over Bob, Carol: Input signal
line 1
line 2
end ref-->Alice: Output signal"#;
        let result = parse(input).unwrap();
        assert_eq!(result.items.len(), 1);
        match &result.items[0] {
            Item::Ref {
                participants,
                text,
                input_from,
                input_label,
                output_to,
                output_label,
            } => {
                assert_eq!(participants, &["Bob", "Carol"]);
                assert_eq!(text, "line 1\\nline 2");
                assert_eq!(input_from.as_deref(), Some("Alice"));
                assert_eq!(input_label.as_deref(), Some("Input signal"));
                assert_eq!(output_to.as_deref(), Some("Alice"));
                assert_eq!(output_label.as_deref(), Some("Output signal"));
            }
            _ => panic!("Expected Ref"),
        }
    }

    // Task 4: Option test
    #[test]
    fn test_option() {
        let result = parse("option footer=none").unwrap();
        assert_eq!(result.items.len(), 1);
        match &result.items[0] {
            Item::DiagramOption { key, value } => {
                assert_eq!(key, "footer");
                assert_eq!(value, "none");
            }
            _ => panic!("Expected DiagramOption"),
        }
    }

    // Task 6: Quoted name with colon test
    #[test]
    fn test_quoted_name_with_colon() {
        let result = parse(r#"":Alice"->":Bob": Hello"#).unwrap();
        assert_eq!(result.items.len(), 1);
        match &result.items[0] {
            Item::Message { from, to, text, .. } => {
                assert_eq!(from, ":Alice");
                assert_eq!(to, ":Bob");
                assert_eq!(text, "Hello");
            }
            _ => panic!("Expected Message"),
        }
    }
}
