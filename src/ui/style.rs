/// RGB Color representation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    // Basic colors
    pub const BLACK: Color = Color::new(0, 0, 0);
    pub const WHITE: Color = Color::new(255, 255, 255);
    pub const RED: Color = Color::new(255, 0, 0);
    pub const GREEN: Color = Color::new(0, 255, 0);
    pub const BLUE: Color = Color::new(0, 0, 255);
    pub const YELLOW: Color = Color::new(255, 255, 0);
    pub const CYAN: Color = Color::new(0, 255, 255);
    pub const MAGENTA: Color = Color::new(255, 0, 255);
    pub const GRAY: Color = Color::new(128, 128, 128);
    pub const DARK_GRAY: Color = Color::new(64, 64, 64);

    // DAW accent colors
    pub const ORANGE: Color = Color::new(255, 165, 0);
    pub const PINK: Color = Color::new(255, 105, 180);
    pub const PURPLE: Color = Color::new(147, 112, 219);
    pub const LIME: Color = Color::new(50, 205, 50);
    pub const TEAL: Color = Color::new(0, 128, 128);
    pub const CORAL: Color = Color::new(255, 127, 80);
    pub const SKY_BLUE: Color = Color::new(135, 206, 235);
    pub const GOLD: Color = Color::new(255, 215, 0);

    // Module type colors
    pub const MIDI_COLOR: Color = Color::new(255, 100, 160);   // Magenta - MIDI/note source
    pub const OSC_COLOR: Color = Color::new(100, 180, 255);    // Blue - oscillators
    pub const FILTER_COLOR: Color = Color::new(255, 140, 90);  // Orange - filters
    pub const ENV_COLOR: Color = Color::new(180, 130, 255);    // Purple - envelopes
    pub const LFO_COLOR: Color = Color::new(130, 255, 180);    // Mint - LFOs
    pub const FX_COLOR: Color = Color::new(255, 180, 220);     // Pink - effects
    pub const OUTPUT_COLOR: Color = Color::new(255, 220, 100); // Gold - output

    // Port type colors
    pub const AUDIO_PORT: Color = Color::new(80, 200, 255);    // Cyan - audio
    pub const CONTROL_PORT: Color = Color::new(100, 255, 150); // Green - control
    pub const GATE_PORT: Color = Color::new(255, 230, 80);     // Yellow - gate

    // Meter colors
    pub const METER_LOW: Color = Color::new(80, 220, 100);     // Green
    pub const METER_MID: Color = Color::new(255, 220, 50);     // Yellow
    pub const METER_HIGH: Color = Color::new(255, 80, 80);     // Red

    // UI colors
    pub const SELECTION_BG: Color = Color::new(60, 100, 180);  // Selection highlight
    pub const MUTE_COLOR: Color = Color::new(255, 100, 100);   // Muted state
    pub const SOLO_COLOR: Color = Color::new(255, 220, 80);    // Solo state
}

/// Text style with foreground, background, and attributes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
    pub underline: bool,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            fg: None,
            bg: None,
            bold: false,
            underline: false,
        }
    }
}

impl Style {
    pub const fn new() -> Self {
        Self {
            fg: None,
            bg: None,
            bold: false,
            underline: false,
        }
    }

    pub const fn fg(mut self, color: Color) -> Self {
        self.fg = Some(color);
        self
    }

    pub const fn bg(mut self, color: Color) -> Self {
        self.bg = Some(color);
        self
    }

    pub const fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    pub const fn underline(mut self) -> Self {
        self.underline = true;
        self
    }
}

/// Semantic color categories for theming
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticColor {
    Text,
    TextMuted,
    Selected,
    SelectedText,
    Border,
    BorderFocused,
    Background,
    Error,
    Warning,
    Success,
}

impl SemanticColor {
    /// Get the default color for this semantic category
    pub fn default_color(&self) -> Color {
        match self {
            SemanticColor::Text => Color::WHITE,
            SemanticColor::TextMuted => Color::GRAY,
            SemanticColor::Selected => Color::BLUE,
            SemanticColor::SelectedText => Color::WHITE,
            SemanticColor::Border => Color::GRAY,
            SemanticColor::BorderFocused => Color::WHITE,
            SemanticColor::Background => Color::BLACK,
            SemanticColor::Error => Color::RED,
            SemanticColor::Warning => Color::YELLOW,
            SemanticColor::Success => Color::GREEN,
        }
    }
}
