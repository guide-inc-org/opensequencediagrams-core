//! AST definitions for sequence diagrams

/// Diagram options (parsed from option directives)
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DiagramOptions {
    /// Footer style
    pub footer: FooterStyle,
}

/// A complete sequence diagram
#[derive(Debug, Clone, PartialEq)]
pub struct Diagram {
    /// Optional title
    pub title: Option<String>,
    /// Diagram items (messages, notes, blocks, etc.)
    pub items: Vec<Item>,
    /// Diagram options
    pub options: DiagramOptions,
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
                        // Skip boundary markers [ and ]
                        if from != "[" && from != "]" {
                            add_participant(
                                from,
                                None,
                                ParticipantKind::Participant,
                                participants,
                                seen,
                            );
                        }
                        if to != "[" && to != "]" {
                            add_participant(to, None, ParticipantKind::Participant, participants, seen);
                        }
                    }
                    Item::Note { participants: note_participants, .. } => {
                        for p in note_participants {
                            add_participant(p, None, ParticipantKind::Participant, participants, seen);
                        }
                    }
                    Item::State { participants: state_participants, .. } => {
                        for p in state_participants {
                            add_participant(p, None, ParticipantKind::Participant, participants, seen);
                        }
                    }
                    Item::Ref { participants: ref_participants, input_from, output_to, .. } => {
                        // Add input_from first (e.g., Alice in "Alice->ref over Bob, Mary")
                        if let Some(from) = input_from {
                            add_participant(from, None, ParticipantKind::Participant, participants, seen);
                        }
                        // Then add ref participants (e.g., Bob, Mary)
                        for p in ref_participants {
                            add_participant(p, None, ParticipantKind::Participant, participants, seen);
                        }
                        // Finally add output_to if different
                        if let Some(to) = output_to {
                            add_participant(to, None, ParticipantKind::Participant, participants, seen);
                        }
                    }
                    Item::Activate { participant } | Item::Deactivate { participant } | Item::Destroy { participant } => {
                        add_participant(participant, None, ParticipantKind::Participant, participants, seen);
                    }
                    Item::Block {
                        items, else_sections, ..
                    } => {
                        collect_from_items(items, participants, seen);
                        for else_section in else_sections {
                            collect_from_items(&else_section.items, participants, seen);
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
    Activate { participant: String },
    /// Deactivate a participant
    Deactivate { participant: String },
    /// Destroy a participant
    Destroy { participant: String },
    /// Block (alt, opt, loop, par)
    Block {
        kind: BlockKind,
        label: String,
        items: Vec<Item>,
        /// Multiple else sections (for alt blocks with multiple else branches)
        else_sections: Vec<ElseSection>,
    },
    /// Autonumber control
    Autonumber { enabled: bool, start: Option<u32> },
    /// State box (rounded rectangle)
    State {
        participants: Vec<String>,
        text: String,
    },
    /// Reference box
    Ref {
        participants: Vec<String>,
        text: String,
        /// Input signal sender (for A->ref over B: label syntax)
        input_from: Option<String>,
        /// Input signal label
        input_label: Option<String>,
        /// Output signal receiver (for end ref-->A: label syntax)
        output_to: Option<String>,
        /// Output signal label
        output_label: Option<String>,
    },
    /// Diagram option
    DiagramOption { key: String, value: String },
    /// Extended text description (indented comment)
    Description { text: String },
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

/// Footer style for diagram (controlled by option footer=xxx)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FooterStyle {
    /// No footer at all
    None,
    /// Simple horizontal line
    Bar,
    /// Participant boxes at bottom (default, WSD compatible)
    #[default]
    Box,
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
    /// Parallel with braces syntax
    Parallel,
    /// Serial with braces syntax
    Serial,
}

impl BlockKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            BlockKind::Alt => "alt",
            BlockKind::Opt => "opt",
            BlockKind::Loop => "loop",
            BlockKind::Par => "par",
            BlockKind::Seq => "seq",
            BlockKind::Parallel => "parallel",
            BlockKind::Serial => "serial",
        }
    }
}

/// An else section within a block (for alt/opt with multiple else branches)
#[derive(Debug, Clone, PartialEq)]
pub struct ElseSection {
    /// Optional label for this else section
    pub label: Option<String>,
    /// Items in this else section
    pub items: Vec<Item>,
}
