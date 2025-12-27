//! SVG renderer for sequence diagrams

use crate::ast::*;
use crate::theme::{LifelineStyle, ParticipantShape, Theme};
use std::collections::HashMap;
use std::fmt::Write;

/// Rendering configuration
#[derive(Debug, Clone)]
pub struct Config {
    /// Padding around the diagram
    pub padding: f64,
    /// Space between participants
    pub participant_gap: f64,
    /// Height of participant header/footer box
    pub header_height: f64,
    /// Height of each row (message, note, etc.)
    pub row_height: f64,
    /// Width of participant box
    pub participant_width: f64,
    /// Font size
    pub font_size: f64,
    /// Activation box width
    pub activation_width: f64,
    /// Note padding
    pub note_padding: f64,
    /// Block margin
    pub block_margin: f64,
    /// Title height (when title exists)
    pub title_height: f64,
    /// Theme for styling
    pub theme: Theme,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            padding: 20.0,
            participant_gap: 150.0,
            header_height: 40.0,
            row_height: 50.0,
            participant_width: 100.0,
            font_size: 14.0,
            activation_width: 10.0,
            note_padding: 8.0,
            block_margin: 10.0,
            title_height: 30.0,
            theme: Theme::default(),
        }
    }
}

impl Config {
    /// Set the theme
    pub fn with_theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

/// Block background info for deferred rendering
#[derive(Debug, Clone)]
struct BlockBackground {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

/// Block label info for deferred rendering (rendered above lifelines)
#[derive(Debug, Clone)]
struct BlockLabel {
    x1: f64,
    start_y: f64,
    end_y: f64,
    x2: f64,
    kind: String,
    label: String,
    else_y: Option<f64>,
}

/// Render state
struct RenderState {
    config: Config,
    participants: Vec<Participant>,
    participant_x: HashMap<String, f64>,
    participant_widths: HashMap<String, f64>,
    current_y: f64,
    activations: HashMap<String, Vec<(f64, Option<f64>)>>,
    autonumber: Option<u32>,
    destroyed: HashMap<String, f64>,
    has_title: bool,
    total_width: f64,
    /// Collected block backgrounds for deferred rendering
    block_backgrounds: Vec<BlockBackground>,
    /// Collected block labels for deferred rendering (above lifelines)
    block_labels: Vec<BlockLabel>,
    /// Footer style from diagram options
    footer_style: FooterStyle,
}

/// Estimate text width in pixels (rough approximation)
fn estimate_text_width(text: &str, font_size: f64) -> f64 {
    // Handle multiline text - take the longest line
    let max_line_len = text.split("\\n").map(|line| {
        // Count characters, accounting for different widths
        line.chars().map(|c| {
            if c.is_ascii() {
                if c.is_uppercase() { 0.7 } else { 0.5 }
            } else {
                1.0 // CJK and other characters are wider
            }
        }).sum::<f64>()
    }).fold(0.0_f64, |a, b| a.max(b));

    max_line_len * font_size * 1.0 + 16.0 // Add padding
}

/// Calculate dynamic gaps between participants based on message text lengths
fn calculate_participant_gaps(
    participants: &[Participant],
    items: &[Item],
    config: &Config,
) -> Vec<f64> {
    if participants.len() <= 1 {
        return vec![];
    }

    // Create a map from participant id to index
    let mut participant_index: HashMap<String, usize> = HashMap::new();
    for (i, p) in participants.iter().enumerate() {
        participant_index.insert(p.id().to_string(), i);
    }

    // Initialize gaps with minimum gap
    let min_gap = config.participant_gap * 0.6; // Minimum gap (60% of default)
    let mut gaps: Vec<f64> = vec![min_gap; participants.len() - 1];

    // Calculate max text width for each adjacent pair
    fn process_items(
        items: &[Item],
        participant_index: &HashMap<String, usize>,
        gaps: &mut Vec<f64>,
        config: &Config,
    ) {
        for item in items {
            match item {
                Item::Message { from, to, text, .. } => {
                    if let (Some(&from_idx), Some(&to_idx)) =
                        (participant_index.get(from), participant_index.get(to))
                    {
                        if from_idx != to_idx {
                            let (min_idx, max_idx) = if from_idx < to_idx {
                                (from_idx, to_idx)
                            } else {
                                (to_idx, from_idx)
                            };

                            // Calculate text width (estimate ~8px per char, more for CJK)
                            let text_width = text.chars().count() as f64 * 8.0 + 40.0;

                            // Distribute needed width across gaps between the participants
                            let gap_count = (max_idx - min_idx) as f64;
                            let needed_gap = text_width / gap_count + config.participant_width * 0.3;

                            // Update gaps between the participants
                            for gap_idx in min_idx..max_idx {
                                if needed_gap > gaps[gap_idx] {
                                    gaps[gap_idx] = needed_gap;
                                }
                            }
                        }
                    }
                }
                Item::Block { items, else_items, .. } => {
                    process_items(items, participant_index, gaps, config);
                    if let Some(else_items) = else_items {
                        process_items(else_items, participant_index, gaps, config);
                    }
                }
                _ => {}
            }
        }
    }

    process_items(items, &participant_index, &mut gaps, config);

    // Also consider participant name lengths
    for i in 0..participants.len() - 1 {
        let name1_width = participants[i].name.chars().count() as f64 * 8.0;
        let name2_width = participants[i + 1].name.chars().count() as f64 * 8.0;
        let needed_for_names = (name1_width + name2_width) / 2.0 + 20.0;
        if needed_for_names > gaps[i] {
            gaps[i] = needed_for_names;
        }
    }

    // Cap maximum gap
    let max_gap = config.participant_gap * 2.0;
    for gap in &mut gaps {
        if *gap > max_gap {
            *gap = max_gap;
        }
    }

    gaps
}

impl RenderState {
    fn new(config: Config, participants: Vec<Participant>, items: &[Item], has_title: bool, footer_style: FooterStyle) -> Self {
        // Calculate individual participant widths based on their names
        let mut participant_widths: HashMap<String, f64> = HashMap::new();
        let min_width = config.participant_width;

        for p in &participants {
            let text_width = estimate_text_width(&p.name, config.font_size);
            let width = text_width.max(min_width);
            participant_widths.insert(p.id().to_string(), width);
        }

        let gaps = calculate_participant_gaps(&participants, items, &config);

        // Left margin for notes/actions on leftmost participant
        let left_margin = 100.0;
        // Right margin for self-loops and notes on rightmost participant
        let right_margin = 140.0;

        let mut participant_x = HashMap::new();
        let first_width = participants.first()
            .map(|p| *participant_widths.get(p.id()).unwrap_or(&min_width))
            .unwrap_or(min_width);
        let mut current_x = config.padding + left_margin + first_width / 2.0;

        for (i, p) in participants.iter().enumerate() {
            participant_x.insert(p.id().to_string(), current_x);
            if i < gaps.len() {
                let current_width = *participant_widths.get(p.id()).unwrap_or(&min_width);
                let next_width = participants.get(i + 1)
                    .map(|np| *participant_widths.get(np.id()).unwrap_or(&min_width))
                    .unwrap_or(min_width);
                // Gap is between the edges of adjacent participants
                let actual_gap = gaps[i].max((current_width + next_width) / 2.0 + 30.0);
                current_x += actual_gap;
            }
        }

        let last_width = participants.last()
            .map(|p| *participant_widths.get(p.id()).unwrap_or(&min_width))
            .unwrap_or(min_width);
        let total_width = current_x + last_width / 2.0 + right_margin + config.padding;

        Self {
            config,
            participants,
            participant_x,
            participant_widths,
            current_y: 0.0,
            activations: HashMap::new(),
            autonumber: None,
            destroyed: HashMap::new(),
            has_title,
            total_width,
            block_backgrounds: Vec::new(),
            block_labels: Vec::new(),
            footer_style,
        }
    }

    fn get_participant_width(&self, name: &str) -> f64 {
        *self.participant_widths.get(name).unwrap_or(&self.config.participant_width)
    }

    fn get_x(&self, name: &str) -> f64 {
        *self.participant_x.get(name).unwrap_or(&0.0)
    }

    fn diagram_width(&self) -> f64 {
        self.total_width
    }

    /// Get the x position of the leftmost participant
    fn leftmost_x(&self) -> f64 {
        self.participants
            .first()
            .map(|p| self.get_x(p.id()))
            .unwrap_or(self.config.padding)
    }

    /// Get the x position of the rightmost participant
    fn rightmost_x(&self) -> f64 {
        self.participants
            .last()
            .map(|p| self.get_x(p.id()))
            .unwrap_or(self.total_width - self.config.padding)
    }

    /// Get block left boundary (based on leftmost participant)
    fn block_left(&self) -> f64 {
        let leftmost_width = self.participants.first()
            .map(|p| self.get_participant_width(p.id()))
            .unwrap_or(self.config.participant_width);
        self.leftmost_x() - leftmost_width / 2.0 - self.config.block_margin
    }

    /// Get block right boundary (based on rightmost participant)
    fn block_right(&self) -> f64 {
        let rightmost_width = self.participants.last()
            .map(|p| self.get_participant_width(p.id()))
            .unwrap_or(self.config.participant_width);
        self.rightmost_x() + rightmost_width / 2.0 + self.config.block_margin
    }

    fn header_top(&self) -> f64 {
        if self.has_title {
            self.config.padding + self.config.title_height
        } else {
            self.config.padding
        }
    }

    fn content_start(&self) -> f64 {
        self.header_top() + self.config.header_height + 20.0
    }

    fn next_number(&mut self) -> Option<u32> {
        self.autonumber.map(|n| {
            self.autonumber = Some(n + 1);
            n
        })
    }

    /// Add a block background to be rendered later
    fn add_block_background(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.block_backgrounds.push(BlockBackground { x, y, width, height });
    }

    /// Add a block label to be rendered later (above lifelines)
    fn add_block_label(&mut self, x1: f64, start_y: f64, end_y: f64, x2: f64, kind: &str, label: &str, else_y: Option<f64>) {
        self.block_labels.push(BlockLabel {
            x1,
            start_y,
            end_y,
            x2,
            kind: kind.to_string(),
            label: label.to_string(),
            else_y,
        });
    }
}

/// Find participants involved in a list of items (returns min and max x positions)
fn find_involved_participants(items: &[Item], state: &RenderState) -> Option<(f64, f64)> {
    let mut min_x: Option<f64> = None;
    let mut max_x: Option<f64> = None;

    fn update_bounds(participant: &str, state: &RenderState, min_x: &mut Option<f64>, max_x: &mut Option<f64>) {
        let x = state.get_x(participant);
        if x > 0.0 {
            *min_x = Some(min_x.map_or(x, |m| m.min(x)));
            *max_x = Some(max_x.map_or(x, |m| m.max(x)));
        }
    }

    fn process_items(items: &[Item], state: &RenderState, min_x: &mut Option<f64>, max_x: &mut Option<f64>) {
        for item in items {
            match item {
                Item::Message { from, to, .. } => {
                    update_bounds(from, state, min_x, max_x);
                    update_bounds(to, state, min_x, max_x);
                }
                Item::Note { participants, .. } => {
                    for p in participants {
                        update_bounds(p, state, min_x, max_x);
                    }
                }
                Item::Block { items, else_items, .. } => {
                    process_items(items, state, min_x, max_x);
                    if let Some(else_items) = else_items {
                        process_items(else_items, state, min_x, max_x);
                    }
                }
                Item::Activate { participant } | Item::Deactivate { participant } | Item::Destroy { participant } => {
                    update_bounds(participant, state, min_x, max_x);
                }
                _ => {}
            }
        }
    }

    process_items(items, state, &mut min_x, &mut max_x);

    match (min_x, max_x) {
        (Some(min), Some(max)) => Some((min, max)),
        _ => None,
    }
}

/// Calculate block x boundaries based on involved participants and label length
fn calculate_block_bounds_with_label(
    items: &[Item],
    else_items: Option<&[Item]>,
    label: &str,
    kind: &str,
    state: &RenderState,
) -> (f64, f64) {
    let mut all_items: Vec<&Item> = items.iter().collect();
    if let Some(else_items) = else_items {
        all_items.extend(else_items.iter());
    }

    // Convert Vec<&Item> to slice for find_involved_participants
    let items_slice: Vec<Item> = all_items.into_iter().cloned().collect();

    let (base_x1, base_x2) = if let Some((min_x, max_x)) = find_involved_participants(&items_slice, state) {
        let margin = state.config.block_margin + state.config.participant_width / 2.0 + 10.0;
        (min_x - margin, max_x + margin)
    } else {
        // Fallback to full width if no participants found
        (state.block_left(), state.block_right())
    };

    // Calculate minimum width needed for label
    // Pentagon width + gap + condition label width + right margin
    let pentagon_width = (kind.len() as f64 * 8.0 + 12.0).max(35.0);
    let label_char_width = 12.0; // Wider for CJK characters
    let condition_width = if label.is_empty() {
        0.0
    } else {
        label.chars().count() as f64 * label_char_width + 20.0 // [label] with brackets and margin
    };
    let min_label_width = pentagon_width + 8.0 + condition_width + 20.0; // Extra right margin

    // Ensure block is wide enough for the label
    let current_width = base_x2 - base_x1;
    if current_width < min_label_width {
        // Extend the right side to accommodate the label
        (base_x1, base_x1 + min_label_width)
    } else {
        (base_x1, base_x2)
    }
}

/// Pre-calculate block backgrounds by doing a dry run
fn collect_block_backgrounds(state: &mut RenderState, items: &[Item]) {
    for item in items {
        match item {
            Item::Message { text, from, to, arrow, .. } => {
                let is_self = from == to;
                let lines: Vec<&str> = text.split("\\n").collect();
                let line_height = state.config.font_size + 4.0;
                let extra_height = if lines.len() > 1 {
                    (lines.len() - 1) as f64 * line_height
                } else {
                    0.0
                };
                let delay_offset = arrow.delay.map(|d| d as f64 * 10.0).unwrap_or(0.0);

                if is_self {
                    state.current_y += state.config.row_height + extra_height;
                } else {
                    if lines.len() > 1 {
                        state.current_y += extra_height;
                    }
                    state.current_y += state.config.row_height + delay_offset;
                }
            }
            Item::Note { text, .. } => {
                let lines: Vec<&str> = text.split("\\n").collect();
                let line_height = state.config.font_size + 4.0;
                let note_height = state.config.note_padding * 2.0 + lines.len() as f64 * line_height;
                state.current_y += note_height.max(state.config.row_height) + 15.0;
            }
            Item::State { text, .. } => {
                let lines: Vec<&str> = text.split("\\n").collect();
                let line_height = state.config.font_size + 4.0;
                let box_height = state.config.note_padding * 2.0 + lines.len() as f64 * line_height;
                state.current_y += box_height.max(state.config.row_height) + 10.0;
            }
            Item::Ref { text, .. } => {
                let lines: Vec<&str> = text.split("\\n").collect();
                let line_height = state.config.font_size + 4.0;
                let box_height = state.config.note_padding * 2.0 + lines.len() as f64 * line_height;
                state.current_y += box_height.max(state.config.row_height) + 15.0;
            }
            Item::Description { text } => {
                let lines: Vec<&str> = text.split("\\n").collect();
                let line_height = state.config.font_size + 4.0;
                state.current_y += lines.len() as f64 * line_height + 10.0;
            }
            Item::Block { kind, label, items, else_items } => {
                let start_y = state.current_y;

                // Calculate bounds based on involved participants and label width
                let (x1, x2) = calculate_block_bounds_with_label(items, else_items.as_deref(), label, kind.as_str(), state);

                state.current_y += state.config.row_height * 1.0; // Match render_block header space
                collect_block_backgrounds(state, items);

                let else_y = if else_items.is_some() {
                    Some(state.current_y)
                } else {
                    None
                };

                if let Some(else_items) = else_items {
                    state.current_y += state.config.row_height * 0.5;
                    collect_block_backgrounds(state, else_items);
                }

                let end_y = state.current_y + state.config.row_height * 0.3;
                state.current_y = end_y + state.config.row_height * 0.5;

                // Collect this block's background
                state.add_block_background(x1, start_y, x2 - x1, end_y - start_y);
                // Collect this block's label for rendering above lifelines
                state.add_block_label(x1, start_y, end_y, x2, kind.as_str(), label, else_y);
            }
            _ => {}
        }
    }
}

/// Render all collected block backgrounds
fn render_block_backgrounds(svg: &mut String, state: &RenderState) {
    let theme = &state.config.theme;
    for bg in &state.block_backgrounds {
        writeln!(
            svg,
            r##"<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{fill}" stroke="none"/>"##,
            x = bg.x,
            y = bg.y,
            w = bg.width,
            h = bg.height,
            fill = theme.block_fill
        )
        .unwrap();
    }
}

/// Render all collected block labels (frame, pentagon, condition text, else divider)
/// This is called AFTER lifelines are drawn so labels appear on top
fn render_block_labels(svg: &mut String, state: &RenderState) {
    let theme = &state.config.theme;

    for bl in &state.block_labels {
        let x1 = bl.x1;
        let x2 = bl.x2;
        let start_y = bl.start_y;
        let end_y = bl.end_y;

        // Draw block frame
        writeln!(
            svg,
            r#"<rect x="{x}" y="{y}" width="{w}" height="{h}" class="block"/>"#,
            x = x1,
            y = start_y,
            w = x2 - x1,
            h = end_y - start_y
        )
        .unwrap();

        // Pentagon/tab-shaped label (WSD style)
        let label_text = &bl.kind;
        let label_width = (label_text.len() as f64 * 8.0 + 12.0).max(35.0);
        let label_height = 20.0;
        let notch_size = 8.0;

        // Pentagon path
        let pentagon_path = format!(
            "M {x1} {y1} L {x2} {y1} L {x2} {y2} L {x3} {y3} L {x1} {y3} Z",
            x1 = x1,
            y1 = start_y,
            x2 = x1 + label_width,
            y2 = start_y + label_height - notch_size,
            x3 = x1 + label_width - notch_size,
            y3 = start_y + label_height
        );

        writeln!(
            svg,
            r##"<path d="{path}" fill="{fill}" stroke="{stroke}"/>"##,
            path = pentagon_path,
            fill = theme.block_label_fill,
            stroke = theme.block_stroke
        )
        .unwrap();

        // Block type label text
        writeln!(
            svg,
            r#"<text x="{x}" y="{y}" class="block-label">{kind}</text>"#,
            x = x1 + 5.0,
            y = start_y + 14.0,
            kind = label_text
        )
        .unwrap();

        // Condition label (outside the pentagon)
        if !bl.label.is_empty() {
            writeln!(
                svg,
                r#"<text x="{x}" y="{y}" class="block-label">[{label}]</text>"#,
                x = x1 + label_width + 8.0,
                y = start_y + 14.0,
                label = escape_xml(&bl.label)
            )
            .unwrap();
        }

        // Else separator
        if let Some(else_y) = bl.else_y {
            writeln!(
                svg,
                r##"<line x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" stroke="{c}" stroke-dasharray="5,3"/>"##,
                x1 = x1,
                y = else_y,
                x2 = x2,
                c = theme.block_stroke
            )
            .unwrap();
            writeln!(
                svg,
                r#"<text x="{x}" y="{y}" class="block-label">[else]</text>"#,
                x = x1 + 4.0,
                y = else_y - 4.0
            )
            .unwrap();
        }
    }
}

/// Render a diagram to SVG
pub fn render(diagram: &Diagram) -> String {
    render_with_config(diagram, Config::default())
}

/// Render a diagram to SVG with custom config
pub fn render_with_config(diagram: &Diagram, config: Config) -> String {
    let participants = diagram.participants();
    let has_title = diagram.title.is_some();
    let footer_style = diagram.options.footer;
    let mut state = RenderState::new(config, participants, &diagram.items, has_title, footer_style);
    let mut svg = String::new();

    // Pre-calculate height
    let content_height = calculate_height(&diagram.items, &state.config);
    let title_space = if has_title { state.config.title_height } else { 0.0 };
    let total_height = state.config.padding * 2.0
        + title_space
        + state.config.header_height * 2.0  // header + footer
        + content_height
        + 40.0;  // spacing
    let total_width = state.diagram_width();

    // SVG header
    writeln!(
        &mut svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}">"#,
        w = total_width,
        h = total_height
    )
    .unwrap();

    // Styles based on theme
    let theme = &state.config.theme;
    let lifeline_dash = match theme.lifeline_style {
        LifelineStyle::Dashed => "stroke-dasharray: 5,5;",
        LifelineStyle::Solid => "",
    };

    svg.push_str("<defs>\n");
    svg.push_str("<style>\n");
    writeln!(
        &mut svg,
        ".participant {{ fill: {fill}; stroke: {stroke}; stroke-width: 2; }}",
        fill = theme.participant_fill,
        stroke = theme.participant_stroke
    )
    .unwrap();
    writeln!(
        &mut svg,
        ".participant-text {{ font-family: {f}; font-size: {s}px; text-anchor: middle; dominant-baseline: middle; fill: {c}; }}",
        f = theme.font_family,
        s = state.config.font_size,
        c = theme.participant_text
    )
    .unwrap();
    writeln!(
        &mut svg,
        ".lifeline {{ stroke: {c}; stroke-width: 1; {dash} }}",
        c = theme.lifeline_color,
        dash = lifeline_dash
    )
    .unwrap();
    writeln!(
        &mut svg,
        ".message {{ stroke: {c}; stroke-width: 1.5; fill: none; }}",
        c = theme.message_color
    )
    .unwrap();
    writeln!(
        &mut svg,
        ".message-dashed {{ stroke: {c}; stroke-width: 1.5; fill: none; stroke-dasharray: 5,3; }}",
        c = theme.message_color
    )
    .unwrap();
    writeln!(
        &mut svg,
        ".message-text {{ font-family: {f}; font-size: {s}px; fill: {c}; }}",
        f = theme.font_family,
        s = state.config.font_size,
        c = theme.message_text_color
    )
    .unwrap();
    writeln!(
        &mut svg,
        ".note {{ fill: {fill}; stroke: {stroke}; stroke-width: 1; }}",
        fill = theme.note_fill,
        stroke = theme.note_stroke
    )
    .unwrap();
    writeln!(
        &mut svg,
        ".note-text {{ font-family: {f}; font-size: {s}px; fill: {c}; }}",
        f = theme.font_family,
        s = state.config.font_size - 1.0,
        c = theme.note_text_color
    )
    .unwrap();
    writeln!(
        &mut svg,
        ".block {{ fill: none; stroke: {c}; stroke-width: 1; }}",
        c = theme.block_stroke
    )
    .unwrap();
    writeln!(
        &mut svg,
        ".block-label {{ font-family: {f}; font-size: {s}px; font-weight: bold; fill: {c}; }}",
        f = theme.font_family,
        s = state.config.font_size - 1.0,
        c = theme.message_text_color
    )
    .unwrap();
    writeln!(
        &mut svg,
        ".activation {{ fill: {fill}; stroke: {stroke}; stroke-width: 1; }}",
        fill = theme.activation_fill,
        stroke = theme.activation_stroke
    )
    .unwrap();
    writeln!(
        &mut svg,
        ".actor-head {{ fill: {fill}; stroke: {stroke}; stroke-width: 2; }}",
        fill = theme.actor_fill,
        stroke = theme.actor_stroke
    )
    .unwrap();
    writeln!(
        &mut svg,
        ".actor-body {{ stroke: {c}; stroke-width: 2; fill: none; }}",
        c = theme.actor_stroke
    )
    .unwrap();
    writeln!(
        &mut svg,
        ".title {{ font-family: {f}; font-size: {s}px; font-weight: bold; text-anchor: middle; fill: {c}; }}",
        f = theme.font_family,
        s = state.config.font_size + 4.0,
        c = theme.message_text_color
    )
    .unwrap();
    svg.push_str("</style>\n");

    // Arrow markers with theme color
    writeln!(
        &mut svg,
        r##"<marker id="arrow-filled" markerWidth="10" markerHeight="7" refX="9" refY="3.5" orient="auto">"##
    )
    .unwrap();
    writeln!(
        &mut svg,
        r##"<polygon points="0 0, 10 3.5, 0 7" fill="{c}"/>"##,
        c = theme.message_color
    )
    .unwrap();
    svg.push_str("</marker>\n");

    writeln!(
        &mut svg,
        r##"<marker id="arrow-open" markerWidth="10" markerHeight="7" refX="9" refY="3.5" orient="auto">"##
    )
    .unwrap();
    writeln!(
        &mut svg,
        r##"<polyline points="0 0, 10 3.5, 0 7" fill="none" stroke="{c}" stroke-width="1"/>"##,
        c = theme.message_color
    )
    .unwrap();
    svg.push_str("</marker>\n");

    svg.push_str("</defs>\n");

    // Background with theme color
    writeln!(
        &mut svg,
        r##"<rect width="100%" height="100%" fill="{bg}"/>"##,
        bg = theme.background
    )
    .unwrap();

    // Title
    if let Some(title) = &diagram.title {
        writeln!(
            &mut svg,
            r#"<text x="{x}" y="{y}" class="title">{t}</text>"#,
            x = total_width / 2.0,
            y = state.config.padding + state.config.title_height / 2.0 + 5.0,
            t = escape_xml(title)
        )
        .unwrap();
    }

    // Calculate footer position
    let header_y = state.header_top();
    let footer_y = total_height - state.config.padding - state.config.header_height;

    // Pre-calculate block backgrounds (dry run)
    state.current_y = state.content_start();
    collect_block_backgrounds(&mut state, &diagram.items);

    // Draw block backgrounds FIRST (behind lifelines)
    render_block_backgrounds(&mut svg, &state);

    // Reset current_y for actual rendering
    state.current_y = state.content_start();

    // Draw lifelines (behind messages but above block backgrounds)
    let lifeline_start = header_y + state.config.header_height;
    let lifeline_end = footer_y;

    for p in &state.participants {
        let x = state.get_x(p.id());
        writeln!(
            &mut svg,
            r#"<line x1="{x}" y1="{y1}" x2="{x}" y2="{y2}" class="lifeline"/>"#,
            x = x,
            y1 = lifeline_start,
            y2 = lifeline_end
        )
        .unwrap();
    }

    // Draw block labels AFTER lifelines so they appear on top
    render_block_labels(&mut svg, &state);

    // Draw participant headers
    render_participant_headers(&mut svg, &state, header_y);

    // Render items
    state.current_y = state.content_start();
    render_items(&mut svg, &mut state, &diagram.items);

    // Draw activation bars
    render_activations(&mut svg, &mut state, footer_y);

    // Draw participant footers based on footer style option
    match state.footer_style {
        FooterStyle::Box => {
            render_participant_headers(&mut svg, &state, footer_y);
        }
        FooterStyle::Bar => {
            // Draw simple horizontal line across all participants
            let left = state.leftmost_x() - state.get_participant_width(state.participants.first().map(|p| p.id()).unwrap_or("")) / 2.0;
            let right = state.rightmost_x() + state.get_participant_width(state.participants.last().map(|p| p.id()).unwrap_or("")) / 2.0;
            writeln!(
                &mut svg,
                r##"<line x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" stroke="{c}" stroke-width="1"/>"##,
                x1 = left,
                y = footer_y,
                x2 = right,
                c = state.config.theme.lifeline_color
            )
            .unwrap();
        }
        FooterStyle::None => {
            // No footer at all
        }
    }

    svg.push_str("</svg>\n");
    svg
}

fn calculate_height(items: &[Item], config: &Config) -> f64 {
    let mut height = 0.0;
    let line_height = config.font_size + 4.0;
    for item in items {
        match item {
            Item::Message { text, arrow, .. } => {
                let lines = text.split("\\n").count();
                let delay_offset = arrow.delay.map(|d| d as f64 * 10.0).unwrap_or(0.0);
                height += config.row_height + (lines.saturating_sub(1)) as f64 * line_height + delay_offset;
            }
            Item::Note { text, .. } => {
                let lines = text.split("\\n").count();
                height += config.row_height + (lines.saturating_sub(1)) as f64 * line_height + 15.0;
            }
            Item::State { text, .. } => {
                let lines = text.split("\\n").count();
                height += config.row_height + (lines.saturating_sub(1)) as f64 * line_height + 10.0;
            }
            Item::Ref { text, .. } => {
                let lines = text.split("\\n").count();
                height += config.row_height + (lines.saturating_sub(1)) as f64 * line_height + 15.0;
            }
            Item::Description { text } => {
                let lines = text.split("\\n").count();
                height += lines as f64 * line_height + 10.0;
            }
            Item::Block { items, else_items, .. } => {
                height += config.row_height * 1.0; // Block header margin (space for pentagon label + gap)
                height += calculate_height(items, config);
                if let Some(else_items) = else_items {
                    height += config.row_height * 0.5; // Else separator
                    height += calculate_height(else_items, config);
                }
                height += config.row_height * 0.8; // Block footer + margin after block
            }
            Item::Activate { .. } | Item::Deactivate { .. } | Item::Destroy { .. } => {}
            Item::ParticipantDecl { .. } => {}
            Item::Autonumber { .. } => {}
            Item::DiagramOption { .. } => {} // Options don't take space
        }
    }
    height
}

fn render_participant_headers(svg: &mut String, state: &RenderState, y: f64) {
    let shape = state.config.theme.participant_shape;

    for p in &state.participants {
        let x = state.get_x(p.id());
        let p_width = state.get_participant_width(p.id());
        let box_x = x - p_width / 2.0;

        match p.kind {
            ParticipantKind::Participant => {
                // Draw shape based on theme
                match shape {
                    ParticipantShape::Rectangle => {
                        writeln!(
                            svg,
                            r#"<rect x="{x}" y="{y}" width="{w}" height="{h}" class="participant"/>"#,
                            x = box_x,
                            y = y,
                            w = p_width,
                            h = state.config.header_height
                        )
                        .unwrap();
                    }
                    ParticipantShape::RoundedRect => {
                        writeln!(
                            svg,
                            r#"<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="8" ry="8" class="participant"/>"#,
                            x = box_x,
                            y = y,
                            w = p_width,
                            h = state.config.header_height
                        )
                        .unwrap();
                    }
                    ParticipantShape::Circle => {
                        // Draw ellipse/circle that fits in the header area
                        let rx = p_width / 2.0 - 5.0;
                        let ry = state.config.header_height / 2.0 - 2.0;
                        writeln!(
                            svg,
                            r#"<ellipse cx="{cx}" cy="{cy}" rx="{rx}" ry="{ry}" class="participant"/>"#,
                            cx = x,
                            cy = y + state.config.header_height / 2.0,
                            rx = rx,
                            ry = ry
                        )
                        .unwrap();
                    }
                }
                // Name centered in box (handle multiline with \n)
                let lines: Vec<&str> = p.name.split("\\n").collect();
                if lines.len() == 1 {
                    writeln!(
                        svg,
                        r#"<text x="{x}" y="{y}" class="participant-text">{name}</text>"#,
                        x = x,
                        y = y + state.config.header_height / 2.0 + 5.0,
                        name = escape_xml(&p.name)
                    )
                    .unwrap();
                } else {
                    let line_height = state.config.font_size + 2.0;
                    let total_height = lines.len() as f64 * line_height;
                    let start_y = y + state.config.header_height / 2.0 - total_height / 2.0 + line_height * 0.8;
                    write!(svg, r#"<text x="{x}" class="participant-text">"#, x = x).unwrap();
                    for (i, line) in lines.iter().enumerate() {
                        let dy = if i == 0 { start_y } else { line_height };
                        if i == 0 {
                            writeln!(
                                svg,
                                r#"<tspan x="{x}" y="{y}">{text}</tspan>"#,
                                x = x,
                                y = dy,
                                text = escape_xml(line)
                            )
                            .unwrap();
                        } else {
                            writeln!(
                                svg,
                                r#"<tspan x="{x}" dy="{dy}">{text}</tspan>"#,
                                x = x,
                                dy = dy,
                                text = escape_xml(line)
                            )
                            .unwrap();
                        }
                    }
                    writeln!(svg, "</text>").unwrap();
                }
            }
            ParticipantKind::Actor => {
                // Stick figure centered in header area
                let fig_center_y = y + state.config.header_height / 2.0;
                let head_r = 8.0;
                let body_len = 12.0;
                let arm_y = fig_center_y + 2.0;
                let arm_len = 10.0;
                let leg_len = 10.0;

                // Head
                writeln!(
                    svg,
                    r#"<circle cx="{x}" cy="{cy}" r="{r}" class="actor-head"/>"#,
                    x = x,
                    cy = fig_center_y - body_len / 2.0 - head_r,
                    r = head_r
                )
                .unwrap();
                // Body
                writeln!(
                    svg,
                    r#"<line x1="{x}" y1="{y1}" x2="{x}" y2="{y2}" class="actor-body"/>"#,
                    x = x,
                    y1 = fig_center_y - body_len / 2.0,
                    y2 = fig_center_y + body_len / 2.0
                )
                .unwrap();
                // Arms
                writeln!(
                    svg,
                    r#"<line x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" class="actor-body"/>"#,
                    x1 = x - arm_len,
                    y = arm_y,
                    x2 = x + arm_len
                )
                .unwrap();
                // Left leg
                writeln!(
                    svg,
                    r#"<line x1="{x}" y1="{y1}" x2="{x2}" y2="{y2}" class="actor-body"/>"#,
                    x = x,
                    y1 = fig_center_y + body_len / 2.0,
                    x2 = x - leg_len * 0.6,
                    y2 = fig_center_y + body_len / 2.0 + leg_len
                )
                .unwrap();
                // Right leg
                writeln!(
                    svg,
                    r#"<line x1="{x}" y1="{y1}" x2="{x2}" y2="{y2}" class="actor-body"/>"#,
                    x = x,
                    y1 = fig_center_y + body_len / 2.0,
                    x2 = x + leg_len * 0.6,
                    y2 = fig_center_y + body_len / 2.0 + leg_len
                )
                .unwrap();
                // Name below figure (with multiline support)
                let name_lines: Vec<&str> = p.name.split("\\n").collect();
                if name_lines.len() == 1 {
                    writeln!(
                        svg,
                        r#"<text x="{x}" y="{y}" class="participant-text">{name}</text>"#,
                        x = x,
                        y = y + state.config.header_height + 15.0,
                        name = escape_xml(&p.name)
                    )
                    .unwrap();
                } else {
                    // Multiline actor name using tspan
                    let line_height = 16.0;
                    let start_y = y + state.config.header_height + 15.0;
                    writeln!(
                        svg,
                        r#"<text x="{x}" class="participant-text">"#,
                        x = x
                    )
                    .unwrap();
                    for (i, line) in name_lines.iter().enumerate() {
                        if i == 0 {
                            writeln!(
                                svg,
                                r#"<tspan x="{x}" y="{y}">{text}</tspan>"#,
                                x = x,
                                y = start_y,
                                text = escape_xml(line)
                            )
                            .unwrap();
                        } else {
                            writeln!(
                                svg,
                                r#"<tspan x="{x}" dy="{dy}">{text}</tspan>"#,
                                x = x,
                                dy = line_height,
                                text = escape_xml(line)
                            )
                            .unwrap();
                        }
                    }
                    writeln!(svg, "</text>").unwrap();
                }
            }
        }
    }
}

fn render_items(svg: &mut String, state: &mut RenderState, items: &[Item]) {
    for item in items {
        match item {
            Item::Message {
                from,
                to,
                text,
                arrow,
                activate,
                deactivate,
                ..
            } => {
                render_message(svg, state, from, to, text, arrow, *activate, *deactivate);
            }
            Item::Note {
                position,
                participants,
                text,
            } => {
                render_note(svg, state, position, participants, text);
            }
            Item::Block {
                kind,
                label,
                items,
                else_items,
            } => {
                render_block(svg, state, kind, label, items, else_items.as_deref());
            }
            Item::Activate { participant } => {
                let y = state.current_y;
                state
                    .activations
                    .entry(participant.clone())
                    .or_default()
                    .push((y, None));
            }
            Item::Deactivate { participant } => {
                if let Some(acts) = state.activations.get_mut(participant) {
                    if let Some(act) = acts.last_mut() {
                        if act.1.is_none() {
                            act.1 = Some(state.current_y);
                        }
                    }
                }
            }
            Item::Destroy { participant } => {
                state.destroyed.insert(participant.clone(), state.current_y);
                // Draw X mark on the lifeline
                let x = state.get_x(participant);
                let y = state.current_y;
                let size = 12.0;
                let theme = &state.config.theme;
                writeln!(
                    svg,
                    r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{stroke}" stroke-width="2"/>"#,
                    x1 = x - size,
                    y1 = y - size,
                    x2 = x + size,
                    y2 = y + size,
                    stroke = theme.message_color
                )
                .unwrap();
                writeln!(
                    svg,
                    r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{stroke}" stroke-width="2"/>"#,
                    x1 = x + size,
                    y1 = y - size,
                    x2 = x - size,
                    y2 = y + size,
                    stroke = theme.message_color
                )
                .unwrap();
            }
            Item::Autonumber { enabled, start } => {
                if *enabled {
                    state.autonumber = Some(start.unwrap_or(1));
                } else {
                    state.autonumber = None;
                }
            }
            Item::ParticipantDecl { .. } => {
                // Already processed
            }
            Item::State { participants, text } => {
                render_state(svg, state, participants, text);
            }
            Item::Ref { participants, text, input_from, input_label, output_to, output_label } => {
                render_ref(svg, state, participants, text, input_from.as_deref(), input_label.as_deref(), output_to.as_deref(), output_label.as_deref());
            }
            Item::DiagramOption { .. } => {
                // Options are processed at render start, not during item rendering
            }
            Item::Description { text } => {
                render_description(svg, state, text);
            }
        }
    }
}

fn render_message(
    svg: &mut String,
    state: &mut RenderState,
    from: &str,
    to: &str,
    text: &str,
    arrow: &Arrow,
    activate: bool,
    deactivate: bool,
) {
    let x1 = state.get_x(from);
    let x2 = state.get_x(to);

    let is_self = from == to;
    let line_class = match arrow.line {
        LineStyle::Solid => "message",
        LineStyle::Dashed => "message-dashed",
    };
    let marker = match arrow.head {
        ArrowHead::Filled => "url(#arrow-filled)",
        ArrowHead::Open => "url(#arrow-open)",
    };

    // Get autonumber prefix
    let num_prefix = state
        .next_number()
        .map(|n| format!("{}. ", n))
        .unwrap_or_default();

    // Calculate text lines and height
    let display_text = format!("{}{}", num_prefix, text);
    let lines: Vec<&str> = display_text.split("\\n").collect();
    let line_height = state.config.font_size + 4.0;
    let extra_height = if lines.len() > 1 {
        (lines.len() - 1) as f64 * line_height
    } else {
        0.0
    };

    // Add space BEFORE the message for multiline text (text is rendered above arrow)
    if !is_self && lines.len() > 1 {
        state.current_y += extra_height;
    }

    let y = state.current_y;

    if is_self {
        // Self message - loop back
        let loop_width = 40.0;
        let text_block_height = lines.len() as f64 * line_height;
        let loop_height = (text_block_height + 10.0).max(25.0);
        writeln!(
            svg,
            r#"<path d="M {x1} {y} L {x2} {y} L {x2} {y2} L {x1} {y2}" class="{cls}" marker-end="{marker}"/>"#,
            x1 = x1,
            y = y,
            x2 = x1 + loop_width,
            y2 = y + loop_height,
            cls = line_class,
            marker = marker
        )
        .unwrap();

        // Text - multiline support
        for (i, line) in lines.iter().enumerate() {
            let line_y = y + 4.0 + (i as f64 + 0.5) * line_height;
            writeln!(
                svg,
                r#"<text x="{x}" y="{y}" class="message-text">{t}</text>"#,
                x = x1 + loop_width + 5.0,
                y = line_y,
                t = escape_xml(line)
            )
            .unwrap();
        }

        state.current_y += state.config.row_height + extra_height;
    } else {
        // Regular message - check for delay
        let delay_offset = arrow.delay.map(|d| d as f64 * 10.0).unwrap_or(0.0);
        let y2 = y + delay_offset;

        let text_x = (x1 + x2) / 2.0;
        let text_y = (y + y2) / 2.0 - 8.0;

        // Draw arrow line (slanted if delay)
        writeln!(
            svg,
            r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" class="{cls}" marker-end="{marker}"/>"#,
            x1 = x1,
            y1 = y,
            x2 = x2,
            y2 = y2,
            cls = line_class,
            marker = marker
        )
        .unwrap();

        // Text with multiline support (positioned at midpoint of slanted line)
        for (i, line) in lines.iter().enumerate() {
            let line_y = text_y - (lines.len() - 1 - i) as f64 * line_height;
            writeln!(
                svg,
                r#"<text x="{x}" y="{y}" class="message-text" text-anchor="middle">{t}</text>"#,
                x = text_x,
                y = line_y,
                t = escape_xml(line)
            )
            .unwrap();
        }

        // Add row_height plus delay offset
        state.current_y += state.config.row_height + delay_offset;
    }

    // Handle activation
    if activate {
        state
            .activations
            .entry(to.to_string())
            .or_default()
            .push((y, None));
    }
    if deactivate {
        if let Some(acts) = state.activations.get_mut(from) {
            if let Some(act) = acts.last_mut() {
                if act.1.is_none() {
                    act.1 = Some(y);
                }
            }
        }
    }
}

fn render_note(
    svg: &mut String,
    state: &mut RenderState,
    position: &NotePosition,
    participants: &[String],
    text: &str,
) {
    let lines: Vec<&str> = text.split("\\n").collect();
    let line_height = state.config.font_size + 4.0;
    let note_height = state.config.note_padding * 2.0 + lines.len() as f64 * line_height;

    // Calculate note width based on content or participant span
    // Use 12.0 per char for CJK text estimation (wider than ASCII)
    let max_line_len = lines.iter().map(|l| l.chars().count()).max().unwrap_or(10);
    let content_width = (max_line_len as f64 * 10.0 + state.config.note_padding * 2.0).max(80.0);

    let (x, note_width, text_anchor) = match position {
        NotePosition::Left => {
            let px = state.get_x(&participants[0]);
            let p_width = state.get_participant_width(&participants[0]);
            let w = content_width.min(300.0);
            // Clamp to not go off left edge
            let x = (px - p_width / 2.0 - w - 10.0).max(state.config.padding);
            (x, w, "start")
        }
        NotePosition::Right => {
            let px = state.get_x(&participants[0]);
            let p_width = state.get_participant_width(&participants[0]);
            let w = content_width.min(300.0);
            (px + p_width / 2.0 + 10.0, w, "start")
        }
        NotePosition::Over => {
            if participants.len() == 1 {
                let px = state.get_x(&participants[0]);
                // Allow wider notes for single participant
                let w = content_width;
                // Clamp to stay within diagram
                let x = (px - w / 2.0).max(state.config.padding);
                (x, w, "middle")
            } else {
                // Span across multiple participants
                let x1 = state.get_x(&participants[0]);
                let x2 = state.get_x(participants.last().unwrap());
                let p1_width = state.get_participant_width(&participants[0]);
                let p2_width = state.get_participant_width(participants.last().unwrap());
                let span_width = (x2 - x1).abs() + (p1_width + p2_width) / 2.0 * 0.8;
                let w = span_width.max(content_width);
                let center = (x1 + x2) / 2.0;
                // Clamp x to stay within padding
                let x = (center - w / 2.0).max(state.config.padding);
                (x, w, "middle")
            }
        }
    };

    let y = state.current_y;
    let fold_size = 8.0; // Size of the dog-ear fold

    // Note background with dog-ear (folded corner) effect
    // Path: start at top-left, go right (leaving space for fold), diagonal fold, down, left, up
    let note_path = format!(
        "M {x} {y} L {x2} {y} L {x3} {y2} L {x3} {y3} L {x} {y3} Z",
        x = x,
        y = y,
        x2 = x + note_width - fold_size,
        x3 = x + note_width,
        y2 = y + fold_size,
        y3 = y + note_height
    );

    writeln!(
        svg,
        r#"<path d="{path}" class="note"/>"#,
        path = note_path
    )
    .unwrap();

    // Draw the fold triangle (represents the folded corner)
    let theme = &state.config.theme;
    // Triangle: from fold start, to diagonal corner, to bottom of fold
    let fold_path = format!(
        "M {x1} {y1} L {x2} {y2} L {x1} {y2} Z",
        x1 = x + note_width - fold_size,
        y1 = y,
        x2 = x + note_width,
        y2 = y + fold_size
    );

    writeln!(
        svg,
        r##"<path d="{path}" fill="{fill}" stroke="{stroke}" stroke-width="1"/>"##,
        path = fold_path,
        fill = "#e0e0a0",  // Slightly darker yellow for fold
        stroke = theme.note_stroke
    )
    .unwrap();

    // Note text
    let text_x = match text_anchor {
        "middle" => x + note_width / 2.0,
        "start" => x + state.config.note_padding,
        _ => x + note_width - state.config.note_padding,
    };

    for (i, line) in lines.iter().enumerate() {
        let text_y = y + state.config.note_padding + (i as f64 + 0.8) * line_height;
        writeln!(
            svg,
            r#"<text x="{x}" y="{y}" class="note-text" text-anchor="{anchor}">{t}</text>"#,
            x = text_x,
            y = text_y,
            anchor = if *position == NotePosition::Over { "middle" } else { "start" },
            t = escape_xml(line)
        )
        .unwrap();
    }

    // Add note height plus margin
    state.current_y += note_height.max(state.config.row_height) + 15.0;
}

/// Render a state box (rounded rectangle)
fn render_state(
    svg: &mut String,
    state: &mut RenderState,
    participants: &[String],
    text: &str,
) {
    let theme = &state.config.theme;
    let lines: Vec<&str> = text.split("\\n").collect();
    let line_height = state.config.font_size + 4.0;
    let box_height = state.config.note_padding * 2.0 + lines.len() as f64 * line_height;

    // Calculate box position and width
    let (x, box_width) = if participants.len() == 1 {
        let px = state.get_x(&participants[0]);
        let max_line_len = lines.iter().map(|l| l.chars().count()).max().unwrap_or(8);
        let w = (max_line_len as f64 * 8.0 + state.config.note_padding * 2.0).max(60.0);
        (px - w / 2.0, w)
    } else {
        let x1 = state.get_x(&participants[0]);
        let x2 = state.get_x(participants.last().unwrap());
        let span_width = (x2 - x1).abs() + state.config.participant_width * 0.6;
        let center = (x1 + x2) / 2.0;
        (center - span_width / 2.0, span_width)
    };

    let y = state.current_y;

    // Draw rounded rectangle
    writeln!(
        svg,
        r##"<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="8" ry="8" fill="{fill}" stroke="{stroke}" stroke-width="1.5"/>"##,
        x = x,
        y = y,
        w = box_width,
        h = box_height,
        fill = theme.state_fill,
        stroke = theme.state_stroke
    )
    .unwrap();

    // Draw text
    let text_x = x + box_width / 2.0;
    for (i, line) in lines.iter().enumerate() {
        let text_y = y + state.config.note_padding + (i as f64 + 0.8) * line_height;
        writeln!(
            svg,
            r##"<text x="{x}" y="{y}" text-anchor="middle" fill="{fill}" font-family="{font}" font-size="{size}px">{t}</text>"##,
            x = text_x,
            y = text_y,
            fill = theme.state_text_color,
            font = theme.font_family,
            size = state.config.font_size,
            t = escape_xml(line)
        )
        .unwrap();
    }

    state.current_y += box_height.max(state.config.row_height) + 10.0;
}

/// Render a ref box (hexagon-like shape)
fn render_ref(
    svg: &mut String,
    state: &mut RenderState,
    participants: &[String],
    text: &str,
    input_from: Option<&str>,
    input_label: Option<&str>,
    output_to: Option<&str>,
    output_label: Option<&str>,
) {
    let theme = &state.config.theme;
    let lines: Vec<&str> = text.split("\\n").collect();
    let line_height = state.config.font_size + 4.0;
    let box_height = state.config.note_padding * 2.0 + lines.len() as f64 * line_height;
    let notch_size = 10.0;

    // Calculate box position and width
    let (x, box_width) = if participants.len() == 1 {
        let px = state.get_x(&participants[0]);
        let max_line_len = lines.iter().map(|l| l.chars().count()).max().unwrap_or(15);
        let w = (max_line_len as f64 * 8.0 + state.config.note_padding * 2.0 + notch_size * 2.0).max(100.0);
        (px - w / 2.0, w)
    } else {
        let x1 = state.get_x(&participants[0]);
        let x2 = state.get_x(participants.last().unwrap());
        let span_width = (x2 - x1).abs() + state.config.participant_width * 0.8;
        let center = (x1 + x2) / 2.0;
        (center - span_width / 2.0, span_width)
    };

    let y = state.current_y;

    // Draw input signal arrow if present
    if let Some(from) = input_from {
        let from_x = state.get_x(from);
        let to_x = x; // Left edge of ref box
        let arrow_y = y + box_height / 2.0;

        // Draw arrow line
        writeln!(
            svg,
            r##"<line x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" class="message" marker-end="url(#arrow-filled)"/>"##,
            x1 = from_x,
            y = arrow_y,
            x2 = to_x
        )
        .unwrap();

        // Draw label if present
        if let Some(label) = input_label {
            let text_x = (from_x + to_x) / 2.0;
            writeln!(
                svg,
                r##"<text x="{x}" y="{y}" class="message-text" text-anchor="middle">{t}</text>"##,
                x = text_x,
                y = arrow_y - 8.0,
                t = escape_xml(label)
            )
            .unwrap();
        }
    }

    // Draw hexagon-like shape (ref box in WSD style)
    // Left side has a notch cut
    let ref_path = format!(
        "M {x1} {y1} L {x2} {y1} L {x2} {y2} L {x1} {y2} L {x3} {y3} Z",
        x1 = x + notch_size,
        y1 = y,
        x2 = x + box_width,
        y2 = y + box_height,
        x3 = x,
        y3 = y + box_height / 2.0
    );

    writeln!(
        svg,
        r##"<path d="{path}" fill="{fill}" stroke="{stroke}" stroke-width="1.5"/>"##,
        path = ref_path,
        fill = theme.ref_fill,
        stroke = theme.ref_stroke
    )
    .unwrap();

    // Add "ref" label in top-left
    writeln!(
        svg,
        r##"<text x="{x}" y="{y}" fill="{fill}" font-family="{font}" font-size="{size}px" font-weight="bold">ref</text>"##,
        x = x + notch_size + 4.0,
        y = y + state.config.font_size,
        fill = theme.ref_text_color,
        font = theme.font_family,
        size = state.config.font_size - 2.0
    )
    .unwrap();

    // Draw text centered
    let text_x = x + box_width / 2.0;
    for (i, line) in lines.iter().enumerate() {
        let text_y = y + state.config.note_padding + (i as f64 + 0.8) * line_height;
        writeln!(
            svg,
            r##"<text x="{x}" y="{y}" text-anchor="middle" fill="{fill}" font-family="{font}" font-size="{size}px">{t}</text>"##,
            x = text_x,
            y = text_y,
            fill = theme.ref_text_color,
            font = theme.font_family,
            size = state.config.font_size,
            t = escape_xml(line)
        )
        .unwrap();
    }

    // Draw output signal arrow if present
    if let Some(to) = output_to {
        let from_x = x + box_width; // Right edge of ref box
        let to_x = state.get_x(to);
        let arrow_y = y + box_height;

        // Draw dashed arrow line (response style)
        writeln!(
            svg,
            r##"<line x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" class="message-dashed" marker-end="url(#arrow-filled)"/>"##,
            x1 = from_x,
            y = arrow_y,
            x2 = to_x
        )
        .unwrap();

        // Draw label if present
        if let Some(label) = output_label {
            let text_x = (from_x + to_x) / 2.0;
            writeln!(
                svg,
                r##"<text x="{x}" y="{y}" class="message-text" text-anchor="middle">{t}</text>"##,
                x = text_x,
                y = arrow_y - 8.0,
                t = escape_xml(label)
            )
            .unwrap();
        }
    }

    state.current_y += box_height.max(state.config.row_height) + 15.0;
}

/// Render a description (extended text explanation)
fn render_description(
    svg: &mut String,
    state: &mut RenderState,
    text: &str,
) {
    let theme = &state.config.theme;
    let lines: Vec<&str> = text.split("\\n").collect();
    let line_height = state.config.font_size + 4.0;

    // Draw text on the left side of the diagram
    let x = state.config.padding + 10.0;
    let y = state.current_y;

    for (i, line) in lines.iter().enumerate() {
        let text_y = y + (i as f64 + 0.8) * line_height;
        writeln!(
            svg,
            r##"<text x="{x}" y="{y}" fill="{fill}" font-family="{font}" font-size="{size}px" font-style="italic">{t}</text>"##,
            x = x,
            y = text_y,
            fill = theme.description_text_color,
            font = theme.font_family,
            size = state.config.font_size - 1.0,
            t = escape_xml(line)
        )
        .unwrap();
    }

    state.current_y += lines.len() as f64 * line_height + 10.0;
}

fn render_block(
    svg: &mut String,
    state: &mut RenderState,
    _kind: &BlockKind,
    _label: &str,
    items: &[Item],
    else_items: Option<&[Item]>,
) {
    // Note: Block frame, labels, and else separators are rendered by render_block_labels()
    // This function only handles Y position tracking and rendering of inner items
    // svg is still used for rendering inner items via render_items()

    // Block header space (must be larger than pentagon label height of 20px + margin)
    state.current_y += state.config.row_height * 1.0;

    // Render items
    render_items(svg, state, items);

    // Render else items if present
    if let Some(else_items) = else_items {
        state.current_y += state.config.row_height * 0.5;
        render_items(svg, state, else_items);
    }

    let end_y = state.current_y + state.config.row_height * 0.3;

    // Set current_y to end of block + margin
    state.current_y = end_y + state.config.row_height * 0.5;

    // Block frame, labels, and else separators are rendered earlier by render_block_labels()
    // which is called after lifelines are drawn, so labels appear on top of lifelines
}

fn render_activations(svg: &mut String, state: &mut RenderState, footer_y: f64) {
    for (participant, activations) in &state.activations {
        let x = state.get_x(participant);
        let box_x = x - state.config.activation_width / 2.0;

        for (start_y, end_y) in activations {
            // If no end_y, extend to footer
            let end = end_y.unwrap_or(footer_y);
            let height = end - start_y;

            if height > 0.0 {
                writeln!(
                    svg,
                    r#"<rect x="{x}" y="{y}" width="{w}" height="{h}" class="activation"/>"#,
                    x = box_x,
                    y = start_y,
                    w = state.config.activation_width,
                    h = height
                )
                .unwrap();
            }
        }
    }
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    #[test]
    fn test_render_simple() {
        let diagram = parse("Alice->Bob: Hello").unwrap();
        let svg = render(&diagram);
        assert!(svg.contains("<svg"));
        assert!(svg.contains("Alice"));
        assert!(svg.contains("Bob"));
        assert!(svg.contains("Hello"));
    }

    #[test]
    fn test_render_with_note() {
        let diagram = parse("Alice->Bob: Hello\nnote over Alice: Thinking").unwrap();
        let svg = render(&diagram);
        assert!(svg.contains("Thinking"));
    }
}
