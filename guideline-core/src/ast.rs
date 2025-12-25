//! AST definitions for sequence diagrams

/// A complete sequence diagram
#[derive(Debug, Clone, PartialEq)]
pub struct Diagram {
    /// Optional title
    pub title: Option<String>,
    /// Diagram items (messages, notes, blocks, etc.)
    pub items: Vec<Item>,
}

impl Diagram {
    /// Extract all participants from the diagram in order of appearance
    pub fn participants(&self) -> Vec<Participant> {
        let mut participants = Vec::new();
        let mut seen = std::collections::HashSet::new();

        fn add_participant(
            name: &str,
            alias: Option<&str>,
            kind: ParticipantKind,
            participants: &mut Vec<Participant>,
            seen: &mut std::collections::HashSet<String>,
        ) {
            let key = alias.unwrap_or(name).to_string();
            if !seen.contains(&key) {
                seen.insert(key.clone());
                participants.push(Participant {
                    name: name.to_string(),
                    alias: alias.map(|s| s.to_string()),
                    kind,
                });
            }
        }

        fn collect_from_items(
            items: &[Item],
            participants: &mut Vec<Participant>,
            seen: &mut std::collections::HashSet<String>,
        ) {
            for item in items {
                match item {
                    Item::ParticipantDecl { name, alias, kind } => {
                        add_participant(name, alias.as_deref(), *kind, participants, seen);
                    }
                    Item::Message { from, to, .. } => {
                        add_participant(from, None, ParticipantKind::Participant, participants, seen);
                        add_participant(to, None, ParticipantKind::Participant, participants, seen);
                    }
                    Item::Block { items, else_items, .. } => {
                        collect_from_items(items, participants, seen);
                        if let Some(else_items) = else_items {
                            collect_from_items(else_items, participants, seen);
                        }
                    }
                    _ => {}
                }
            }
        }

        collect_from_items(&self.items, &mut participants, &mut seen);
        participants
    }
}

/// A participant in the sequence diagram
#[derive(Debug, Clone, PartialEq)]
pub struct Participant {
    /// Display name
    pub name: String,
    /// Optional short alias
    pub alias: Option<String>,
    /// Kind of participant (actor or regular)
    pub kind: ParticipantKind,
}

impl Participant {
    /// Get the identifier used in messages (alias if present, otherwise name)
    pub fn id(&self) -> &str {
        self.alias.as_deref().unwrap_or(&self.name)
    }
}

/// Kind of participant
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticipantKind {
    /// Regular participant (box)
    Participant,
    /// Actor (stick figure)
    Actor,
}

/// A diagram item
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    /// Participant declaration
    ParticipantDecl {
        name: String,
        alias: Option<String>,
        kind: ParticipantKind,
    },
    /// Message between participants
    Message {
        from: String,
        to: String,
        text: String,
        arrow: Arrow,
        /// Activate the receiver
        activate: bool,
        /// Deactivate the sender
        deactivate: bool,
        /// Create the receiver
        create: bool,
    },
    /// Note
    Note {
        position: NotePosition,
        participants: Vec<String>,
        text: String,
    },
    /// Activate a participant
    Activate {
        participant: String,
    },
    /// Deactivate a participant
    Deactivate {
        participant: String,
    },
    /// Destroy a participant
    Destroy {
        participant: String,
    },
    /// Block (alt, opt, loop, par)
    Block {
        kind: BlockKind,
        label: String,
        items: Vec<Item>,
        else_items: Option<Vec<Item>>,
    },
    /// Autonumber control
    Autonumber {
        enabled: bool,
        start: Option<u32>,
    },
}

/// Arrow style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arrow {
    /// Line style
    pub line: LineStyle,
    /// Arrowhead style
    pub head: ArrowHead,
    /// Delay amount (for `->(n)` syntax)
    pub delay: Option<u32>,
}

impl Arrow {
    pub const SYNC: Arrow = Arrow {
        line: LineStyle::Solid,
        head: ArrowHead::Filled,
        delay: None,
    };

    pub const SYNC_OPEN: Arrow = Arrow {
        line: LineStyle::Solid,
        head: ArrowHead::Open,
        delay: None,
    };

    pub const RESPONSE: Arrow = Arrow {
        line: LineStyle::Dashed,
        head: ArrowHead::Filled,
        delay: None,
    };

    pub const RESPONSE_OPEN: Arrow = Arrow {
        line: LineStyle::Dashed,
        head: ArrowHead::Open,
        delay: None,
    };
}

/// Line style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineStyle {
    /// Solid line (`->`)
    Solid,
    /// Dashed line (`-->`)
    Dashed,
}

/// Arrowhead style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrowHead {
    /// Filled arrowhead (`->`)
    Filled,
    /// Open arrowhead (`->>`)
    Open,
}

/// Note position
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotePosition {
    /// Left of participant
    Left,
    /// Right of participant
    Right,
    /// Over participant(s)
    Over,
}

/// Block kind
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    /// Alternative (if/else)
    Alt,
    /// Optional
    Opt,
    /// Loop
    Loop,
    /// Parallel
    Par,
    /// Sequential (inside par)
    Seq,
}

impl BlockKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            BlockKind::Alt => "alt",
            BlockKind::Opt => "opt",
            BlockKind::Loop => "loop",
            BlockKind::Par => "par",
            BlockKind::Seq => "seq",
        }
    }
}
