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
        // Values calibrated to match websequencediagrams.com output exactly
        // Reference: WSD Ultimate Stress Test produces:
        //   - First participant box: x=78.5, y=110.5
        //   - Box height: 46px (1 line), 108px (2+ lines)
        //   - Box width: dynamic based on text
        Self {
            padding: 10.5,           // WSD: padding from SVG edge
            left_margin: 66.0,       // WSD: Mobile lifeline at 124.5, box left edge at 78.5
            right_margin: 10.0,      // WSD: minimal right margin, dynamically expanded for notes
            participant_gap: 85.0,   // WSD: minimum gap for participants with no messages between
            header_height: 46.0,     // WSD: participant box height = 46px (single line)
            row_height: 32.0,        // WSD: actual row height = 32px
            participant_width: 92.0, // WSD: minimum participant width = 92px
            font_size: 14.0,         // WSD: uses 14px font
            activation_width: 8.0,   // WSD: narrower activation bars
            note_padding: 6.0,
            block_margin: 5.0,
            title_height: 100.0,     // WSD: title + space before participant boxes (y=110.5)
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

#[derive(Debug, Clone)]
struct LabelBox {
    x_min: f64,
    x_max: f64,
    y_min: f64,
    y_max: f64,
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
    /// Tracks message label bounding boxes to avoid overlap
    message_label_boxes: Vec<LabelBox>,
}

const TEXT_WIDTH_PADDING: f64 = 41.0;
const TEXT_WIDTH_SCALE: f64 = 1.3;
const MESSAGE_WIDTH_PADDING: f64 = 4.0;  // WSD: minimal padding
const MESSAGE_WIDTH_SCALE: f64 = 0.82;  // WSD: text width estimate for gap calculation
const DELAY_UNIT: f64 = 18.0;
// ブロック関連定数（シンプル化）
const BLOCK_PADDING: f64 = 8.0;                  // 4隅同じパディング
const BLOCK_LABEL_HEIGHT: f64 = 22.0;            // ラベル領域の高さ（ペンタゴン部分）
const BLOCK_TITLE_PADDING: f64 = 12.0;           // タイトル行と最初のメッセージの間のパディング
const BLOCK_FOOTER_PADDING: f64 = 8.0;           // フッター余白（統一）
const BLOCK_ELSE_BEFORE: f64 = 8.0;              // else線の前の余白（小さめ）
const BLOCK_ELSE_AFTER: f64 = 32.0;              // else線の後の余白（メッセージテキストが線と重ならないように）
const BLOCK_NESTED_OFFSET: f64 = 22.0;           // ネスト時のオフセット
const BLOCK_GAP: f64 = 14.0;                     // ブロック間の余白

// 要素間余白
const ROW_SPACING: f64 = 20.0;                   // 要素間の基本余白（広め）

const MESSAGE_SPACING_MULT: f64 = 0.375;         // Fine-tuned from 0.5625
const SELF_MESSAGE_MIN_SPACING: f64 = 54.0;      // Fine-tuned from 78
const SELF_MESSAGE_GAP: f64 = 14.0;              // WSD: gap after self-message loop
const SELF_MESSAGE_PRE_GAP_REDUCTION: f64 = 9.0; // WSD: reduced gap before self-message
const CREATE_MESSAGE_SPACING: f64 = 27.5;        // Fine-tuned from 41
const DESTROY_SPACING: f64 = 10.7;               // Fine-tuned from 15
// ノート関連定数（シンプル化）
const NOTE_PADDING: f64 = 8.0;                   // 4隅同じパディング
const NOTE_MARGIN: f64 = 10.0;                   // ノート端とライフライン間
const NOTE_FOLD_SIZE: f64 = 8.0;                 // 折り目サイズ
const NOTE_CHAR_WIDTH: f64 = 7.0;                // 文字幅推定値
const NOTE_LINE_HEIGHT: f64 = 17.0;              // 行高さ（フォント13px + 4px）
const NOTE_MIN_WIDTH: f64 = 50.0;                // 最小幅
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
const MESSAGE_LABEL_COLLISION_PADDING: f64 = 2.0;
const MESSAGE_LABEL_COLLISION_STEP_RATIO: f64 = 0.9;
const MESSAGE_LABEL_ASCENT_FACTOR: f64 = 0.8;
const MESSAGE_LABEL_DESCENT_FACTOR: f64 = 0.2;

fn block_header_space(_config: &Config, _depth: usize) -> f64 {
    // タイトル行（ペンタゴン+ラベル）の高さ + タイトルとコンテンツ間のパディング
    BLOCK_LABEL_HEIGHT + BLOCK_TITLE_PADDING
}

fn block_frame_shift(depth: usize) -> f64 {
    if depth == 0 {
        0.0
    } else {
        BLOCK_NESTED_OFFSET
    }
}

fn block_footer_padding(_config: &Config, _depth: usize) -> f64 {
    // シンプル化：深さに関係なく統一値
    BLOCK_FOOTER_PADDING
}

fn block_else_before(_config: &Config, _depth: usize) -> f64 {
    BLOCK_ELSE_BEFORE
}

fn block_else_after(_config: &Config, _depth: usize) -> f64 {
    BLOCK_ELSE_AFTER
}

fn message_spacing_line_height(config: &Config) -> f64 {
    config.row_height * MESSAGE_SPACING_MULT
}

fn self_message_spacing(config: &Config, lines: usize) -> f64 {
    let line_height = config.font_size + 4.0;
    let text_block_height = lines as f64 * line_height;
    // WSD: loop height equals text block height, no extra padding
    let loop_height = text_block_height.max(25.0);
    // WSD: gap after loop = 14px (pre-gap reduction is applied separately before self-message)
    let base = loop_height + SELF_MESSAGE_GAP;
    if lines >= 3 {
        base.max(SELF_MESSAGE_MIN_SPACING)
    } else {
        base
    }
}

fn note_line_height(_config: &Config) -> f64 {
    // シンプル化：固定値（フォントサイズ13px + 余白4px = 17px）
    NOTE_LINE_HEIGHT
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

fn label_boxes_overlap(x_min: f64, x_max: f64, y_min: f64, y_max: f64, other: &LabelBox) -> bool {
    let x_overlap = x_max >= other.x_min - MESSAGE_LABEL_COLLISION_PADDING
        && x_min <= other.x_max + MESSAGE_LABEL_COLLISION_PADDING;
    let y_overlap = y_max >= other.y_min - MESSAGE_LABEL_COLLISION_PADDING
        && y_min <= other.y_max + MESSAGE_LABEL_COLLISION_PADDING;
    x_overlap && y_overlap
}

fn actor_footer_extra(_participants: &[Participant], _config: &Config) -> f64 {
    // Actor names are now rendered within the header, so no extra footer space needed
    0.0
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

/// Arrowhead size constant
const ARROWHEAD_SIZE: f64 = 10.0;

/// Generate arrowhead polygon points for a given end position and direction
fn arrowhead_points(x: f64, y: f64, direction: f64) -> String {
    let size = ARROWHEAD_SIZE;
    let half_width = size * 0.35;

    // Tip of the arrow
    let tip_x = x;
    let tip_y = y;

    // Back points of the arrow (rotated by direction)
    let back_x = x - size * direction.cos();
    let back_y = y - size * direction.sin();

    // Perpendicular offset for the two back points
    let perp_x = -direction.sin() * half_width;
    let perp_y = direction.cos() * half_width;

    format!(
        "{:.1},{:.1} {:.1},{:.1} {:.1},{:.1}",
        back_x + perp_x,
        back_y + perp_y,
        tip_x,
        tip_y,
        back_x - perp_x,
        back_y - perp_y
    )
}

/// Calculate direction angle from (x1, y1) to (x2, y2)
fn arrow_direction(x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    (y2 - y1).atan2(x2 - x1)
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
        if c.is_uppercase() {
            0.7
        } else {
            0.5
        }
    } else {
        1.0 // CJK and other characters are wider
    }
}

/// Character width for participant box calculation (WSD proportional font metrics)
/// Based on analysis of WSD SVG glyph definitions and actual output comparison
fn participant_char_width(c: char) -> f64 {
    match c {
        // Very wide: W, M, m, w, @
        'W' | 'w' => 14.0,
        'M' | 'm' => 12.5,
        '@' | '%' => 14.0,
        // Wide uppercase
        'A' | 'B' | 'C' | 'D' | 'E' | 'G' | 'H' | 'K' | 'N' | 'O' | 'P' | 'Q' | 'R' | 'S' | 'T' | 'U' | 'V' | 'X' | 'Y' | 'Z' => 12.0,
        // Narrow uppercase
        'F' | 'I' | 'J' | 'L' => 7.0,
        // Wide lowercase
        'o' | 'e' | 'a' | 'n' | 'u' | 'v' | 'x' | 'z' | 'b' | 'd' | 'g' | 'h' | 'k' | 'p' | 'q' | 's' | 'c' | 'y' => 8.5,
        // Narrow lowercase
        'i' | 'j' | 'l' => 4.0,
        't' | 'f' | 'r' => 6.0,
        // Punctuation and special chars (WSD uses wider glyphs for these)
        ':' => 6.5,
        '-' | '_' => 7.0,
        '[' | ']' | '(' | ')' | '{' | '}' => 7.0,
        '.' | ',' | '\'' | '`' | ';' => 4.0,
        ' ' => 5.0,
        // Numbers
        '0'..='9' => 9.0,
        // Default for other ASCII
        _ if c.is_ascii() => 8.5,
        // CJK and other characters
        _ => 14.0,
    }
}

/// Calculate participant box width based on WSD proportional font metrics
fn calculate_participant_width(name: &str, min_width: f64) -> f64 {
    let lines: Vec<&str> = name.split("\\n").collect();
    let max_line_width = lines
        .iter()
        .map(|line| line.chars().map(participant_char_width).sum::<f64>())
        .fold(0.0_f64, |a, b| a.max(b));

    // WSD uses consistent padding for all participant boxes
    let padding = 50.0;

    (max_line_width + padding).max(min_width)
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

/// Calculate note width based on text content
fn calculate_note_width(text: &str, _config: &Config) -> f64 {
    let lines: Vec<&str> = text.split("\\n").collect();
    let max_line_len = lines.iter().map(|l| l.chars().count()).max().unwrap_or(5);
    let text_width = max_line_len as f64 * NOTE_CHAR_WIDTH;
    (NOTE_PADDING * 2.0 + text_width).max(NOTE_MIN_WIDTH)
}

/// Calculate required right margin based on right-side notes on the rightmost participant only
fn calculate_right_margin(
    participants: &[Participant],
    items: &[Item],
    config: &Config,
) -> f64 {
    let rightmost_id = match participants.last() {
        Some(p) => p.id().to_string(),
        None => return config.right_margin,
    };
    let mut max_right_note_width: f64 = 0.0;

    fn process_items_for_right_notes(
        items: &[Item],
        rightmost_id: &str,
        max_width: &mut f64,
        config: &Config,
    ) {
        for item in items {
            match item {
                Item::Note {
                    position: NotePosition::Right,
                    participants,
                    text,
                } => {
                    // Only consider notes on the rightmost participant
                    if participants.first().map(|s| s.as_str()) == Some(rightmost_id) {
                        let note_width = calculate_note_width(text, config);
                        if note_width > *max_width {
                            *max_width = note_width;
                        }
                    }
                }
                Item::Block {
                    items, else_items, ..
                } => {
                    process_items_for_right_notes(items, rightmost_id, max_width, config);
                    if let Some(else_items) = else_items {
                        process_items_for_right_notes(else_items, rightmost_id, max_width, config);
                    }
                }
                _ => {}
            }
        }
    }

    process_items_for_right_notes(items, &rightmost_id, &mut max_right_note_width, config);

    // right_margin needs to accommodate: NOTE_MARGIN + note_width
    if max_right_note_width > 0.0 {
        (max_right_note_width + NOTE_MARGIN).max(config.right_margin)
    } else {
        config.right_margin
    }
}

/// Calculate required left margin based on left-side notes on the leftmost participant
fn calculate_left_margin(
    participants: &[Participant],
    items: &[Item],
    config: &Config,
) -> f64 {
    let leftmost_id = match participants.first() {
        Some(p) => p.id().to_string(),
        None => return config.padding,
    };
    let mut max_left_note_width: f64 = 0.0;

    fn process_items_for_left_notes(
        items: &[Item],
        leftmost_id: &str,
        max_width: &mut f64,
        config: &Config,
    ) {
        for item in items {
            match item {
                Item::Note {
                    position: NotePosition::Left,
                    participants,
                    text,
                } => {
                    // Only consider notes on the leftmost participant
                    if participants.first().map(|s| s.as_str()) == Some(leftmost_id) {
                        let note_width = calculate_note_width(text, config);
                        if note_width > *max_width {
                            *max_width = note_width;
                        }
                    }
                }
                Item::Block {
                    items, else_items, ..
                } => {
                    process_items_for_left_notes(items, leftmost_id, max_width, config);
                    if let Some(else_items) = else_items {
                        process_items_for_left_notes(else_items, leftmost_id, max_width, config);
                    }
                }
                _ => {}
            }
        }
    }

    process_items_for_left_notes(items, &leftmost_id, &mut max_left_note_width, config);

    // left_margin needs to accommodate: note_width + NOTE_MARGIN
    if max_left_note_width > 0.0 {
        (max_left_note_width + NOTE_MARGIN).max(config.padding)
    } else {
        config.padding
    }
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

    // Initialize gaps with WSD-compatible minimum gap
    // WSD uses ~59px center-to-center for simple diagrams
    let min_gap = config.participant_gap;
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
                Item::Message { from, to, text, arrow, .. } => {
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

                            // WSD: delay messages need extra horizontal space for diagonal lines
                            // Delay coefficient 86.4 for WSD gap matching (645px for delay(7))
                            let delay_extra = arrow.delay.map(|d| d as f64 * 86.4).unwrap_or(0.0);

                            // WSD: distribute text width across gaps with appropriate spacing
                            let gap_count = (max_idx - min_idx) as f64;
                            let needed_gap = if gap_count == 1.0 {
                                // Adjacent: text width minus overlap allowance
                                text_width - 36.0 + delay_extra
                            } else {
                                // Non-adjacent: distribute evenly with margin
                                text_width / gap_count - 20.0 + delay_extra
                            };

                            // Update gaps between the participants
                            for gap_idx in min_idx..max_idx {
                                if needed_gap > gaps[gap_idx] {
                                    gaps[gap_idx] = needed_gap;
                                }
                            }
                        }
                    }
                }
                Item::Note {
                    position,
                    participants: note_participants,
                    text,
                } => {
                    // ノート幅を計算
                    let note_width = calculate_note_width(text, config);

                    if let Some(participant) = note_participants.first() {
                        if let Some(&idx) = participant_index.get(participant) {
                            match position {
                                NotePosition::Left => {
                                    // 左ノートの場合：左隣の参加者との間にスペースが必要
                                    if idx > 0 {
                                        // ノート幅 + マージン分のギャップが必要
                                        let needed_gap = note_width + NOTE_MARGIN * 2.0;
                                        if needed_gap > gaps[idx - 1] {
                                            gaps[idx - 1] = needed_gap;
                                        }
                                    }
                                }
                                NotePosition::Right => {
                                    // 右ノートの場合：右隣の参加者との間にスペースが必要
                                    if idx < gaps.len() {
                                        let needed_gap = note_width + NOTE_MARGIN * 2.0;
                                        if needed_gap > gaps[idx] {
                                            gaps[idx] = needed_gap;
                                        }
                                    }
                                }
                                NotePosition::Over => {
                                    // 中央ノートは両端にまたがる場合のみ処理
                                    // 単一参加者の場合は幅を超えない限り問題なし
                                }
                            }
                        }
                    }
                }
                Item::Block {
                    items, else_items, ..
                } => {
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

    // WSD: participant name lengths don't directly increase gaps
    // The participant box widths (already calculated elsewhere) handle this
    // No additional gap increase needed for names

    // Cap maximum gap (WSD allows up to ~645px for long messages)
    let max_gap = 645.0;
    for gap in &mut gaps {
        if *gap > max_gap {
            *gap = max_gap;
        }
    }

    gaps
}

impl RenderState {
    fn new(
        config: Config,
        participants: Vec<Participant>,
        items: &[Item],
        has_title: bool,
        footer_style: FooterStyle,
    ) -> Self {
        let mut config = config;
        // WSD header height calculation:
        // - 1 line: 46px
        // - 2+ lines: 108px (WSD caps at 108px regardless of line count)
        // - Actor: ~108px for 2-line names
        let mut required_header_height = config.header_height;
        for p in &participants {
            let lines = p.name.split("\\n").count();
            let needed = match p.kind {
                ParticipantKind::Participant => {
                    // WSD: 46px for 1 line, 108px for 2+ lines (capped)
                    if lines <= 1 {
                        46.0
                    } else {
                        108.0 // WSD uses fixed 108px for multi-line
                    }
                }
                ParticipantKind::Actor => {
                    // WSD: Actor has stick figure + name below
                    // ~85px for 1-line, ~108px for 2+ lines
                    if lines <= 1 {
                        85.0
                    } else {
                        108.0
                    }
                }
            };
            if needed > required_header_height {
                required_header_height = needed;
            }
        }
        if required_header_height > config.header_height {
            config.header_height = required_header_height;
        }
        // Calculate individual participant widths based on their names
        // Using WSD proportional font metrics for accurate box widths
        let mut participant_widths: HashMap<String, f64> = HashMap::new();
        let min_width = config.participant_width;

        for p in &participants {
            let width = calculate_participant_width(&p.name, min_width);
            participant_widths.insert(p.id().to_string(), width);
        }

        let gaps = calculate_participant_gaps(&participants, items, &config);

        // Left margin for notes on leftmost participant (dynamic)
        let left_margin = calculate_left_margin(&participants, items, &config);
        // Right margin for self-loops and notes on rightmost participant (dynamic)
        let right_margin = calculate_right_margin(&participants, items, &config);

        let mut participant_x = HashMap::new();
        let first_width = participants
            .first()
            .map(|p| *participant_widths.get(p.id()).unwrap_or(&min_width))
            .unwrap_or(min_width);
        let mut current_x = config.padding + left_margin + first_width / 2.0;

        for (i, p) in participants.iter().enumerate() {
            participant_x.insert(p.id().to_string(), current_x);
            if i < gaps.len() {
                let current_width = *participant_widths.get(p.id()).unwrap_or(&min_width);
                let next_p = participants.get(i + 1);
                let next_width = next_p
                    .map(|np| *participant_widths.get(np.id()).unwrap_or(&min_width))
                    .unwrap_or(min_width);

                // WSD: Actor doesn't have a header box, so it takes less horizontal space
                // Reduce gap when current or next participant is an Actor
                let current_is_actor = p.kind == ParticipantKind::Actor;
                let next_is_actor = next_p.map(|np| np.kind == ParticipantKind::Actor).unwrap_or(false);

                // Note: Actor gap reduction disabled - it changes total width
                // WSD and OSD have different actor placement algorithms
                let actor_gap_reduction = 0.0;
                let _ = (current_is_actor, next_is_actor); // suppress warnings

                // WSD: edge-to-edge gap varies by message density
                // Variable edge padding: more messages = more edge padding
                let calculated_gap = gaps[i] - actor_gap_reduction;

                // Determine edge padding based on message density and participant types
                // WSD uses variable edge padding based on content
                let half_widths = (current_width + next_width) / 2.0;
                let neither_is_actor = !current_is_actor && !next_is_actor;

                let either_is_actor = current_is_actor || next_is_actor;
                let edge_padding = if calculated_gap > 500.0 {
                    // Very high (delay messages): minimal extra padding
                    10.0
                } else if either_is_actor && calculated_gap > 130.0 {
                    // Actor-adjacent gaps: WSD uses tighter spacing around actors
                    33.0
                } else if neither_is_actor && half_widths > 155.0 && calculated_gap > 130.0 {
                    // Two large normal boxes with medium traffic: extra padding
                    90.0
                } else if calculated_gap > 130.0 {
                    // Medium-high traffic: WSD uses ~49px for these gaps
                    49.0
                } else if calculated_gap > config.participant_gap {
                    // Medium traffic: moderate padding
                    25.0
                } else {
                    // Low traffic: edge_padding depends on individual participant widths
                    let max_width = current_width.max(next_width);
                    let min_width_val = current_width.min(next_width);
                    let width_diff = max_width - min_width_val;

                    if max_width > 160.0 && min_width_val > 160.0 {
                        // Both participants are very wide (>160): small positive padding
                        // WSD UserDB→Cache: both 161.2, gap=163, ep≈1.8
                        1.8
                    } else if max_width > 160.0 && min_width_val > 140.0 {
                        // One very wide, one large: negative padding
                        // WSD ML→Notify: max=161.2, min=149.6, gap=148.5, ep≈-7
                        -7.0
                    } else if max_width > 160.0 && min_width_val < 110.0 {
                        // One very wide, one small: large positive padding
                        // WSD Cache→Kafka: max=161.2, min=103.2, gap=143.5, ep≈11.3
                        11.3
                    } else if max_width > 160.0 && width_diff > 45.0 {
                        // One very wide, one medium-small: negative padding
                        // WSD Notify→Payment: max=161.2, min=114.8, diff=46.4, gap=132, ep≈-6
                        -6.0
                    } else if min_width_val < 115.0 {
                        // One small participant: moderate padding
                        // WSD Kafka→ML, Payment→Worker
                        10.0
                    } else {
                        // Medium participants: moderate padding
                        11.0
                    }
                };

                let min_center_gap = (current_width + next_width) / 2.0 + edge_padding - actor_gap_reduction;
                let actual_gap = calculated_gap.max(min_center_gap).max(60.0);
                current_x += actual_gap;
            }
        }

        let last_width = participants
            .last()
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
            message_label_boxes: Vec::new(),
        }
    }

    fn get_participant_width(&self, name: &str) -> f64 {
        *self
            .participant_widths
            .get(name)
            .unwrap_or(&self.config.participant_width)
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

    fn reserve_message_label(
        &mut self,
        x_min: f64,
        x_max: f64,
        mut y_min: f64,
        mut y_max: f64,
        step: f64,
    ) -> f64 {
        let mut offset = 0.0;
        let mut attempts = 0;
        while self
            .message_label_boxes
            .iter()
            .any(|b| label_boxes_overlap(x_min, x_max, y_min, y_max, b))
            && attempts < 20
        {
            y_min += step;
            y_max += step;
            offset += step;
            attempts += 1;
        }
        self.message_label_boxes.push(LabelBox {
            x_min,
            x_max,
            y_min,
            y_max,
        });
        offset
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

    /// Check if a participant has an active activation at the given Y position
    fn is_participant_active_at(&self, participant: &str, y: f64) -> bool {
        if let Some(acts) = self.activations.get(participant) {
            acts.iter().any(|(start_y, end_y)| {
                *start_y <= y && end_y.map_or(true, |end| y <= end)
            })
        } else {
            false
        }
    }

    /// Get arrow start X position, accounting for activation bar
    fn get_arrow_start_x(&self, participant: &str, y: f64, going_right: bool) -> f64 {
        let x = self.get_x(participant);
        if self.is_participant_active_at(participant, y) {
            let half_width = self.config.activation_width / 2.0;
            if going_right {
                x + half_width // Arrow starts from right edge of activation bar
            } else {
                x - half_width // Arrow starts from left edge of activation bar
            }
        } else {
            x
        }
    }

    /// Get arrow end X position, accounting for activation bar
    fn get_arrow_end_x(&self, participant: &str, y: f64, coming_from_right: bool) -> f64 {
        let x = self.get_x(participant);
        if self.is_participant_active_at(participant, y) {
            let half_width = self.config.activation_width / 2.0;
            if coming_from_right {
                x + half_width // Arrow ends at right edge of activation bar
            } else {
                x - half_width // Arrow ends at left edge of activation bar
            }
        } else {
            x
        }
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
        let leftmost_width = self
            .participants
            .first()
            .map(|p| self.get_participant_width(p.id()))
            .unwrap_or(self.config.participant_width);
        self.leftmost_x() - leftmost_width / 2.0 - self.config.block_margin
    }

    /// Get block right boundary (based on rightmost participant)
    fn block_right(&self) -> f64 {
        let rightmost_width = self
            .participants
            .last()
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
        // WSD first message Y: 250.5
        // header_top (110.5) + header_height (108) + row_height (32) = 250.5
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
        self.block_backgrounds.push(BlockBackground {
            x,
            y,
            width,
            height,
        });
    }

    /// Add a block label to be rendered later (above activations/lifelines)
    fn add_block_label(
        &mut self,
        x1: f64,
        start_y: f64,
        end_y: f64,
        x2: f64,
        kind: &str,
        label: &str,
        else_y: Option<f64>,
    ) {
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
                    update_bounds(
                        from,
                        state,
                        min_left,
                        max_right,
                        includes_leftmost,
                        leftmost_id,
                    );
                    update_bounds(
                        to,
                        state,
                        min_left,
                        max_right,
                        includes_leftmost,
                        leftmost_id,
                    );
                }
                Item::Note { participants, .. } => {
                    for p in participants {
                        update_bounds(
                            p,
                            state,
                            min_left,
                            max_right,
                            includes_leftmost,
                            leftmost_id,
                        );
                    }
                }
                Item::Block {
                    items, else_items, ..
                } => {
                    process_items(
                        items,
                        state,
                        min_left,
                        max_right,
                        includes_leftmost,
                        leftmost_id,
                    );
                    if let Some(else_items) = else_items {
                        process_items(
                            else_items,
                            state,
                            min_left,
                            max_right,
                            includes_leftmost,
                            leftmost_id,
                        );
                    }
                }
                Item::Activate { participant }
                | Item::Deactivate { participant }
                | Item::Destroy { participant } => {
                    update_bounds(
                        participant,
                        state,
                        min_left,
                        max_right,
                        includes_leftmost,
                        leftmost_id,
                    );
                }
                _ => {}
            }
        }
    }

    process_items(
        items,
        state,
        &mut min_left,
        &mut max_right,
        &mut includes_leftmost,
        leftmost_id,
    );

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

    let (base_x1, base_x2) =
        if let Some((min_left, max_right, _includes_leftmost)) =
            find_involved_participants(&items_slice, state)
        {
            let margin = state.config.block_margin;
            (min_left - margin, max_right + margin)
        } else {
            // Fallback to full width if no participants found
            (state.block_left(), state.block_right())
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
        let base_width =
            (estimate_text_width(&condition_text, label_font_size) - TEXT_WIDTH_PADDING).max(0.0);
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

    // 最左端の参加者を含む場合でも、参加者ボックスの左端から適度なマージンを取る
    // （paddingまで拡張しない）
    // Note: WSDはブロックを参加者に近づけて配置する

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
            Item::Message {
                text,
                from,
                to,
                arrow,
                activate,
                deactivate,
                create,
                ..
            } => {
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
                    // WSD: reduced gap before self-message (must match render_message)
                    state.current_y -= SELF_MESSAGE_PRE_GAP_REDUCTION;
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
                let note_height =
                    note_padding(&state.config) * 2.0 + lines.len() as f64 * line_height;
                // ROW_SPACING を使用（render_note と統一）
                state.current_y += note_height.max(state.config.row_height) + ROW_SPACING;
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
            Item::Block {
                kind,
                label,
                items,
                else_items,
            } => {
                if block_is_parallel(kind) {
                    state.push_parallel();
                    let start_y = state.current_y;
                    let mut max_end_y = start_y;
                    let start_activation_count = *active_activation_count;
                    for item in items {
                        state.current_y = start_y;
                        *active_activation_count = start_activation_count;
                        collect_block_backgrounds(
                            state,
                            std::slice::from_ref(item),
                            depth,
                            active_activation_count,
                        );
                        if state.current_y > max_end_y {
                            max_end_y = state.current_y;
                        }
                    }
                    *active_activation_count = start_activation_count;
                    let gap = if parallel_needs_gap(items) {
                        BLOCK_GAP
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
                        collect_block_backgrounds(
                            state,
                            else_items,
                            depth,
                            active_activation_count,
                        );
                    }
                    state.pop_serial_first_row_pending();
                    continue;
                }

                if !block_has_frame(kind) {
                    collect_block_backgrounds(state, items, depth, active_activation_count);
                    if let Some(else_items) = else_items {
                        collect_block_backgrounds(
                            state,
                            else_items,
                            depth,
                            active_activation_count,
                        );
                    }
                    continue;
                }

                let start_y = state.current_y;
                let frame_shift = block_frame_shift(depth);
                let frame_start_y = start_y - frame_shift;

                // Calculate bounds based on involved participants and label width
                let (x1, x2) = calculate_block_bounds_with_label(
                    items,
                    else_items.as_deref(),
                    label,
                    kind.as_str(),
                    depth,
                    state,
                );

                state.current_y += block_header_space(&state.config, depth);
                collect_block_backgrounds(state, items, depth + 1, active_activation_count);

                // else線の前にパディングを追加（小さめ）
                let else_y = if else_items.is_some() {
                    state.current_y += block_else_before(&state.config, depth);
                    Some(state.current_y)
                } else {
                    None
                };

                if let Some(else_items) = else_items {
                    state.push_else_return_pending();
                    // else線の後にパディングを追加（十分な間隔）
                    state.current_y += block_else_after(&state.config, depth);
                    collect_block_backgrounds(
                        state,
                        else_items,
                        depth + 1,
                        active_activation_count,
                    );
                    state.pop_else_return_pending();
                }

                // ブロック下端 = 現在のY位置 + フッターパディング
                // （メッセージがブロック外にはみ出ないように）
                let end_y = state.current_y + block_footer_padding(&state.config, depth);
                let frame_end_y = end_y - frame_shift;
                state.current_y = end_y + state.config.row_height;

                // Collect this block's background
                state.add_block_background(x1, frame_start_y, x2 - x1, frame_end_y - frame_start_y);
                // Collect this block's label for rendering above activations/lifelines
                state.add_block_label(
                    x1,
                    frame_start_y,
                    frame_end_y,
                    x2,
                    kind.as_str(),
                    label,
                    else_y,
                );
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

        // Condition label (text only, no background per WSD style)
        if !bl.label.is_empty() {
            let condition_text = format!("[{}]", bl.label);
            let text_x = x1 + label_width + 8.0;
            let text_y = start_y + label_text_offset;

            writeln!(
                svg,
                r#"<text x="{x}" y="{y}" class="block-label">{label}</text>"#,
                x = text_x,
                y = text_y,
                label = escape_xml(&condition_text)
            )
            .unwrap();
        }

        // Else separator (dashed line only, no [else] text per WSD style)
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
    let mut state = RenderState::new(
        config,
        participants,
        &diagram.items,
        has_title,
        footer_style,
    );
    let mut svg = String::new();

    // Pre-calculate height
    let content_height = calculate_height(&diagram.items, &state.config, 0);
    let title_space = if has_title {
        state.config.title_height
    } else {
        0.0
    };
    let footer_space = match footer_style {
        FooterStyle::Box => state.config.header_height,
        FooterStyle::Bar | FooterStyle::None => 0.0,
    };
    let footer_label_extra = match footer_style {
        FooterStyle::Box => actor_footer_extra(&state.participants, &state.config),
        FooterStyle::Bar | FooterStyle::None => 0.0,
    };
    let footer_margin = state.config.row_height; // Space between content and footer
    let base_total_height = state.config.padding * 2.0
        + title_space
        + state.config.header_height
        + content_height
        + footer_margin
        + footer_space;
    let total_height = base_total_height + footer_label_extra;
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
        ".message-text {{ font-family: {f}; font-size: {s}px; fill: {c}; stroke: none; }}",
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
    // Arrowhead styles
    writeln!(
        &mut svg,
        ".arrowhead {{ fill: {c}; stroke: none; }}",
        c = theme.message_color
    )
    .unwrap();
    writeln!(
        &mut svg,
        ".arrowhead-open {{ fill: none; stroke: {c}; stroke-width: 1; }}",
        c = theme.message_color
    )
    .unwrap();
    svg.push_str("</style>\n");
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
        let title_y = state.config.padding + state.config.font_size + 7.36; // WSD: 31.86
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
        FooterStyle::Box => base_total_height - state.config.padding - state.config.header_height,
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
            let left = state.leftmost_x()
                - state.get_participant_width(
                    state.participants.first().map(|p| p.id()).unwrap_or(""),
                ) / 2.0;
            let right = state.rightmost_x()
                + state
                    .get_participant_width(state.participants.last().map(|p| p.id()).unwrap_or(""))
                    / 2.0;
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
                Item::Message {
                    from,
                    to,
                    text,
                    arrow,
                    create,
                    activate,
                    deactivate,
                    ..
                } => {
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
                        height += config.row_height
                            + (lines.saturating_sub(1)) as f64 * spacing_line_height
                            + delay_offset;
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
                    let note_height =
                        note_padding(config) * 2.0 + lines as f64 * note_line_height(config);
                    // ROW_SPACING を使用（render_note と統一）
                    height += note_height.max(config.row_height) + ROW_SPACING;
                }
                Item::State { text, .. } => {
                    let lines = text.split("\\n").count();
                    let box_height =
                        config.note_padding * 2.0 + lines as f64 * state_line_height(config);
                    height += box_height + item_pre_gap(config) + STATE_EXTRA_GAP;
                }
                Item::Ref { text, .. } => {
                    let lines = text.split("\\n").count();
                    let box_height =
                        config.note_padding * 2.0 + lines as f64 * ref_line_height(config);
                    height += box_height + item_pre_gap(config) + REF_EXTRA_GAP;
                }
                Item::Description { text } => {
                    let lines = text.split("\\n").count();
                    height += lines as f64 * line_height + 10.0;
                }
                Item::Block {
                    kind,
                    items,
                    else_items,
                    ..
                } => {
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
                            BLOCK_GAP
                        } else {
                            0.0
                        };
                        height += max_branch_height + gap;
                        continue;
                    }

                    if matches!(kind, BlockKind::Serial) {
                        serial_pending.push(true);
                        height += inner(
                            items,
                            config,
                            depth,
                            else_pending,
                            serial_pending,
                            active_activation_count,
                            parallel_depth,
                        );
                        if let Some(else_items) = else_items {
                            height += inner(
                                else_items,
                                config,
                                depth,
                                else_pending,
                                serial_pending,
                                active_activation_count,
                                parallel_depth,
                            );
                        }
                        serial_pending.pop();
                    } else if !block_has_frame(kind) {
                        height += inner(
                            items,
                            config,
                            depth,
                            else_pending,
                            serial_pending,
                            active_activation_count,
                            parallel_depth,
                        );
                        if let Some(else_items) = else_items {
                            height += inner(
                                else_items,
                                config,
                                depth,
                                else_pending,
                                serial_pending,
                                active_activation_count,
                                parallel_depth,
                            );
                        }
                    } else {
                        height += block_header_space(config, depth);
                        height += inner(
                            items,
                            config,
                            depth + 1,
                            else_pending,
                            serial_pending,
                            active_activation_count,
                            parallel_depth,
                        );
                        if let Some(else_items) = else_items {
                            else_pending.push(true);
                            // else線の前後にパディング
                            height += block_else_before(config, depth) + block_else_after(config, depth);
                            height += inner(
                                else_items,
                                config,
                                depth + 1,
                                else_pending,
                                serial_pending,
                                active_activation_count,
                                parallel_depth,
                            );
                            else_pending.pop();
                        }
                        // ブロック下端とその後の余白
                        height += block_footer_padding(config, depth) + config.row_height;
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
                    let start_y = y + state.config.header_height / 2.0 - total_height / 2.0
                        + line_height * 0.8;
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
                // Stick figure at top of header area, name below within header
                let head_r = 8.0;
                let body_len = 12.0;
                let arm_len = 10.0;
                let leg_len = 10.0;
                let figure_height = 38.0; // head(16) + body(12) + legs(10)

                // Position figure at top with small margin
                let fig_top = y + 8.0;
                let fig_center_y = fig_top + head_r + body_len / 2.0;
                let arm_y = fig_center_y + 2.0;

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
                // Name below figure (within header)
                let name_lines: Vec<&str> = p.name.split("\\n").collect();
                let name_start_y = fig_top + figure_height + 5.0;
                if name_lines.len() == 1 {
                    writeln!(
                        svg,
                        r#"<text x="{x}" y="{y}" class="participant-text">{name}</text>"#,
                        x = x,
                        y = name_start_y + state.config.font_size,
                        name = escape_xml(&p.name)
                    )
                    .unwrap();
                } else {
                    // Multiline actor name using tspan
                    let line_height = state.config.font_size + 2.0;
                    writeln!(svg, r#"<text x="{x}" class="participant-text">"#, x = x).unwrap();
                    for (i, line) in name_lines.iter().enumerate() {
                        if i == 0 {
                            writeln!(
                                svg,
                                r#"<tspan x="{x}" y="{y}">{text}</tspan>"#,
                                x = x,
                                y = name_start_y + state.config.font_size,
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
                // X mark should be at the previous message's arrow position (WSD compatible)
                // After a message, current_y is incremented by row_height, so we subtract it back
                let destroy_y = state.current_y - state.config.row_height;
                state.destroyed.insert(participant.clone(), destroy_y);
                // Draw X mark on the lifeline
                let x = state.get_x(participant);
                let y = destroy_y;
                let size = 15.0; // WSD uses 15px for X mark size
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
            Item::Ref {
                participants,
                text,
                input_from,
                input_label,
                output_to,
                output_label,
            } => {
                render_ref(
                    svg,
                    state,
                    participants,
                    text,
                    input_from.as_deref(),
                    input_label.as_deref(),
                    output_to.as_deref(),
                    output_label.as_deref(),
                );
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
    // Get base lifeline positions (used for text centering and direction calculation)
    let base_x1 = state.get_x(from);
    let base_x2 = state.get_x(to);

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
    let is_filled = matches!(arrow.head, ArrowHead::Filled);

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

    // WSD: self-messages have a reduced pre-gap
    if is_self {
        state.current_y -= SELF_MESSAGE_PRE_GAP_REDUCTION;
    }

    let y = state.current_y;
    let has_label_text = lines.iter().any(|line| !line.trim().is_empty());

    // Calculate activation-aware arrow endpoints
    let going_right = base_x2 > base_x1;
    let x1 = state.get_arrow_start_x(from, y, going_right);
    let x2 = state.get_arrow_end_x(to, y, !going_right);

    // Open message group
    writeln!(svg, r#"<g class="message">"#).unwrap();

    if is_self {
        // Self message - loop back
        let loop_width = 40.0;
        let text_block_height = lines.len() as f64 * line_height;
        // WSD: loop height equals text block height, no extra padding
        let loop_height = text_block_height.max(25.0);
        let arrow_end_x = x1;
        let arrow_end_y = y + loop_height;
        // Arrowhead points left (PI radians)
        let direction = std::f64::consts::PI;
        let arrow_points = arrowhead_points(arrow_end_x, arrow_end_y, direction);

        writeln!(
            svg,
            r#"  <path d="M {x1} {y} L {x2} {y} L {x2} {y2} L {arrow_x} {y2}" class="{cls}"/>"#,
            x1 = x1,
            y = y,
            x2 = x1 + loop_width,
            y2 = y + loop_height,
            arrow_x = arrow_end_x + ARROWHEAD_SIZE,
            cls = line_class
        )
        .unwrap();

        // Draw arrowhead as polygon or polyline
        if is_filled {
            writeln!(
                svg,
                r#"  <polygon points="{points}" class="arrowhead"/>"#,
                points = arrow_points
            )
            .unwrap();
        } else {
            writeln!(
                svg,
                r#"  <polyline points="{points}" class="arrowhead-open"/>"#,
                points = arrow_points
            )
            .unwrap();
        }

        // Text - multiline support
        let text_x = x1 + loop_width + 5.0;
        let max_width = lines
            .iter()
            .map(|line| estimate_message_width(line, state.config.font_size))
            .fold(0.0, f64::max);
        let top_line_y = y + 4.0 + 0.5 * line_height;
        let bottom_line_y = y + 4.0 + (lines.len() as f64 - 0.5) * line_height;
        let label_y_min = top_line_y - line_height * MESSAGE_LABEL_ASCENT_FACTOR;
        let label_y_max = bottom_line_y + line_height * MESSAGE_LABEL_DESCENT_FACTOR;
        let label_x_min = text_x;
        let label_x_max = text_x + max_width;
        let label_offset = if has_label_text {
            let step = line_height * MESSAGE_LABEL_COLLISION_STEP_RATIO;
            state.reserve_message_label(label_x_min, label_x_max, label_y_min, label_y_max, step)
        } else {
            0.0
        };
        for (i, line) in lines.iter().enumerate() {
            let line_y = y + 4.0 + (i as f64 + 0.5) * line_height + label_offset;
            writeln!(
                svg,
                r#"  <text x="{x}" y="{y}" class="message-text">{t}</text>"#,
                x = text_x,
                y = line_y,
                t = escape_xml(line)
            )
            .unwrap();
        }

        // Close message group
        writeln!(svg, r#"</g>"#).unwrap();

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

        // Text is centered between lifelines (not activation bar edges)
        let text_x = (base_x1 + base_x2) / 2.0;
        let text_y = (y + y2) / 2.0 - 6.0;  // WSD: label slightly above arrow

        // Calculate arrowhead direction and shorten line to not overlap with arrowhead
        let direction = arrow_direction(x1, y, x2, y2);
        let arrow_points = arrowhead_points(x2, y2, direction);

        // Shorten the line so it doesn't overlap with the arrowhead
        let line_end_x = x2 - ARROWHEAD_SIZE * direction.cos();
        let line_end_y = y2 - ARROWHEAD_SIZE * direction.sin();

        // Draw arrow line (slanted if delay)
        writeln!(
            svg,
            r#"  <line x1="{x1}" y1="{y1}" x2="{lx2}" y2="{ly2}" class="{cls}"/>"#,
            x1 = x1,
            y1 = y,
            lx2 = line_end_x,
            ly2 = line_end_y,
            cls = line_class
        )
        .unwrap();

        // Draw arrowhead as polygon or polyline
        if is_filled {
            writeln!(
                svg,
                r#"  <polygon points="{points}" class="arrowhead"/>"#,
                points = arrow_points
            )
            .unwrap();
        } else {
            writeln!(
                svg,
                r#"  <polyline points="{points}" class="arrowhead-open"/>"#,
                points = arrow_points
            )
            .unwrap();
        }

        // Text with multiline support (positioned at midpoint of slanted line)
        let max_width = lines
            .iter()
            .map(|line| estimate_message_width(line, state.config.font_size))
            .fold(0.0, f64::max);
        let top_line_y = text_y - (lines.len() as f64 - 1.0) * line_height;
        let bottom_line_y = text_y;
        let label_offset = if has_label_text {
            let label_y_min = top_line_y - line_height * MESSAGE_LABEL_ASCENT_FACTOR;
            let label_y_max = bottom_line_y + line_height * MESSAGE_LABEL_DESCENT_FACTOR;
            let label_x_min = text_x - max_width / 2.0;
            let label_x_max = text_x + max_width / 2.0;
            let step = line_height * MESSAGE_LABEL_COLLISION_STEP_RATIO;
            state.reserve_message_label(label_x_min, label_x_max, label_y_min, label_y_max, step)
        } else {
            0.0
        };
        // Calculate rotation angle for delayed messages (slanted arrow)
        let rotation = if delay_offset > 0.0 {
            let dx = x2 - x1;
            let dy = delay_offset;
            let angle_rad = dy.atan2(dx.abs());
            let angle_deg = angle_rad.to_degrees();
            // Rotate in the direction of the arrow
            if dx < 0.0 { -angle_deg } else { angle_deg }
        } else {
            0.0
        };

        for (i, line) in lines.iter().enumerate() {
            let line_y = text_y - (lines.len() - 1 - i) as f64 * line_height + label_offset;
            if rotation.abs() > 0.1 {
                // Apply rotation transform for delayed messages
                writeln!(
                    svg,
                    r#"  <text x="{x}" y="{y}" class="message-text" text-anchor="middle" transform="rotate({rot},{cx},{cy})">{t}</text>"#,
                    x = text_x,
                    y = line_y,
                    rot = rotation,
                    cx = text_x,
                    cy = line_y,
                    t = escape_xml(line)
                )
                .unwrap();
            } else {
                writeln!(
                    svg,
                    r#"  <text x="{x}" y="{y}" class="message-text" text-anchor="middle">{t}</text>"#,
                    x = text_x,
                    y = line_y,
                    t = escape_xml(line)
                )
                .unwrap();
            }
        }

        // Close message group
        writeln!(svg, r#"</g>"#).unwrap();

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

    // ノートサイズ計算（4隅同じパディング）
    let max_line_len = lines.iter().map(|l| l.chars().count()).max().unwrap_or(5);
    let text_width = max_line_len as f64 * NOTE_CHAR_WIDTH;
    let content_width = (NOTE_PADDING * 2.0 + text_width).max(NOTE_MIN_WIDTH);
    let note_height = NOTE_PADDING * 2.0 + lines.len() as f64 * line_height;

    let (x, note_width, text_anchor) = match position {
        NotePosition::Left => {
            let px = state.get_x(&participants[0]);
            // ノート右端 = px - NOTE_MARGIN
            let x = (px - NOTE_MARGIN - content_width).max(state.config.padding);
            (x, content_width, "start")
        }
        NotePosition::Right => {
            let px = state.get_x(&participants[0]);
            // ノート左端 = px + NOTE_MARGIN
            (px + NOTE_MARGIN, content_width, "start")
        }
        NotePosition::Over => {
            if participants.len() == 1 {
                let px = state.get_x(&participants[0]);
                // ライフライン中心に配置
                let x = (px - content_width / 2.0).max(state.config.padding);
                (x, content_width, "middle")
            } else {
                // 複数参加者にまたがる
                let x1 = state.get_x(&participants[0]);
                let x2 = state.get_x(participants.last().unwrap());
                let span_width = (x2 - x1).abs() + NOTE_MARGIN * 2.0;
                let w = span_width.max(content_width);
                let x = (x1 - NOTE_MARGIN).max(state.config.padding);
                (x, w, "middle")
            }
        }
    };

    let y = state.current_y;
    let fold_size = NOTE_FOLD_SIZE;

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

    writeln!(svg, r#"<path d="{path}" class="note"/>"#, path = note_path).unwrap();

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
        r##"<path d="{path}" fill="none" stroke="{stroke}" stroke-width="1"/>"##,
        path = fold_path,
        stroke = theme.note_stroke
    )
    .unwrap();

    // テキスト位置（4隅同じパディング使用）
    let text_x = match text_anchor {
        "middle" => x + note_width / 2.0,
        _ => x + NOTE_PADDING,
    };
    let text_anchor_attr = if *position == NotePosition::Over { "middle" } else { "start" };

    for (i, line) in lines.iter().enumerate() {
        let text_y = y + NOTE_PADDING + (i as f64 + 0.8) * line_height;
        writeln!(
            svg,
            r#"<text x="{x}" y="{y}" class="note-text" text-anchor="{anchor}">{t}</text>"#,
            x = text_x,
            y = text_y,
            anchor = text_anchor_attr,
            t = escape_xml(line)
        )
        .unwrap();
    }

    // 要素間余白を追加
    state.current_y += note_height.max(state.config.row_height) + ROW_SPACING;
}

/// Render a state box (rounded rectangle)
fn render_state(svg: &mut String, state: &mut RenderState, participants: &[String], text: &str) {
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
        let w = (max_line_len as f64 * 8.0 + state.config.note_padding * 2.0 + notch_size * 2.0)
            .max(100.0);
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

        // Calculate arrowhead
        let direction = arrow_direction(from_x, arrow_y, to_x, arrow_y);
        let arrow_points = arrowhead_points(to_x, arrow_y, direction);
        let line_end_x = to_x - ARROWHEAD_SIZE * direction.cos();

        // Draw arrow line
        writeln!(
            svg,
            r##"<line x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" class="message"/>"##,
            x1 = from_x,
            y = arrow_y,
            x2 = line_end_x
        )
        .unwrap();

        // Draw arrowhead
        writeln!(
            svg,
            r#"<polygon points="{points}" class="arrowhead"/>"#,
            points = arrow_points
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

        // Calculate arrowhead
        let direction = arrow_direction(from_x, arrow_y, to_x, arrow_y);
        let arrow_points = arrowhead_points(to_x, arrow_y, direction);
        let line_end_x = to_x - ARROWHEAD_SIZE * direction.cos();

        // Draw dashed arrow line (response style)
        writeln!(
            svg,
            r##"<line x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" class="message-dashed"/>"##,
            x1 = from_x,
            y = arrow_y,
            x2 = line_end_x
        )
        .unwrap();

        // Draw arrowhead
        writeln!(
            svg,
            r#"<polygon points="{points}" class="arrowhead"/>"#,
            points = arrow_points
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
fn render_description(svg: &mut String, state: &mut RenderState, text: &str) {
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
            BLOCK_GAP
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
        // else線の前にパディング（collect_block_backgroundsと同じ）
        state.current_y += block_else_before(&state.config, depth);
        // else線の後にパディング
        state.current_y += block_else_after(&state.config, depth);
        render_items(svg, state, else_items, depth + 1);
        state.pop_else_return_pending();
    }

    // ブロック下端 = 現在のY位置 + フッターパディング
    // （メッセージがブロック外にはみ出ないように）
    let end_y = state.current_y + block_footer_padding(&state.config, depth);

    // Set current_y to end of block + margin
    state.current_y = end_y + state.config.row_height;

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
