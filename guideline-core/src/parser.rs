//! Parser for WebSequenceDiagrams-compatible sequence diagram syntax

use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::{tag, tag_no_case, take_until, take_while, take_while1},
    character::complete::{char, digit1, space0, space1},
    combinator::{map, opt, value},
    multi::separated_list1,
    sequence::{delimited, pair, preceded},
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

    for (line_num, line) in input.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Try parsing title first
        if let Ok((_, t)) = parse_title(trimmed) {
            title = Some(t);
            continue;
        }

        match parse_line(trimmed) {
            Ok((_, item)) => {
                items.push(item);
            }
            Err(e) => {
                return Err(ParseError::SyntaxError {
                    line: line_num + 1,
                    message: format!("Failed to parse: {:?}", e),
                });
            }
        }
    }

    // Second pass: handle blocks (alt/opt/loop/par/end/else)
    let items = build_blocks(items)?;

    Ok(Diagram { title, items })
}

/// Parse a single line
fn parse_line(input: &str) -> IResult<&str, Item> {
    alt((
        parse_participant_decl,
        parse_note,
        parse_activate,
        parse_deactivate,
        parse_destroy,
        parse_autonumber,
        parse_block_keyword,
        parse_message,
    )).parse(input)
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
    )).parse(input)?;

    let (input, _) = space1.parse(input)?;

    // Parse name (possibly quoted)
    let (input, name) = parse_name(input)?;

    // Check for alias
    let (input, alias) = opt(preceded(
        (space1, tag_no_case("as"), space1),
        parse_identifier,
    )).parse(input)?;

    Ok((
        input,
        Item::ParticipantDecl {
            name: name.to_string(),
            alias: alias.map(|s| s.to_string()),
            kind,
        },
    ))
}

/// Parse a name (quoted or unquoted)
fn parse_name(input: &str) -> IResult<&str, &str> {
    alt((
        // Quoted name
        delimited(char('"'), take_until("\""), char('"')),
        // Unquoted identifier
        parse_identifier,
    )).parse(input)
}

/// Parse an identifier (alphanumeric + underscore)
fn parse_identifier(input: &str) -> IResult<&str, &str> {
    take_while1(|c: char| c.is_alphanumeric() || c == '_').parse(input)
}

/// Parse a message: `A->B: text` or `A->>B: text` etc.
fn parse_message(input: &str) -> IResult<&str, Item> {
    let (input, from) = parse_identifier(input)?;
    let (input, arrow) = parse_arrow(input)?;
    let (input, modifiers) = parse_arrow_modifiers(input)?;
    let (input, to) = parse_identifier(input)?;
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

/// Parse arrow: `->`, `->>`, `-->`, `-->>`, `->(n)`
fn parse_arrow(input: &str) -> IResult<&str, Arrow> {
    alt((
        // -->> dashed open
        value(Arrow::RESPONSE_OPEN, tag("-->>")),
        // --> dashed filled
        value(Arrow::RESPONSE, tag("-->")),
        // ->> solid open
        value(Arrow::SYNC_OPEN, tag("->>")),
        // ->(n) delayed
        map(
            delimited(tag("->("), digit1, char(')')),
            |n: &str| Arrow {
                line: LineStyle::Solid,
                head: ArrowHead::Filled,
                delay: n.parse().ok(),
            },
        ),
        // -> solid filled
        value(Arrow::SYNC, tag("->")),
    )).parse(input)
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
    )).parse(input)?;

    let (input, position) = if position == NotePosition::Over {
        let (input, _) = tag_no_case("over").parse(input)?;
        (input, NotePosition::Over)
    } else {
        let (input, _) = tag_no_case("of").parse(input)?;
        (input, position)
    };

    let (input, _) = space1.parse(input)?;

    // Parse participants (comma-separated)
    let (input, participants) = separated_list1(
        (space0, char(','), space0),
        parse_identifier,
    ).parse(input)?;

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

/// Parse activate: `activate A`
fn parse_activate(input: &str) -> IResult<&str, Item> {
    let (input, _) = tag_no_case("activate").parse(input)?;
    let (input, _) = space1.parse(input)?;
    let (_input, participant) = parse_identifier(input)?;
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
    let (_input, participant) = parse_identifier(input)?;
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
    let (_input, participant) = parse_identifier(input)?;
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

    let (input, rest) = opt(preceded(space1, take_while1(|c: char| !c.is_whitespace()))).parse(input)?;

    let (enabled, start) = match rest {
        Some("off") => (false, None),
        Some(n) => (true, n.parse().ok()),
        None => (true, None),
    };

    Ok(("", Item::Autonumber { enabled, start }))
}

/// Parse block keywords: alt, opt, loop, par, else, end
fn parse_block_keyword(input: &str) -> IResult<&str, Item> {
    alt((
        parse_block_start,
        parse_else,
        parse_end,
    )).parse(input)
}

/// Parse block start: `alt condition`, `opt condition`, `loop condition`, `par`
fn parse_block_start(input: &str) -> IResult<&str, Item> {
    let (input, kind) = alt((
        value(BlockKind::Alt, tag_no_case("alt")),
        value(BlockKind::Opt, tag_no_case("opt")),
        value(BlockKind::Loop, tag_no_case("loop")),
        value(BlockKind::Par, tag_no_case("par")),
        value(BlockKind::Seq, tag_no_case("seq")),
    )).parse(input)?;

    let (input, _) = space0.parse(input)?;
    let label = input.trim().to_string();

    // Return a marker block that will be processed later
    Ok((
        "",
        Item::Block {
            kind,
            label,
            items: vec![],
            else_items: None,
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
            else_items: None,
        },
    ))
}

/// Parse end
fn parse_end(input: &str) -> IResult<&str, Item> {
    let (_input, _) = tag_no_case("end").parse(input)?;
    Ok((
        "",
        Item::Block {
            kind: BlockKind::Alt, // marker
            label: "__END__".to_string(),
            items: vec![],
            else_items: None,
        },
    ))
}

/// Build block structure from flat list of items
fn build_blocks(items: Vec<Item>) -> Result<Vec<Item>, ParseError> {
    let mut result = Vec::new();
    let mut stack: Vec<(BlockKind, String, Vec<Item>, Option<Vec<Item>>, bool)> = Vec::new();

    for item in items {
        match &item {
            Item::Block { label, .. } if label == "__END__" => {
                // End of block
                if let Some((kind, label, items, else_items, _)) = stack.pop() {
                    let block = Item::Block {
                        kind,
                        label,
                        items,
                        else_items,
                    };
                    if let Some(parent) = stack.last_mut() {
                        if parent.4 {
                            // In else branch
                            parent.3.get_or_insert_with(Vec::new).push(block);
                        } else {
                            parent.2.push(block);
                        }
                    } else {
                        result.push(block);
                    }
                }
            }
            Item::Block { label, .. } if label.starts_with("__ELSE__") => {
                // Else marker
                if let Some(parent) = stack.last_mut() {
                    parent.4 = true; // Switch to else branch
                    parent.3 = Some(Vec::new());
                }
            }
            Item::Block { kind, label, .. } if !label.starts_with("__") => {
                // Block start
                stack.push((*kind, label.clone(), Vec::new(), None, false));
            }
            _ => {
                // Regular item
                if let Some(parent) = stack.last_mut() {
                    if parent.4 {
                        // In else branch
                        parent.3.get_or_insert_with(Vec::new).push(item);
                    } else {
                        parent.2.push(item);
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
            Item::Note { position, participants, text } => {
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
            Item::Block { kind, label, items, .. } => {
                assert_eq!(*kind, BlockKind::Opt);
                assert_eq!(label, "condition");
                assert_eq!(items.len(), 1);
            }
            _ => panic!("Expected Block"),
        }
    }

    #[test]
    fn test_alt_else_block() {
        let result = parse("alt success\nAlice->Bob: OK\nelse failure\nAlice->Bob: Error\nend").unwrap();
        assert_eq!(result.items.len(), 1);
        match &result.items[0] {
            Item::Block { kind, label, items, else_items, .. } => {
                assert_eq!(*kind, BlockKind::Alt);
                assert_eq!(label, "success");
                assert_eq!(items.len(), 1);
                assert!(else_items.is_some());
                assert_eq!(else_items.as_ref().unwrap().len(), 1);
            }
            _ => panic!("Expected Block"),
        }
    }
}
