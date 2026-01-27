use std::any::Any;

use crate::state::{MixerSelection, OutputTarget, RackState};
use crate::ui::{Action, Color, Graphics, InputEvent, KeyCode, Keymap, Pane, Rect, Style};

const STRIP_WIDTH: u16 = 10;
const NUM_VISIBLE_CHANNELS: usize = 8;
const NUM_VISIBLE_BUSES: usize = 2;

pub struct MixerPane {
    keymap: Keymap,
}

impl MixerPane {
    pub fn new() -> Self {
        Self {
            keymap: Keymap::new()
                .bind_key(KeyCode::Escape, "back", "Return to rack")
                .bind_key(KeyCode::Left, "prev", "Previous channel")
                .bind_key(KeyCode::Right, "next", "Next channel")
                .bind_key(KeyCode::Home, "first", "First channel")
                .bind_key(KeyCode::End, "last", "Last channel")
                .bind_key(KeyCode::Up, "level_up", "Increase level")
                .bind_key(KeyCode::Down, "level_down", "Decrease level")
                .bind_key(KeyCode::PageUp, "level_up_big", "Increase level +10%")
                .bind_key(KeyCode::PageDown, "level_down_big", "Decrease level -10%")
                .bind('m', "mute", "Toggle mute")
                .bind('s', "solo", "Toggle solo")
                .bind('o', "output", "Cycle output target")
                .bind('O', "output_rev", "Cycle output target backwards")
                .bind_key(KeyCode::Tab, "section", "Cycle section")
                .bind('h', "home", "Home screen"),
        }
    }

    /// Convert linear level (0.0-1.0) to dB string
    fn level_to_db(level: f32) -> String {
        if level <= 0.0 {
            "-∞dB".to_string()
        } else {
            let db = 20.0 * level.log10();
            format!("{:+.0}dB", db.max(-99.0))
        }
    }

    /// Render a horizontal level meter
    fn render_meter(level: f32, width: usize) -> String {
        let filled = ((level * width as f32) as usize).min(width);
        let empty = width - filled;
        format!("{}{}", "█".repeat(filled), " ".repeat(empty))
    }

    /// Format output target as string
    fn format_output(target: OutputTarget) -> &'static str {
        match target {
            OutputTarget::Master => "ST>MST",
            OutputTarget::Bus(1) => "ST>B1",
            OutputTarget::Bus(2) => "ST>B2",
            OutputTarget::Bus(3) => "ST>B3",
            OutputTarget::Bus(4) => "ST>B4",
            OutputTarget::Bus(5) => "ST>B5",
            OutputTarget::Bus(6) => "ST>B6",
            OutputTarget::Bus(7) => "ST>B7",
            OutputTarget::Bus(8) => "ST>B8",
            OutputTarget::Bus(_) => "ST>??",
        }
    }
}

impl Default for MixerPane {
    fn default() -> Self {
        Self::new()
    }
}

impl Pane for MixerPane {
    fn id(&self) -> &'static str {
        "mixer"
    }

    fn handle_input(&mut self, event: InputEvent) -> Action {
        match self.keymap.lookup(&event) {
            Some("back") => Action::SwitchPane("rack"),
            Some("prev") => Action::MixerMove(-1),
            Some("next") => Action::MixerMove(1),
            Some("first") => Action::MixerJump(1),
            Some("last") => Action::MixerJump(-1),
            Some("level_up") => Action::MixerAdjustLevel(0.05),
            Some("level_down") => Action::MixerAdjustLevel(-0.05),
            Some("level_up_big") => Action::MixerAdjustLevel(0.10),
            Some("level_down_big") => Action::MixerAdjustLevel(-0.10),
            Some("mute") => Action::MixerToggleMute,
            Some("solo") => Action::MixerToggleSolo,
            Some("output") => Action::MixerCycleOutput,
            Some("output_rev") => Action::MixerCycleOutputReverse,
            Some("section") => Action::MixerCycleSection,
            Some("home") => Action::SwitchPane("home"),
            _ => Action::None,
        }
    }

    fn render(&self, g: &mut dyn Graphics) {
        let (width, height) = g.size();
        let rect = Rect::centered(width, height, 80, 14);

        g.set_style(Style::new().fg(Color::BLACK));
        g.draw_box(rect, Some(" MIXER "));

        g.put_str(rect.x + 2, rect.y + 2, "Mixer pane - use MixerPane::render_with_state");
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl MixerPane {
    /// Calculate scroll offset to keep selection visible
    fn calc_scroll_offset(selected: usize, total: usize, visible: usize) -> usize {
        if selected >= visible {
            // Keep selection at the end of visible range
            (selected - visible + 1).min(total.saturating_sub(visible))
        } else {
            0
        }
    }

    /// Render the mixer with access to rack state
    pub fn render_with_state(&self, g: &mut dyn Graphics, rack: &RackState) {
        let (width, height) = g.size();

        // Calculate box width based on content
        // 8 channels + separator + 2 buses + separator + master
        let box_width = (NUM_VISIBLE_CHANNELS as u16 * STRIP_WIDTH) + 2 +
                        (NUM_VISIBLE_BUSES as u16 * STRIP_WIDTH) + 2 +
                        STRIP_WIDTH + 4;
        let box_height = 15;
        let rect = Rect::centered(width, height, box_width, box_height);

        g.set_style(Style::new().fg(Color::CYAN));
        g.draw_box(rect, Some(" MIXER "));

        let base_x = rect.x + 2;
        let base_y = rect.y + 2;

        // Row positions (relative to base_y)
        let label_row = 0;
        let name_row = 1;
        let meter_row = 2;
        let level_row = 3;
        let indicator_row = 4;
        let output_row = 6;

        // Calculate scroll offsets based on selection
        let channel_scroll = match rack.mixer.selection {
            MixerSelection::Channel(id) => {
                Self::calc_scroll_offset((id - 1) as usize, rack.mixer.channels.len(), NUM_VISIBLE_CHANNELS)
            }
            _ => 0,
        };

        let bus_scroll = match rack.mixer.selection {
            MixerSelection::Bus(id) => {
                Self::calc_scroll_offset((id - 1) as usize, rack.mixer.buses.len(), NUM_VISIBLE_BUSES)
            }
            _ => 0,
        };

        let mut x = base_x;

        // Render channels with scroll offset
        for i in 0..NUM_VISIBLE_CHANNELS {
            let ch_idx = channel_scroll + i;
            if ch_idx >= rack.mixer.channels.len() {
                break;
            }
            let ch = &rack.mixer.channels[ch_idx];
            let is_selected = matches!(rack.mixer.selection, MixerSelection::Channel(id) if id == ch.id);

            // Get module name
            let module_name = ch.module_id
                .and_then(|id| rack.modules.get(&id))
                .map(|m| m.name.as_str())
                .unwrap_or("---");

            self.render_channel_strip(
                g, x, base_y,
                &format!("CH{}", ch.id),
                module_name,
                ch.level,
                ch.mute,
                ch.solo,
                Some(ch.output_target),
                is_selected,
                label_row, name_row, meter_row, level_row, indicator_row, output_row,
            );

            x += STRIP_WIDTH;
        }

        // Vertical separator before buses
        g.set_style(Style::new().fg(Color::PURPLE));
        for row in 0..=output_row {
            g.put_char(x, base_y + row, '│');
        }
        x += 2;

        // Render buses with scroll offset
        for i in 0..NUM_VISIBLE_BUSES {
            let bus_idx = bus_scroll + i;
            if bus_idx >= rack.mixer.buses.len() {
                break;
            }
            let bus = &rack.mixer.buses[bus_idx];
            let is_selected = matches!(rack.mixer.selection, MixerSelection::Bus(id) if id == bus.id);

            self.render_channel_strip(
                g, x, base_y,
                &format!("BUS{}", bus.id),
                &bus.name,
                bus.level,
                bus.mute,
                bus.solo,
                None, // Buses don't show output routing
                is_selected,
                label_row, name_row, meter_row, level_row, indicator_row, output_row,
            );

            x += STRIP_WIDTH;
        }

        // Vertical separator before master
        g.set_style(Style::new().fg(Color::GOLD));
        for row in 0..=output_row {
            g.put_char(x, base_y + row, '│');
        }
        x += 2;

        // Render master
        let is_master_selected = matches!(rack.mixer.selection, MixerSelection::Master);
        self.render_channel_strip(
            g, x, base_y,
            "MASTER",
            "",
            rack.mixer.master_level,
            rack.mixer.master_mute,
            false, // Master can't be soloed
            None,
            is_master_selected,
            label_row, name_row, meter_row, level_row, indicator_row, output_row,
        );

        // Scroll indicators row
        let scroll_y = base_y + output_row + 1;
        g.set_style(Style::new().fg(Color::DARK_GRAY));

        // Channel scroll indicator
        let total_channels = rack.mixer.channels.len();
        if total_channels > NUM_VISIBLE_CHANNELS {
            let ch_start = channel_scroll + 1;
            let ch_end = (channel_scroll + NUM_VISIBLE_CHANNELS).min(total_channels);
            let left_arrow = if channel_scroll > 0 { "‹" } else { " " };
            let right_arrow = if channel_scroll + NUM_VISIBLE_CHANNELS < total_channels { "›" } else { " " };
            let indicator = format!("{}{}-{}/{}{}", left_arrow, ch_start, ch_end, total_channels, right_arrow);
            g.put_str(base_x, scroll_y, &indicator);
        }

        // Bus scroll indicator (after channel section)
        let total_buses = rack.mixer.buses.len();
        if total_buses > NUM_VISIBLE_BUSES {
            let bus_section_x = base_x + (NUM_VISIBLE_CHANNELS as u16 * STRIP_WIDTH) + 2;
            let bus_start = bus_scroll + 1;
            let bus_end = (bus_scroll + NUM_VISIBLE_BUSES).min(total_buses);
            let left_arrow = if bus_scroll > 0 { "‹" } else { " " };
            let right_arrow = if bus_scroll + NUM_VISIBLE_BUSES < total_buses { "›" } else { " " };
            let indicator = format!("{}{}-{}/{}{}", left_arrow, bus_start, bus_end, total_buses, right_arrow);
            g.put_str(bus_section_x, scroll_y, &indicator);
        }

        // Help text at bottom
        let help_y = rect.y + rect.height - 2;
        g.set_style(Style::new().fg(Color::DARK_GRAY));
        g.put_str(
            base_x,
            help_y,
            "[←/→] Select   [↑/↓] Level   [M] Mute   [S] Solo   [o] Output   [ESC] Back",
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn render_channel_strip(
        &self,
        g: &mut dyn Graphics,
        x: u16,
        base_y: u16,
        label: &str,
        name: &str,
        level: f32,
        mute: bool,
        solo: bool,
        output: Option<OutputTarget>,
        selected: bool,
        label_row: u16,
        name_row: u16,
        meter_row: u16,
        level_row: u16,
        indicator_row: u16,
        output_row: u16,
    ) {
        let meter_width = (STRIP_WIDTH - 1) as usize;

        // Style for this strip
        let label_style = if selected {
            Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold()
        } else if label.starts_with("BUS") {
            Style::new().fg(Color::PURPLE).bold()
        } else if label == "MASTER" {
            Style::new().fg(Color::GOLD).bold()
        } else {
            Style::new().fg(Color::CYAN)
        };

        let text_style = if selected {
            Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG)
        } else {
            Style::new().fg(Color::DARK_GRAY)
        };

        // Label (CH1, BUS1, MASTER)
        g.set_style(label_style);
        g.put_str(x, base_y + label_row, label);

        // Module name (or bus name)
        g.set_style(text_style);
        let name_display: String = if name.is_empty() {
            String::new()
        } else {
            name.chars().take(meter_width).collect()
        };
        // Show "---" for empty channels
        let display = if name_display.is_empty() && label.starts_with("CH") {
            "---".to_string()
        } else {
            name_display
        };
        g.put_str(x, base_y + name_row, &display);

        // Horizontal meter with color gradient
        let meter_color = if level > 0.9 {
            Color::METER_HIGH
        } else if level > 0.7 {
            Color::METER_MID
        } else {
            Color::METER_LOW
        };
        let meter_style = if selected {
            Style::new().fg(meter_color).bg(Color::SELECTION_BG)
        } else {
            Style::new().fg(meter_color)
        };
        g.set_style(meter_style);
        let meter = Self::render_meter(level, meter_width);
        g.put_str(x, base_y + meter_row, &meter);

        // Level in dB
        let db_style = if selected {
            Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG)
        } else {
            Style::new().fg(Color::SKY_BLUE)
        };
        g.set_style(db_style);
        let db_str = Self::level_to_db(level);
        g.put_str(x, base_y + level_row, &db_str);

        // Status indicator (mute/solo/normal)
        let (indicator, indicator_style) = if mute {
            ("M", Style::new().fg(Color::MUTE_COLOR).bold())
        } else if solo {
            ("S", Style::new().fg(Color::SOLO_COLOR).bold())
        } else {
            ("●", Style::new().fg(Color::DARK_GRAY))
        };
        g.set_style(indicator_style);
        g.put_str(x + (meter_width as u16 / 2), base_y + indicator_row, indicator);

        // Output routing (channels only)
        if let Some(target) = output {
            let routing_style = if selected {
                Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG)
            } else {
                Style::new().fg(Color::TEAL)
            };
            g.set_style(routing_style);
            g.put_str(x, base_y + output_row, Self::format_output(target));
        }
    }
}
