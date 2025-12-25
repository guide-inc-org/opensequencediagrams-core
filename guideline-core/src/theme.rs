//! Theme definitions for sequence diagrams

/// Participant box shape
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParticipantShape {
    /// Rectangle with square corners
    #[default]
    Rectangle,
    /// Rectangle with rounded corners
    RoundedRect,
    /// Circle/ellipse
    Circle,
}

/// Line style for lifelines
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LifelineStyle {
    /// Dashed line (default)
    #[default]
    Dashed,
    /// Solid line
    Solid,
}

/// Theme colors and styles
#[derive(Debug, Clone)]
pub struct Theme {
    /// Theme name
    pub name: String,
    /// Background color
    pub background: String,
    /// Participant box fill color
    pub participant_fill: String,
    /// Participant box stroke color
    pub participant_stroke: String,
    /// Participant text color
    pub participant_text: String,
    /// Participant box shape
    pub participant_shape: ParticipantShape,
    /// Lifeline color
    pub lifeline_color: String,
    /// Lifeline style
    pub lifeline_style: LifelineStyle,
    /// Message line color
    pub message_color: String,
    /// Message text color
    pub message_text_color: String,
    /// Note background color
    pub note_fill: String,
    /// Note stroke color
    pub note_stroke: String,
    /// Note text color
    pub note_text_color: String,
    /// Activation box fill color
    pub activation_fill: String,
    /// Activation box stroke color
    pub activation_stroke: String,
    /// Block stroke color
    pub block_stroke: String,
    /// Block label background
    pub block_label_fill: String,
    /// Block background fill (inside the block area)
    pub block_fill: String,
    /// Font family
    pub font_family: String,
    /// Actor head fill color
    pub actor_fill: String,
    /// Actor stroke color
    pub actor_stroke: String,
}

impl Default for Theme {
    fn default() -> Self {
        Self::default_theme()
    }
}

impl Theme {
    /// Default theme (simple black and white)
    pub fn default_theme() -> Self {
        Self {
            name: "default".to_string(),
            background: "#fff".to_string(),
            participant_fill: "#fff".to_string(),
            participant_stroke: "#333".to_string(),
            participant_text: "#000".to_string(),
            participant_shape: ParticipantShape::Rectangle,
            lifeline_color: "#999".to_string(),
            lifeline_style: LifelineStyle::Dashed,
            message_color: "#333".to_string(),
            message_text_color: "#000".to_string(),
            note_fill: "#ffffcc".to_string(),
            note_stroke: "#333".to_string(),
            note_text_color: "#000".to_string(),
            activation_fill: "#e0e0e0".to_string(),
            activation_stroke: "#333".to_string(),
            block_stroke: "#666".to_string(),
            block_label_fill: "#fff".to_string(),
            block_fill: "rgba(240, 240, 240, 0.6)".to_string(),
            font_family: "sans-serif".to_string(),
            actor_fill: "#fff".to_string(),
            actor_stroke: "#333".to_string(),
        }
    }

    /// Modern blue theme
    pub fn modern_blue() -> Self {
        Self {
            name: "modern-blue".to_string(),
            background: "#fff".to_string(),
            participant_fill: "#4a90d9".to_string(),
            participant_stroke: "#2a5a8a".to_string(),
            participant_text: "#fff".to_string(),
            participant_shape: ParticipantShape::RoundedRect,
            lifeline_color: "#4a90d9".to_string(),
            lifeline_style: LifelineStyle::Solid,
            message_color: "#333".to_string(),
            message_text_color: "#000".to_string(),
            note_fill: "#e8f4fd".to_string(),
            note_stroke: "#4a90d9".to_string(),
            note_text_color: "#000".to_string(),
            activation_fill: "#b8d4f0".to_string(),
            activation_stroke: "#4a90d9".to_string(),
            block_stroke: "#4a90d9".to_string(),
            block_label_fill: "#e8f4fd".to_string(),
            block_fill: "rgba(74, 144, 217, 0.1)".to_string(),
            font_family: "sans-serif".to_string(),
            actor_fill: "#4a90d9".to_string(),
            actor_stroke: "#2a5a8a".to_string(),
        }
    }

    /// Modern green theme
    pub fn modern_green() -> Self {
        Self {
            name: "modern-green".to_string(),
            background: "#fff".to_string(),
            participant_fill: "#2d8659".to_string(),
            participant_stroke: "#1a5c3a".to_string(),
            participant_text: "#fff".to_string(),
            participant_shape: ParticipantShape::RoundedRect,
            lifeline_color: "#2d8659".to_string(),
            lifeline_style: LifelineStyle::Dashed,
            message_color: "#2d8659".to_string(),
            message_text_color: "#000".to_string(),
            note_fill: "#e8f5e9".to_string(),
            note_stroke: "#2d8659".to_string(),
            note_text_color: "#000".to_string(),
            activation_fill: "#a5d6a7".to_string(),
            activation_stroke: "#2d8659".to_string(),
            block_stroke: "#2d8659".to_string(),
            block_label_fill: "#e8f5e9".to_string(),
            block_fill: "rgba(45, 134, 89, 0.1)".to_string(),
            font_family: "sans-serif".to_string(),
            actor_fill: "#2d8659".to_string(),
            actor_stroke: "#1a5c3a".to_string(),
        }
    }

    /// Rose/pink theme with circles
    pub fn rose() -> Self {
        Self {
            name: "rose".to_string(),
            background: "#fff".to_string(),
            participant_fill: "#c2185b".to_string(),
            participant_stroke: "#880e4f".to_string(),
            participant_text: "#fff".to_string(),
            participant_shape: ParticipantShape::Circle,
            lifeline_color: "#c2185b".to_string(),
            lifeline_style: LifelineStyle::Solid,
            message_color: "#c2185b".to_string(),
            message_text_color: "#000".to_string(),
            note_fill: "#fce4ec".to_string(),
            note_stroke: "#c2185b".to_string(),
            note_text_color: "#000".to_string(),
            activation_fill: "#f48fb1".to_string(),
            activation_stroke: "#c2185b".to_string(),
            block_stroke: "#c2185b".to_string(),
            block_label_fill: "#fce4ec".to_string(),
            block_fill: "rgba(194, 24, 91, 0.1)".to_string(),
            font_family: "sans-serif".to_string(),
            actor_fill: "#c2185b".to_string(),
            actor_stroke: "#880e4f".to_string(),
        }
    }

    /// Napkin/sketch style theme
    pub fn napkin() -> Self {
        Self {
            name: "napkin".to_string(),
            background: "#fff".to_string(),
            participant_fill: "#fff".to_string(),
            participant_stroke: "#333".to_string(),
            participant_text: "#000".to_string(),
            participant_shape: ParticipantShape::Rectangle,
            lifeline_color: "#666".to_string(),
            lifeline_style: LifelineStyle::Dashed,
            message_color: "#333".to_string(),
            message_text_color: "#000".to_string(),
            note_fill: "#fff".to_string(),
            note_stroke: "#333".to_string(),
            note_text_color: "#000".to_string(),
            activation_fill: "#f5f5f5".to_string(),
            activation_stroke: "#333".to_string(),
            block_stroke: "#333".to_string(),
            block_label_fill: "#fff".to_string(),
            block_fill: "rgba(200, 200, 200, 0.3)".to_string(),
            font_family: "'Comic Sans MS', 'Chalkboard', cursive".to_string(),
            actor_fill: "#fff".to_string(),
            actor_stroke: "#333".to_string(),
        }
    }

    /// Earth tones theme
    pub fn earth() -> Self {
        Self {
            name: "earth".to_string(),
            background: "#faf8f5".to_string(),
            participant_fill: "#8d6e63".to_string(),
            participant_stroke: "#5d4037".to_string(),
            participant_text: "#fff".to_string(),
            participant_shape: ParticipantShape::RoundedRect,
            lifeline_color: "#8d6e63".to_string(),
            lifeline_style: LifelineStyle::Dashed,
            message_color: "#5d4037".to_string(),
            message_text_color: "#3e2723".to_string(),
            note_fill: "#efebe9".to_string(),
            note_stroke: "#8d6e63".to_string(),
            note_text_color: "#3e2723".to_string(),
            activation_fill: "#bcaaa4".to_string(),
            activation_stroke: "#8d6e63".to_string(),
            block_stroke: "#8d6e63".to_string(),
            block_label_fill: "#efebe9".to_string(),
            block_fill: "rgba(141, 110, 99, 0.1)".to_string(),
            font_family: "Georgia, serif".to_string(),
            actor_fill: "#8d6e63".to_string(),
            actor_stroke: "#5d4037".to_string(),
        }
    }

    /// Plain monochrome theme
    pub fn plain() -> Self {
        Self {
            name: "plain".to_string(),
            background: "#fff".to_string(),
            participant_fill: "#fff".to_string(),
            participant_stroke: "#000".to_string(),
            participant_text: "#000".to_string(),
            participant_shape: ParticipantShape::Rectangle,
            lifeline_color: "#000".to_string(),
            lifeline_style: LifelineStyle::Solid,
            message_color: "#000".to_string(),
            message_text_color: "#000".to_string(),
            note_fill: "#fff".to_string(),
            note_stroke: "#000".to_string(),
            note_text_color: "#000".to_string(),
            activation_fill: "#ccc".to_string(),
            activation_stroke: "#000".to_string(),
            block_stroke: "#000".to_string(),
            block_label_fill: "#fff".to_string(),
            block_fill: "rgba(200, 200, 200, 0.3)".to_string(),
            font_family: "sans-serif".to_string(),
            actor_fill: "#fff".to_string(),
            actor_stroke: "#000".to_string(),
        }
    }

    /// Mellow/pastel theme with circles
    pub fn mellow() -> Self {
        Self {
            name: "mellow".to_string(),
            background: "#fff".to_string(),
            participant_fill: "#a8e6cf".to_string(),
            participant_stroke: "#56ab91".to_string(),
            participant_text: "#2d5a4a".to_string(),
            participant_shape: ParticipantShape::Circle,
            lifeline_color: "#56ab91".to_string(),
            lifeline_style: LifelineStyle::Dashed,
            message_color: "#56ab91".to_string(),
            message_text_color: "#2d5a4a".to_string(),
            note_fill: "#dcedc1".to_string(),
            note_stroke: "#56ab91".to_string(),
            note_text_color: "#2d5a4a".to_string(),
            activation_fill: "#a8e6cf".to_string(),
            activation_stroke: "#56ab91".to_string(),
            block_stroke: "#56ab91".to_string(),
            block_label_fill: "#dcedc1".to_string(),
            block_fill: "rgba(86, 171, 145, 0.1)".to_string(),
            font_family: "sans-serif".to_string(),
            actor_fill: "#a8e6cf".to_string(),
            actor_stroke: "#56ab91".to_string(),
        }
    }

    /// Blue outline theme
    pub fn blue_outline() -> Self {
        Self {
            name: "blue-outline".to_string(),
            background: "#fff".to_string(),
            participant_fill: "#fff".to_string(),
            participant_stroke: "#1976d2".to_string(),
            participant_text: "#1976d2".to_string(),
            participant_shape: ParticipantShape::Rectangle,
            lifeline_color: "#1976d2".to_string(),
            lifeline_style: LifelineStyle::Dashed,
            message_color: "#1976d2".to_string(),
            message_text_color: "#1976d2".to_string(),
            note_fill: "#e3f2fd".to_string(),
            note_stroke: "#1976d2".to_string(),
            note_text_color: "#1976d2".to_string(),
            activation_fill: "#bbdefb".to_string(),
            activation_stroke: "#1976d2".to_string(),
            block_stroke: "#1976d2".to_string(),
            block_label_fill: "#e3f2fd".to_string(),
            block_fill: "rgba(25, 118, 210, 0.1)".to_string(),
            font_family: "sans-serif".to_string(),
            actor_fill: "#fff".to_string(),
            actor_stroke: "#1976d2".to_string(),
        }
    }

    /// Orange/yellow warm theme
    pub fn warm() -> Self {
        Self {
            name: "warm".to_string(),
            background: "#fffbf0".to_string(),
            participant_fill: "#ffcc80".to_string(),
            participant_stroke: "#ef6c00".to_string(),
            participant_text: "#000".to_string(),
            participant_shape: ParticipantShape::RoundedRect,
            lifeline_color: "#ef6c00".to_string(),
            lifeline_style: LifelineStyle::Dashed,
            message_color: "#ef6c00".to_string(),
            message_text_color: "#000".to_string(),
            note_fill: "#fff3e0".to_string(),
            note_stroke: "#ef6c00".to_string(),
            note_text_color: "#000".to_string(),
            activation_fill: "#ffcc80".to_string(),
            activation_stroke: "#ef6c00".to_string(),
            block_stroke: "#ef6c00".to_string(),
            block_label_fill: "#fff3e0".to_string(),
            block_fill: "rgba(239, 108, 0, 0.1)".to_string(),
            font_family: "sans-serif".to_string(),
            actor_fill: "#ffcc80".to_string(),
            actor_stroke: "#ef6c00".to_string(),
        }
    }

    /// Gray professional theme
    pub fn gray() -> Self {
        Self {
            name: "gray".to_string(),
            background: "#fafafa".to_string(),
            participant_fill: "#757575".to_string(),
            participant_stroke: "#424242".to_string(),
            participant_text: "#fff".to_string(),
            participant_shape: ParticipantShape::Rectangle,
            lifeline_color: "#757575".to_string(),
            lifeline_style: LifelineStyle::Solid,
            message_color: "#424242".to_string(),
            message_text_color: "#212121".to_string(),
            note_fill: "#eeeeee".to_string(),
            note_stroke: "#757575".to_string(),
            note_text_color: "#212121".to_string(),
            activation_fill: "#bdbdbd".to_string(),
            activation_stroke: "#757575".to_string(),
            block_stroke: "#757575".to_string(),
            block_label_fill: "#eeeeee".to_string(),
            block_fill: "rgba(117, 117, 117, 0.1)".to_string(),
            font_family: "sans-serif".to_string(),
            actor_fill: "#757575".to_string(),
            actor_stroke: "#424242".to_string(),
        }
    }

    /// Get theme by name
    pub fn by_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "default" => Some(Self::default_theme()),
            "modern-blue" | "modernblue" | "blue" => Some(Self::modern_blue()),
            "modern-green" | "moderngreen" | "green" => Some(Self::modern_green()),
            "rose" | "pink" => Some(Self::rose()),
            "napkin" | "sketch" => Some(Self::napkin()),
            "earth" | "brown" => Some(Self::earth()),
            "plain" | "monochrome" => Some(Self::plain()),
            "mellow" | "pastel" => Some(Self::mellow()),
            "blue-outline" | "blueoutline" => Some(Self::blue_outline()),
            "warm" | "orange" => Some(Self::warm()),
            "gray" | "grey" => Some(Self::gray()),
            _ => None,
        }
    }

    /// List all available theme names
    pub fn available_themes() -> Vec<&'static str> {
        vec![
            "default",
            "modern-blue",
            "modern-green",
            "rose",
            "napkin",
            "earth",
            "plain",
            "mellow",
            "blue-outline",
            "warm",
            "gray",
        ]
    }
}
