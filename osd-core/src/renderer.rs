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
    /// Left margin for diagram content
    pub left_margin: f64,
    /// Right margin for diagram content
    pub right_margin: f64,
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
            padding: 10.5,
            left_margin: 122.5,
            right_margin: 15.0,
            participant_gap: 200.0,
            header_height: 108.0,
            row_height: 32.0,
            participant_width: 90.0,
            font_size: 12.0,
            activation_width: 10.0,
            note_padding: 6.0,
            block_margin: 5.0,
            title_height: 100.0,
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

/// Block label info for deferred rendering (rendered above activations/lifelines)
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
    /// Collected block labels for deferred rendering (above activations/lifelines)
    block_labels: Vec<BlockLabel>,
    /// Footer style from diagram options
    footer_style: FooterStyle,
    /// Tracks whether the first return message in an else branch needs extra spacing
    else_return_pending: Vec<bool>,
    /// Tracks whether a serial block needs extra spacing after its first row
    serial_first_row_pending: Vec<bool>,
    /// Tracks nested parallel depth for serial row spacing
    parallel_depth: usize,
}

const TEXT_WIDTH_PADDING: f64 = 41.0;
const TEXT_WIDTH_SCALE: f64 = 1.3;
const MESSAGE_WIDTH_PADDING: f64 = 8.0;
const MESSAGE_WIDTH_SCALE: f64 = 0.9;
const DELAY_UNIT: f64 = 18.0;
const BLOCK_LABEL_HEIGHT: f64 = 24.0;
const BLOCK_FOOTER_PADDING_LEVEL1: f64 = 0.90625;
const BLOCK_FOOTER_PADDING_DEEP: f64 = 0.90625;
const BLOCK_FOOTER_PADDING_TOP_FACTOR: f64 = 1.28125;
const BLOCK_ELSE_SPACING_LEVEL1: f64 = 1.1875;
const BLOCK_ELSE_SPACING_DEEP: f64 = 1.15625;
const BLOCK_ELSE_TOP_SPACING_FACTOR: f64 = 1.15625;
const BLOCK_NESTED_HEADER_ADJUST: f64 = 18.0;
const BLOCK_NESTED_FRAME_SHIFT: f64 = 18.0;
const PARALLEL_BLOCK_GAP: f64 = 22.0;
const MESSAGE_SPACING_MULT: f64 = 0.5625;
const SELF_MESSAGE_MIN_SPACING: f64 = 78.0;
const SELF_MESSAGE_GAP: f64 = 4.0;
const CREATE_MESSAGE_SPACING: f64 = 41.0;
const DESTROY_SPACING: f64 = 15.0;
const NOTE_PADDING: f64 = 9.5;
const NOTE_LINE_HEIGHT_EXTRA: f64 = 6.0;
const NOTE_MARGIN: f64 = 12.6;
const STATE_LINE_HEIGHT_EXTRA: f64 = 11.0;
const REF_LINE_HEIGHT_EXTRA: f64 = 16.333333;
const ELSE_RETURN_GAP: f64 = 1.0;
const SERIAL_FIRST_ROW_GAP: f64 = 0.0;
const SERIAL_FIRST_ROW_PARALLEL_GAP: f64 = 1.0;
const SERIAL_SELF_MESSAGE_ADJUST: f64 = 1.0;
const ACTIVATION_START_GAP: f64 = 0.0;
const ACTIVATION_CHAIN_GAP: f64 = 1.0;
const REF_EXTRA_GAP: f64 = 2.5;
const SELF_MESSAGE_ACTIVE_ADJUST: f64 = 1.0;
const STATE_EXTRA_GAP: f64 = 0.0;

fn block_header_space(config: &Config, depth: usize) -> f64 {
    let base = config.row_height + BLOCK_LABEL_HEIGHT;
    if depth == 0 {
        base
    } else {
        (base - BLOCK_NESTED_HEADER_ADJUST).max(BLOCK_LABEL_HEIGHT)
    }
}

fn block_frame_shift(depth: usize) -> f64 {
    if depth == 0 {
        0.0
    } else if depth == 1 {
        BLOCK_NESTED_FRAME_SHIFT
    } else {
        BLOCK_NESTED_FRAME_SHIFT
    }
}

fn block_footer_padding(config: &Config, depth: usize) -> f64 {
    let factor = if depth == 0 {
        BLOCK_FOOTER_PADDING_TOP_FACTOR
    } else if depth == 1 {
        BLOCK_FOOTER_PADDING_LEVEL1
    } else {
        BLOCK_FOOTER_PADDING_DEEP
    };
    config.row_height * factor
}

fn block_else_spacing(config: &Config, depth: usize) -> f64 {
    if depth == 0 {
        config.row_height * BLOCK_ELSE_TOP_SPACING_FACTOR
    } else if depth == 1 {
        config.row_height * BLOCK_ELSE_SPACING_LEVEL1
    } else {
        config.row_height * BLOCK_ELSE_SPACING_DEEP
    }
}

fn message_spacing_line_height(config: &Config) -> f64 {
    config.row_height * MESSAGE_SPACING_MULT
}

fn self_message_spacing(config: &Config, lines: usize) -> f64 {
    let line_height = config.font_size + 4.0;
    let text_block_height = lines as f64 * line_height;
    let loop_height = (text_block_height + 10.0).max(25.0);
    let base = loop_height + SELF_MESSAGE_GAP;
    if lines >= 3 {
        base.max(SELF_MESSAGE_MIN_SPACING)
    } else {
        base
    }
}

fn note_line_height(config: &Config) -> f64 {
    config.font_size + NOTE_LINE_HEIGHT_EXTRA
}

fn note_padding(_config: &Config) -> f64 {
    NOTE_PADDING
}

fn item_pre_gap(config: &Config) -> f64 {
    config.font_size + 1.0
}

fn item_pre_shift(config: &Config) -> f64 {
    (config.row_height - item_pre_gap(config)).max(0.0)
}

fn serial_first_row_gap(parallel_depth: usize) -> f64 {
    if parallel_depth > 0 {
        SERIAL_FIRST_ROW_PARALLEL_GAP
    } else {
        SERIAL_FIRST_ROW_GAP
    }
}

fn state_line_height(config: &Config) -> f64 {
    config.font_size + STATE_LINE_HEIGHT_EXTRA
}

fn ref_line_height(config: &Config) -> f64 {
    config.font_size + REF_LINE_HEIGHT_EXTRA
}

fn block_has_frame(kind: &BlockKind) -> bool {
    !matches!(kind, BlockKind::Parallel | BlockKind::Serial)
}

fn block_is_parallel(kind: &BlockKind) -> bool {
    matches!(kind, BlockKind::Parallel)
}

fn parallel_needs_gap(items: &[Item]) -> bool {
    items.iter().any(|item| matches!(item, Item::Block { .. }))
}

fn text_char_weight(c: char) -> f64 {
    if c.is_ascii() {
        if c.is_uppercase() { 0.7 } else { 0.5 }
    } else {
        1.0 // CJK and other characters are wider
    }
}

fn max_weighted_line(text: &str) -> f64 {
    text.split("\\n")
        .map(|line| line.chars().map(text_char_weight).sum::<f64>())
        .fold(0.0_f64, |a, b| a.max(b))
}

/// Estimate text width in pixels (rough approximation)
fn estimate_text_width(text: &str, font_size: f64) -> f64 {
    let weighted = max_weighted_line(text);
    weighted * font_size * TEXT_WIDTH_SCALE + TEXT_WIDTH_PADDING
}

fn estimate_message_width(text: &str, font_size: f64) -> f64 {
    let weighted = max_weighted_line(text);
    weighted * font_size * MESSAGE_WIDTH_SCALE + MESSAGE_WIDTH_PADDING
}

fn block_tab_width(kind: &str) -> f64 {
    (kind.chars().count() as f64 * 12.0 + 21.0).max(57.0)
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

                            let text_width = estimate_message_width(text, config.font_size);

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
        let name1_width = estimate_message_width(&participants[i].name, config.font_size);
        let name2_width = estimate_message_width(&participants[i + 1].name, config.font_size);
        let needed_for_names = (name1_width + name2_width) / 2.0 + 20.0;
        if needed_for_names > gaps[i] {
            gaps[i] = needed_for_names;
        }
    }

    // Cap maximum gap
    let max_gap = config.participant_gap * 3.0;
    for gap in &mut gaps {
        if *gap > max_gap {
            *gap = max_gap;
        }
    }

    gaps
}

impl RenderState {
    fn new(config: Config, participants: Vec<Participant>, items: &[Item], has_title: bool, footer_style: FooterStyle) -> Self {
        let mut config = config;
        let line_height = config.font_size + 2.0;
        let mut required_header_height = config.header_height;
        for p in &participants {
            let lines = p.name.split("\\n").count();
            let total_height = lines as f64 * line_height;
            let needed = total_height + 10.0;
            if needed > required_header_height {
                required_header_height = needed;
            }
        }
        if required_header_height > config.header_height {
            config.header_height = required_header_height;
        }
        // Calculate individual participant widths based on their names
        let mut participant_widths: HashMap<String, f64> = HashMap::new();
        let min_width = config.participant_width;

        for p in &participants {
            let text_width = estimate_text_width(&p.name, config.font_size);
            let width = (text_width + 8.0).max(min_width);
            participant_widths.insert(p.id().to_string(), width);
        }

        let gaps = calculate_participant_gaps(&participants, items, &config);

        // Left margin for notes/actions on leftmost participant
        let left_margin = config.left_margin;
        // Right margin for self-loops and notes on rightmost participant
        let right_margin = config.right_margin;

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
            else_return_pending: Vec::new(),
            serial_first_row_pending: Vec::new(),
            parallel_depth: 0,
        }
    }

    fn get_participant_width(&self, name: &str) -> f64 {
        *self.participant_widths.get(name).unwrap_or(&self.config.participant_width)
    }

    fn get_x(&self, name: &str) -> f64 {
        *self.participant_x.get(name).unwrap_or(&0.0)
    }

    fn push_else_return_pending(&mut self) {
        self.else_return_pending.push(true);
    }

    fn pop_else_return_pending(&mut self) {
        self.else_return_pending.pop();
    }

    fn apply_else_return_gap(&mut self, arrow: &Arrow) {
        if let Some(pending) = self.else_return_pending.last_mut() {
            if *pending && matches!(arrow.line, LineStyle::Dashed) {
                self.current_y += ELSE_RETURN_GAP;
                *pending = false;
            }
        }
    }

    fn push_serial_first_row_pending(&mut self) {
        self.serial_first_row_pending.push(true);
    }

    fn pop_serial_first_row_pending(&mut self) {
        self.serial_first_row_pending.pop();
    }

    fn in_serial_block(&self) -> bool {
        !self.serial_first_row_pending.is_empty()
    }

    fn apply_serial_first_row_gap(&mut self) {
        if let Some(pending) = self.serial_first_row_pending.last_mut() {
            if *pending {
                self.current_y += serial_first_row_gap(self.parallel_depth);
                *pending = false;
            }
        }
    }

    fn push_parallel(&mut self) {
        self.parallel_depth += 1;
    }

    fn pop_parallel(&mut self) {
        if self.parallel_depth > 0 {
            self.parallel_depth -= 1;
        }
    }

    fn active_activation_count(&self) -> usize {
        self.activations
            .values()
            .map(|acts| acts.iter().filter(|(_, end)| end.is_none()).count())
            .sum()
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
        self.header_top() + self.config.header_height + self.config.row_height
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

    /// Add a block label to be rendered later (above activations/lifelines)
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

/// Find participants involved in a list of items (returns min/max edges and whether leftmost is included)
fn find_involved_participants(items: &[Item], state: &RenderState) -> Option<(f64, f64, bool)> {
    let mut min_left: Option<f64> = None;
    let mut max_right: Option<f64> = None;
    let leftmost_id = state.participants.first().map(|p| p.id()).unwrap_or("");
    let mut includes_leftmost = false;

    fn update_bounds(
        participant: &str,
        state: &RenderState,
        min_left: &mut Option<f64>,
        max_right: &mut Option<f64>,
        includes_leftmost: &mut bool,
        leftmost_id: &str,
    ) {
        let x = state.get_x(participant);
        if x > 0.0 {
            let width = state.get_participant_width(participant);
            let left = x - width / 2.0;
            let right = x + width / 2.0;
            *min_left = Some(min_left.map_or(left, |m| m.min(left)));
            *max_right = Some(max_right.map_or(right, |m| m.max(right)));
            if participant == leftmost_id {
                *includes_leftmost = true;
            }
        }
    }

    fn process_items(
        items: &[Item],
        state: &RenderState,
        min_left: &mut Option<f64>,
        max_right: &mut Option<f64>,
        includes_leftmost: &mut bool,
        leftmost_id: &str,
    ) {
        for item in items {
            match item {
                Item::Message { from, to, .. } => {
                    update_bounds(from, state, min_left, max_right, includes_leftmost, leftmost_id);
                    update_bounds(to, state, min_left, max_right, includes_leftmost, leftmost_id);
                }
                Item::Note { participants, .. } => {
                    for p in participants {
                        update_bounds(p, state, min_left, max_right, includes_leftmost, leftmost_id);
                    }
                }
                Item::Block { items, else_items, .. } => {
                    process_items(items, state, min_left, max_right, includes_leftmost, leftmost_id);
                    if let Some(else_items) = else_items {
                        process_items(else_items, state, min_left, max_right, includes_leftmost, leftmost_id);
                    }
                }
                Item::Activate { participant } | Item::Deactivate { participant } | Item::Destroy { participant } => {
                    update_bounds(participant, state, min_left, max_right, includes_leftmost, leftmost_id);
                }
                _ => {}
            }
        }
    }

    process_items(items, state, &mut min_left, &mut max_right, &mut includes_leftmost, leftmost_id);

    match (min_left, max_right) {
        (Some(min), Some(max)) => Some((min, max, includes_leftmost)),
        _ => None,
    }
}

/// Calculate block x boundaries based on involved participants and label length
fn calculate_block_bounds_with_label(
    items: &[Item],
    else_items: Option<&[Item]>,
    label: &str,
    kind: &str,
    depth: usize,
    state: &RenderState,
) -> (f64, f64) {
    let mut all_items: Vec<&Item> = items.iter().collect();
    if let Some(else_items) = else_items {
        all_items.extend(else_items.iter());
    }

    // Convert Vec<&Item> to slice for find_involved_participants
    let items_slice: Vec<Item> = all_items.into_iter().cloned().collect();

    let (base_x1, base_x2, includes_leftmost) = if let Some((min_left, max_right, includes_leftmost)) =
        find_involved_participants(&items_slice, state)
    {
        let margin = state.config.block_margin;
        (min_left - margin, max_right + margin, includes_leftmost)
    } else {
        // Fallback to full width if no participants found
        (state.block_left(), state.block_right(), false)
    };

    // Calculate minimum width needed for label
    // Pentagon width + gap + condition label width + right margin
    let pentagon_width = block_tab_width(kind);
    let label_font_size = state.config.font_size - 1.0;
    let label_padding_x = 6.0;
    let condition_width = if label.is_empty() {
        0.0
    } else {
        let condition_text = format!("[{}]", label);
        let base_width = (estimate_text_width(&condition_text, label_font_size) - TEXT_WIDTH_PADDING).max(0.0);
        base_width + label_padding_x * 2.0
    };
    let min_label_width = pentagon_width + 8.0 + condition_width + 20.0; // Extra right margin

    // Ensure block is wide enough for the label
    let current_width = base_x2 - base_x1;
    let (mut x1, mut x2) = if current_width < min_label_width {
        // Extend the right side to accommodate the label
        (base_x1, base_x1 + min_label_width)
    } else {
        (base_x1, base_x2)
    };

    // Inset nested blocks so they sit inside their parent with padding.
    let nested_padding = depth as f64 * 20.0;
    if nested_padding > 0.0 {
        let available = x2 - x1;
        let max_padding = ((available - min_label_width) / 2.0).max(0.0);
        let inset = nested_padding.min(max_padding);
        x1 += inset;
        x2 -= inset;
    }

    if depth == 0 && includes_leftmost {
        x1 = x1.min(state.config.padding);
    }

    (x1, x2)
}

/// Pre-calculate block backgrounds by doing a dry run
fn collect_block_backgrounds(
    state: &mut RenderState,
    items: &[Item],
    depth: usize,
    active_activation_count: &mut usize,
) {
    for item in items {
        match item {
            Item::Message { text, from, to, arrow, activate, deactivate, create, .. } => {
                state.apply_else_return_gap(arrow);
                let chain_gap = if *activate && depth == 0 && *active_activation_count == 1 {
                    ACTIVATION_CHAIN_GAP
                } else {
                    0.0
                };
                let is_self = from == to;
                let lines: Vec<&str> = text.split("\\n").collect();
                let delay_offset = arrow.delay.map(|d| d as f64 * DELAY_UNIT).unwrap_or(0.0);

                if is_self {
                    let mut spacing = self_message_spacing(&state.config, lines.len());
                    if state.in_serial_block() {
                        spacing -= SERIAL_SELF_MESSAGE_ADJUST;
                    }
                    if *active_activation_count > 0 {
                        spacing -= SELF_MESSAGE_ACTIVE_ADJUST;
                    }
                    state.current_y += spacing;
                } else {
                    let spacing_line_height = message_spacing_line_height(&state.config);
                    let extra_height = if lines.len() > 1 {
                        (lines.len() - 1) as f64 * spacing_line_height
                    } else {
                        0.0
                    };
                    if lines.len() > 1 {
                        state.current_y += extra_height;
                    }
                    state.current_y += state.config.row_height + delay_offset;
                }

                if *create {
                    state.current_y += CREATE_MESSAGE_SPACING;
                }

                state.apply_serial_first_row_gap();

                if *activate && depth == 0 {
                    state.current_y += ACTIVATION_START_GAP;
                }
                if chain_gap > 0.0 {
                    state.current_y += chain_gap;
                }
                if *activate {
                    *active_activation_count += 1;
                }
                if *deactivate && *active_activation_count > 0 {
                    *active_activation_count -= 1;
                }
            }
            Item::Note { text, .. } => {
                let lines: Vec<&str> = text.split("\\n").collect();
                let line_height = note_line_height(&state.config);
                let note_height = note_padding(&state.config) * 2.0 + lines.len() as f64 * line_height;
                state.current_y += note_height.max(state.config.row_height) + NOTE_MARGIN;
            }
            Item::State { text, .. } => {
                let lines: Vec<&str> = text.split("\\n").collect();
                let line_height = state_line_height(&state.config);
                let box_height = state.config.note_padding * 2.0 + lines.len() as f64 * line_height;
                state.current_y += box_height + item_pre_gap(&state.config) + STATE_EXTRA_GAP;
            }
            Item::Ref { text, .. } => {
                let lines: Vec<&str> = text.split("\\n").collect();
                let line_height = ref_line_height(&state.config);
                let box_height = state.config.note_padding * 2.0 + lines.len() as f64 * line_height;
                state.current_y += box_height + item_pre_gap(&state.config) + REF_EXTRA_GAP;
            }
            Item::Description { text } => {
                let lines: Vec<&str> = text.split("\\n").collect();
                let line_height = state.config.font_size + 4.0;
                state.current_y += lines.len() as f64 * line_height + 10.0;
            }
            Item::Destroy { .. } => {
                state.current_y += DESTROY_SPACING;
            }
            Item::Activate { .. } => {
                *active_activation_count += 1;
            }
            Item::Deactivate { .. } => {
                if *active_activation_count > 0 {
                    *active_activation_count -= 1;
                }
            }
            Item::Block { kind, label, items, else_items } => {
                if block_is_parallel(kind) {
                    state.push_parallel();
                    let start_y = state.current_y;
                    let mut max_end_y = start_y;
                    let start_activation_count = *active_activation_count;
                    for item in items {
                        state.current_y = start_y;
                        *active_activation_count = start_activation_count;
                        collect_block_backgrounds(state, std::slice::from_ref(item), depth, active_activation_count);
                        if state.current_y > max_end_y {
                            max_end_y = state.current_y;
                        }
                    }
                    *active_activation_count = start_activation_count;
                    let gap = if parallel_needs_gap(items) {
                        PARALLEL_BLOCK_GAP
                    } else {
                        0.0
                    };
                    state.current_y = max_end_y + gap;
                    state.pop_parallel();
                    continue;
                }

                if matches!(kind, BlockKind::Serial) {
                    state.push_serial_first_row_pending();
                    collect_block_backgrounds(state, items, depth, active_activation_count);
                    if let Some(else_items) = else_items {
                        collect_block_backgrounds(state, else_items, depth, active_activation_count);
                    }
                    state.pop_serial_first_row_pending();
                    continue;
                }

                if !block_has_frame(kind) {
                    collect_block_backgrounds(state, items, depth, active_activation_count);
                    if let Some(else_items) = else_items {
                        collect_block_backgrounds(state, else_items, depth, active_activation_count);
                    }
                    continue;
                }

                let start_y = state.current_y;
                let frame_shift = block_frame_shift(depth);
                let frame_start_y = start_y - frame_shift;

                // Calculate bounds based on involved participants and label width
                let (x1, x2) = calculate_block_bounds_with_label(items, else_items.as_deref(), label, kind.as_str(), depth, state);

                state.current_y += block_header_space(&state.config, depth);
                collect_block_backgrounds(state, items, depth + 1, active_activation_count);

                let else_y = if else_items.is_some() {
                    Some(state.current_y)
                } else {
                    None
                };

                if let Some(else_items) = else_items {
                    state.push_else_return_pending();
                    state.current_y += block_else_spacing(&state.config, depth);
                    collect_block_backgrounds(state, else_items, depth + 1, active_activation_count);
                    state.pop_else_return_pending();
                }

                let end_y = state.current_y - state.config.row_height + block_footer_padding(&state.config, depth);
                let frame_end_y = end_y - frame_shift;
                state.current_y = end_y + state.config.row_height * 0.5;

                // Collect this block's background
                state.add_block_background(x1, frame_start_y, x2 - x1, frame_end_y - frame_start_y);
                // Collect this block's label for rendering above activations/lifelines
                state.add_block_label(x1, frame_start_y, frame_end_y, x2, kind.as_str(), label, else_y);
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
/// This is called AFTER activations are drawn so labels appear on top
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
        let label_width = block_tab_width(label_text);
        let label_height = BLOCK_LABEL_HEIGHT;
        let label_text_offset = 16.0;
        let notch_size = 5.0;
        let label_font_size = state.config.font_size - 1.0;
        let label_padding_x = 6.0;

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
            y = start_y + label_text_offset,
            kind = label_text
        )
        .unwrap();

        // Condition label (outside the pentagon)
        if !bl.label.is_empty() {
            let condition_text = format!("[{}]", bl.label);
            let text_x = x1 + label_width + 8.0;
            let text_y = start_y + label_text_offset;
            let base_width = (estimate_text_width(&condition_text, label_font_size) - TEXT_WIDTH_PADDING).max(0.0);
            let bg_width = base_width + label_padding_x * 2.0;

            writeln!(
                svg,
                r##"<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{fill}" stroke="{stroke}" stroke-width="1"/>"##,
                x = text_x - label_padding_x,
                y = start_y,
                w = bg_width,
                h = label_height,
                fill = theme.block_label_fill,
                stroke = theme.block_stroke
            )
            .unwrap();

            writeln!(
                svg,
                r#"<text x="{x}" y="{y}" class="block-label">{label}</text>"#,
                x = text_x,
                y = text_y,
                label = escape_xml(&condition_text)
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

            let else_text = "[else]";
            let else_text_x = x1 + 4.0;
            let else_base_width = (estimate_text_width(else_text, label_font_size) - TEXT_WIDTH_PADDING).max(0.0);
            let else_bg_width = else_base_width + label_padding_x * 2.0;
            let else_rect_y = else_y - label_height;
            let else_text_y = else_rect_y + label_text_offset;

            writeln!(
                svg,
                r##"<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{fill}" stroke="{stroke}" stroke-width="1"/>"##,
                x = else_text_x - label_padding_x,
                y = else_rect_y,
                w = else_bg_width,
                h = label_height,
                fill = theme.block_label_fill,
                stroke = theme.block_stroke
            )
            .unwrap();

            writeln!(
                svg,
                r#"<text x="{x}" y="{y}" class="block-label">{label}</text>"#,
                x = else_text_x,
                y = else_text_y,
                label = else_text
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
    let content_height = calculate_height(&diagram.items, &state.config, 0);
    let title_space = if has_title { state.config.title_height } else { 0.0 };
    let footer_space = match footer_style {
        FooterStyle::Box => state.config.header_height,
        FooterStyle::Bar | FooterStyle::None => 0.0,
    };
    let total_height = state.config.padding * 2.0
        + title_space
        + state.config.header_height
        + footer_space
        + content_height;
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
        let title_y = state.config.padding + state.config.font_size + 10.0;
        writeln!(
            &mut svg,
            r#"<text x="{x}" y="{y}" class="title">{t}</text>"#,
            x = total_width / 2.0,
            y = title_y,
            t = escape_xml(title)
        )
        .unwrap();
    }

    // Calculate footer position
    let header_y = state.header_top();
    let footer_y = match footer_style {
        FooterStyle::Box => total_height - state.config.padding - state.config.header_height,
        FooterStyle::Bar | FooterStyle::None => total_height - state.config.padding,
    };

    // Pre-calculate block backgrounds (dry run)
    state.current_y = state.content_start();
    let mut active_activation_count = 0;
    collect_block_backgrounds(&mut state, &diagram.items, 0, &mut active_activation_count);

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

    // Draw participant headers
    render_participant_headers(&mut svg, &state, header_y);

    // Render items
    state.current_y = state.content_start();
    render_items(&mut svg, &mut state, &diagram.items, 0);

    // Draw activation bars
    render_activations(&mut svg, &mut state, footer_y);

    // Draw block labels AFTER activations so they appear on top
    render_block_labels(&mut svg, &state);

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

fn calculate_height(items: &[Item], config: &Config, depth: usize) -> f64 {
    fn inner(
        items: &[Item],
        config: &Config,
        depth: usize,
        else_pending: &mut Vec<bool>,
        serial_pending: &mut Vec<bool>,
        active_activation_count: &mut usize,
        parallel_depth: &mut usize,
    ) -> f64 {
        let mut height = 0.0;
        let line_height = config.font_size + 4.0;
        for item in items {
            match item {
                Item::Message { from, to, text, arrow, create, activate, deactivate, .. } => {
                    if let Some(pending) = else_pending.last_mut() {
                        if *pending && matches!(arrow.line, LineStyle::Dashed) {
                            height += ELSE_RETURN_GAP;
                            *pending = false;
                        }
                    }
                    let chain_gap = if *activate && depth == 0 && *active_activation_count == 1 {
                        ACTIVATION_CHAIN_GAP
                    } else {
                        0.0
                    };
                    let is_self = from == to;
                    let lines = text.split("\\n").count();
                    let delay_offset = arrow.delay.map(|d| d as f64 * DELAY_UNIT).unwrap_or(0.0);
                    if is_self {
                        let mut spacing = self_message_spacing(config, lines);
                        if !serial_pending.is_empty() {
                            spacing -= SERIAL_SELF_MESSAGE_ADJUST;
                        }
                        if *active_activation_count > 0 {
                            spacing -= SELF_MESSAGE_ACTIVE_ADJUST;
                        }
                        height += spacing;
                    } else {
                        let spacing_line_height = message_spacing_line_height(config);
                        height += config.row_height + (lines.saturating_sub(1)) as f64 * spacing_line_height + delay_offset;
                    }
                    if *create {
                        height += CREATE_MESSAGE_SPACING;
                    }
                    if let Some(pending) = serial_pending.last_mut() {
                        if *pending {
                            height += serial_first_row_gap(*parallel_depth);
                            *pending = false;
                        }
                    }
                    if *activate && depth == 0 {
                        height += ACTIVATION_START_GAP;
                    }
                    height += chain_gap;
                    if *activate {
                        *active_activation_count += 1;
                    }
                    if *deactivate && *active_activation_count > 0 {
                        *active_activation_count -= 1;
                    }
                }
                Item::Note { text, .. } => {
                    let lines = text.split("\\n").count();
                    let note_height = note_padding(config) * 2.0 + lines as f64 * note_line_height(config);
                    height += note_height.max(config.row_height) + NOTE_MARGIN;
                }
                Item::State { text, .. } => {
                    let lines = text.split("\\n").count();
                    let box_height = config.note_padding * 2.0 + lines as f64 * state_line_height(config);
                    height += box_height + item_pre_gap(config) + STATE_EXTRA_GAP;
                }
                Item::Ref { text, .. } => {
                    let lines = text.split("\\n").count();
                    let box_height = config.note_padding * 2.0 + lines as f64 * ref_line_height(config);
                    height += box_height + item_pre_gap(config) + REF_EXTRA_GAP;
                }
                Item::Description { text } => {
                    let lines = text.split("\\n").count();
                    height += lines as f64 * line_height + 10.0;
                }
                Item::Block { kind, items, else_items, .. } => {
                    if block_is_parallel(kind) {
                        let mut max_branch_height = 0.0;
                        let base_activation_count = *active_activation_count;
                        *parallel_depth += 1;
                        for item in items {
                            *active_activation_count = base_activation_count;
                            let branch_height = inner(
                                std::slice::from_ref(item),
                                config,
                                depth,
                                else_pending,
                                serial_pending,
                                active_activation_count,
                                parallel_depth,
                            );
                            if branch_height > max_branch_height {
                                max_branch_height = branch_height;
                            }
                        }
                        *active_activation_count = base_activation_count;
                        if *parallel_depth > 0 {
                            *parallel_depth -= 1;
                        }
                        let gap = if parallel_needs_gap(items) {
                            PARALLEL_BLOCK_GAP
                        } else {
                            0.0
                        };
                        height += max_branch_height + gap;
                        continue;
                    }

                    if matches!(kind, BlockKind::Serial) {
                        serial_pending.push(true);
                        height += inner(items, config, depth, else_pending, serial_pending, active_activation_count, parallel_depth);
                        if let Some(else_items) = else_items {
                            height += inner(else_items, config, depth, else_pending, serial_pending, active_activation_count, parallel_depth);
                        }
                        serial_pending.pop();
                    } else if !block_has_frame(kind) {
                        height += inner(items, config, depth, else_pending, serial_pending, active_activation_count, parallel_depth);
                        if let Some(else_items) = else_items {
                            height += inner(else_items, config, depth, else_pending, serial_pending, active_activation_count, parallel_depth);
                        }
                    } else {
                        height += block_header_space(config, depth);
                        height += inner(items, config, depth + 1, else_pending, serial_pending, active_activation_count, parallel_depth);
                        if let Some(else_items) = else_items {
                            else_pending.push(true);
                            height += block_else_spacing(config, depth);
                            height += inner(else_items, config, depth + 1, else_pending, serial_pending, active_activation_count, parallel_depth);
                            else_pending.pop();
                        }
                        height += block_footer_padding(config, depth) + config.row_height * 0.5 - config.row_height;
                    }
                }
                Item::Activate { .. } => {
                    *active_activation_count += 1;
                }
                Item::Deactivate { .. } => {
                    if *active_activation_count > 0 {
                        *active_activation_count -= 1;
                    }
                }
                Item::Destroy { .. } => {
                    height += DESTROY_SPACING;
                }
                Item::ParticipantDecl { .. } => {}
                Item::Autonumber { .. } => {}
                Item::DiagramOption { .. } => {} // Options don't take space
            }
        }
        height
    }

    let mut else_pending = Vec::new();
    let mut serial_pending = Vec::new();
    let mut active_activation_count = 0;
    let mut parallel_depth = 0;
    inner(
        items,
        config,
        depth,
        &mut else_pending,
        &mut serial_pending,
        &mut active_activation_count,
        &mut parallel_depth,
    )
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

fn render_items(svg: &mut String, state: &mut RenderState, items: &[Item], depth: usize) {
    for item in items {
        match item {
            Item::Message {
                from,
                to,
                text,
                arrow,
                activate,
                deactivate,
                create,
                ..
            } => {
                render_message(
                    svg,
                    state,
                    from,
                    to,
                    text,
                    arrow,
                    *activate,
                    *deactivate,
                    *create,
                    depth,
                );
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
                render_block(svg, state, kind, label, items, else_items.as_deref(), depth);
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
                state.current_y += DESTROY_SPACING;
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
    create: bool,
    depth: usize,
) {
    let x1 = state.get_x(from);
    let x2 = state.get_x(to);

    state.apply_else_return_gap(arrow);
    let active_count = state.active_activation_count();
    let chain_gap = if activate && depth == 0 && active_count == 1 {
        ACTIVATION_CHAIN_GAP
    } else {
        0.0
    };

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
    let extra_height = if !is_self && lines.len() > 1 {
        let spacing_line_height = message_spacing_line_height(&state.config);
        (lines.len() - 1) as f64 * spacing_line_height
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

        let mut spacing = self_message_spacing(&state.config, lines.len());
        if state.in_serial_block() {
            spacing -= SERIAL_SELF_MESSAGE_ADJUST;
        }
        if active_count > 0 {
            spacing -= SELF_MESSAGE_ACTIVE_ADJUST;
        }
        state.current_y += spacing;
    } else {
        // Regular message - check for delay
        let delay_offset = arrow.delay.map(|d| d as f64 * DELAY_UNIT).unwrap_or(0.0);
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

    if create {
        state.current_y += CREATE_MESSAGE_SPACING;
    }

    state.apply_serial_first_row_gap();

    if activate && depth == 0 {
        state.current_y += ACTIVATION_START_GAP;
    }
    if chain_gap > 0.0 {
        state.current_y += chain_gap;
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
    let line_height = note_line_height(&state.config);
    let padding = note_padding(&state.config);
    let note_height = padding * 2.0 + lines.len() as f64 * line_height;

    // Calculate note width based on content or participant span
    // Use 12.0 per char for CJK text estimation (wider than ASCII)
    let max_line_len = lines.iter().map(|l| l.chars().count()).max().unwrap_or(10);
    let content_width = (max_line_len as f64 * 10.0 + padding * 2.0).max(80.0);

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
        "start" => x + padding,
        _ => x + note_width - padding,
    };

    for (i, line) in lines.iter().enumerate() {
        let text_y = y + padding + (i as f64 + 0.8) * line_height;
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
    state.current_y += note_height.max(state.config.row_height) + NOTE_MARGIN;
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
    let line_height = state_line_height(&state.config);
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

    let shift = item_pre_shift(&state.config);
    let y = (state.current_y - shift).max(state.content_start());

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

    state.current_y = y + box_height + state.config.row_height + REF_EXTRA_GAP;
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
    let line_height = ref_line_height(&state.config);
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

    let shift = item_pre_shift(&state.config);
    let y = (state.current_y - shift).max(state.content_start());
    let input_offset = state.config.note_padding + state.config.font_size + 1.0;
    let output_padding = state.config.note_padding + 3.0;

    // Draw input signal arrow if present
    if let Some(from) = input_from {
        let from_x = state.get_x(from);
        let to_x = x; // Left edge of ref box
        let arrow_y = y + input_offset;

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
        let arrow_y = y + box_height - output_padding;

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

    state.current_y = y + box_height + state.config.row_height + STATE_EXTRA_GAP;
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
    kind: &BlockKind,
    _label: &str,
    items: &[Item],
    else_items: Option<&[Item]>,
    depth: usize,
) {
    if block_is_parallel(kind) {
        state.push_parallel();
        let start_y = state.current_y;
        let mut max_end_y = start_y;
        for item in items {
            state.current_y = start_y;
            render_items(svg, state, std::slice::from_ref(item), depth);
            if state.current_y > max_end_y {
                max_end_y = state.current_y;
            }
        }
        let gap = if parallel_needs_gap(items) {
            PARALLEL_BLOCK_GAP
        } else {
            0.0
        };
        state.current_y = max_end_y + gap;
        state.pop_parallel();
        return;
    }

    if matches!(kind, BlockKind::Serial) {
        state.push_serial_first_row_pending();
        render_items(svg, state, items, depth);
        if let Some(else_items) = else_items {
            render_items(svg, state, else_items, depth);
        }
        state.pop_serial_first_row_pending();
        return;
    }

    if !block_has_frame(kind) {
        render_items(svg, state, items, depth);
        if let Some(else_items) = else_items {
            render_items(svg, state, else_items, depth);
        }
        return;
    }

    // Note: Block frame, labels, and else separators are rendered by render_block_labels()
    // This function only handles Y position tracking and rendering of inner items
    // svg is still used for rendering inner items via render_items()

    state.current_y += block_header_space(&state.config, depth);

    // Render items
    render_items(svg, state, items, depth + 1);

    // Render else items if present
    if let Some(else_items) = else_items {
        state.push_else_return_pending();
        state.current_y += block_else_spacing(&state.config, depth);
        render_items(svg, state, else_items, depth + 1);
        state.pop_else_return_pending();
    }

    let end_y = state.current_y - state.config.row_height + block_footer_padding(&state.config, depth);

    // Set current_y to end of block + margin
    state.current_y = end_y + state.config.row_height * 0.5;

    // Block frame, labels, and else separators are rendered later by render_block_labels()
    // which is called after activations are drawn, so labels appear on top
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
