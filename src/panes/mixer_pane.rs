use std::any::Any;

use crate::state::{MixerSelection, OutputTarget, RackState};
use crate::ui::{Action, Color, Graphics, InputEvent, KeyCode, Keymap, Pane, Rect, Style};

const STRIP_WIDTH: u16 = 8;
const METER_HEIGHT: u16 = 12;
const NUM_VISIBLE_CHANNELS: usize = 8;
const NUM_VISIBLE_BUSES: usize = 2;

/// Block characters for vertical meter: ▁▂▃▄▅▆▇█ (U+2581–U+2588)
const BLOCK_CHARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

pub struct MixerPane {
    keymap: Keymap,
    send_target: Option<u8>, // 1-8: selected send bus for adjustment
}

impl MixerPane {
    pub fn new() -> Self {
        Self {
            keymap: Keymap::new()
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
                .bind('1', "send1", "Select send 1")
                .bind('2', "send2", "Select send 2")
                .bind('3', "send3", "Select send 3")
                .bind('4', "send4", "Select send 4")
                .bind('5', "send5", "Select send 5")
                .bind('6', "send6", "Select send 6")
                .bind('7', "send7", "Select send 7")
                .bind('8', "send8", "Select send 8")
                .bind('t', "send_toggle", "Toggle selected send")
                .bind_key(KeyCode::Escape, "clear_send", "Clear send selection"),
            send_target: None,
        }
    }

    /// Convert linear level (0.0-1.0) to dB string
    fn level_to_db(level: f32) -> String {
        if level <= 0.0 {
            "-∞".to_string()
        } else {
            let db = 20.0 * level.log10();
            format!("{:+.0}", db.max(-99.0))
        }
    }

    /// Get meter color for a given row position (0=bottom, height-1=top)
    fn meter_color(row: u16, height: u16) -> Color {
        let frac = row as f32 / height as f32;
        if frac > 0.85 {
            Color::METER_HIGH
        } else if frac > 0.6 {
            Color::METER_MID
        } else {
            Color::METER_LOW
        }
    }

    /// Render a vertical meter at position (x, top_y) going downward
    fn render_vertical_meter(g: &mut dyn Graphics, x: u16, top_y: u16, height: u16, level: f32) {
        let total_sub = height as f32 * 8.0;
        let filled_sub = (level * total_sub) as u16;

        for row in 0..height {
            // row 0 = top of meter, row height-1 = bottom
            let inverted_row = height - 1 - row;
            let y = top_y + row;

            let row_start = inverted_row * 8;
            let row_end = row_start + 8;

            let color = Self::meter_color(inverted_row, height);

            if filled_sub >= row_end {
                // Full block
                g.set_style(Style::new().fg(color));
                g.put_char(x, y, '█');
            } else if filled_sub > row_start {
                // Partial block
                let sub_level = (filled_sub - row_start) as usize;
                g.set_style(Style::new().fg(color));
                g.put_char(x, y, BLOCK_CHARS[sub_level.saturating_sub(1).min(7)]);
            } else {
                // Empty
                g.set_style(Style::new().fg(Color::DARK_GRAY));
                g.put_char(x, y, '·');
            }
        }
    }

    /// Format output target as short string
    fn format_output(target: OutputTarget) -> &'static str {
        match target {
            OutputTarget::Master => ">MST",
            OutputTarget::Bus(1) => ">B1",
            OutputTarget::Bus(2) => ">B2",
            OutputTarget::Bus(3) => ">B3",
            OutputTarget::Bus(4) => ">B4",
            OutputTarget::Bus(5) => ">B5",
            OutputTarget::Bus(6) => ">B6",
            OutputTarget::Bus(7) => ">B7",
            OutputTarget::Bus(8) => ">B8",
            OutputTarget::Bus(_) => ">??",
        }
    }
}

impl Default for MixerPane {
    fn default() -> Self {
        Self::new()
    }
}

impl MixerPane {
    pub fn send_target(&self) -> Option<u8> {
        self.send_target
    }
}

impl Pane for MixerPane {
    fn id(&self) -> &'static str {
        "mixer"
    }

    fn handle_input(&mut self, event: InputEvent) -> Action {
        match self.keymap.lookup(&event) {
            Some("prev") => { self.send_target = None; Action::MixerMove(-1) }
            Some("next") => { self.send_target = None; Action::MixerMove(1) }
            Some("first") => Action::MixerJump(1),
            Some("last") => Action::MixerJump(-1),
            Some("level_up") => {
                if let Some(bus_id) = self.send_target {
                    Action::MixerAdjustSend(bus_id, 0.05)
                } else {
                    Action::MixerAdjustLevel(0.05)
                }
            }
            Some("level_down") => {
                if let Some(bus_id) = self.send_target {
                    Action::MixerAdjustSend(bus_id, -0.05)
                } else {
                    Action::MixerAdjustLevel(-0.05)
                }
            }
            Some("level_up_big") => {
                if let Some(bus_id) = self.send_target {
                    Action::MixerAdjustSend(bus_id, 0.10)
                } else {
                    Action::MixerAdjustLevel(0.10)
                }
            }
            Some("level_down_big") => {
                if let Some(bus_id) = self.send_target {
                    Action::MixerAdjustSend(bus_id, -0.10)
                } else {
                    Action::MixerAdjustLevel(-0.10)
                }
            }
            Some("mute") => Action::MixerToggleMute,
            Some("solo") => Action::MixerToggleSolo,
            Some("output") => Action::MixerCycleOutput,
            Some("output_rev") => Action::MixerCycleOutputReverse,
            Some("section") => { self.send_target = None; Action::MixerCycleSection }
            Some(s) if s.starts_with("send") && s.len() == 5 => {
                if let Some(n) = s[4..].parse::<u8>().ok() {
                    self.send_target = Some(n);
                }
                Action::None
            }
            Some("send_toggle") => {
                if let Some(bus_id) = self.send_target {
                    Action::MixerToggleSend(bus_id)
                } else {
                    Action::None
                }
            }
            Some("clear_send") => { self.send_target = None; Action::None }
            _ => Action::None,
        }
    }

    fn render(&self, g: &mut dyn Graphics) {
        let (width, height) = g.size();
        let rect = Rect::centered(width, height, 80, 14);

        g.set_style(Style::new().fg(Color::CYAN));
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
            (selected - visible + 1).min(total.saturating_sub(visible))
        } else {
            0
        }
    }

    /// Render the mixer with access to rack state
    pub fn render_with_state(&self, g: &mut dyn Graphics, rack: &RackState) {
        let (width, height) = g.size();

        // Box dimensions: channels + sep + buses + sep + master + padding
        let box_width = (NUM_VISIBLE_CHANNELS as u16 * STRIP_WIDTH) + 2 +
                        (NUM_VISIBLE_BUSES as u16 * STRIP_WIDTH) + 2 +
                        STRIP_WIDTH + 4;
        let box_height = METER_HEIGHT + 8; // meter + label + name + db + mute + output + help + padding
        let rect = Rect::centered(width, height, box_width, box_height);

        g.set_style(Style::new().fg(Color::CYAN));
        g.draw_box(rect, Some(" MIXER "));

        let base_x = rect.x + 2;
        let base_y = rect.y + 1;

        // Layout: label, name, [meter], dB, mute/solo, output
        let label_y = base_y;
        let name_y = base_y + 1;
        let meter_top_y = base_y + 2;
        let db_y = meter_top_y + METER_HEIGHT;
        let indicator_y = db_y + 1;
        let output_y = indicator_y + 1;

        // Calculate scroll offsets
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

        // Render channels
        for i in 0..NUM_VISIBLE_CHANNELS {
            let ch_idx = channel_scroll + i;
            if ch_idx >= rack.mixer.channels.len() {
                break;
            }
            let ch = &rack.mixer.channels[ch_idx];
            let is_selected = matches!(rack.mixer.selection, MixerSelection::Channel(id) if id == ch.id);

            let module_name = ch.module_id
                .and_then(|id| rack.modules.get(&id))
                .map(|m| m.name.as_str())
                .unwrap_or("---");

            self.render_vertical_strip(
                g, x, &format!("CH{}", ch.id), module_name,
                ch.level, ch.mute, ch.solo, Some(ch.output_target), is_selected,
                label_y, name_y, meter_top_y, db_y, indicator_y, output_y,
            );

            x += STRIP_WIDTH;
        }

        // Separator before buses
        g.set_style(Style::new().fg(Color::PURPLE));
        for y in label_y..=output_y {
            g.put_char(x, y, '│');
        }
        x += 2;

        // Render buses
        for i in 0..NUM_VISIBLE_BUSES {
            let bus_idx = bus_scroll + i;
            if bus_idx >= rack.mixer.buses.len() {
                break;
            }
            let bus = &rack.mixer.buses[bus_idx];
            let is_selected = matches!(rack.mixer.selection, MixerSelection::Bus(id) if id == bus.id);

            self.render_vertical_strip(
                g, x, &format!("BUS{}", bus.id), &bus.name,
                bus.level, bus.mute, bus.solo, None, is_selected,
                label_y, name_y, meter_top_y, db_y, indicator_y, output_y,
            );

            x += STRIP_WIDTH;
        }

        // Separator before master
        g.set_style(Style::new().fg(Color::GOLD));
        for y in label_y..=output_y {
            g.put_char(x, y, '│');
        }
        x += 2;

        // Master
        let is_master_selected = matches!(rack.mixer.selection, MixerSelection::Master);
        self.render_vertical_strip(
            g, x, "MASTER", "",
            rack.mixer.master_level, rack.mixer.master_mute, false, None, is_master_selected,
            label_y, name_y, meter_top_y, db_y, indicator_y, output_y,
        );

        // Send info line (below output row)
        let send_y = output_y + 1;
        if let Some(bus_id) = self.send_target {
            // Show send level for selected channel to this bus
            if let MixerSelection::Channel(ch_id) = rack.mixer.selection {
                if let Some(ch) = rack.mixer.channel(ch_id) {
                    if let Some(send) = ch.sends.iter().find(|s| s.bus_id == bus_id) {
                        let status = if send.enabled { "ON" } else { "OFF" };
                        let info = format!("Send→B{}: {:.0}% [{}]", bus_id, send.level * 100.0, status);
                        g.set_style(Style::new().fg(Color::TEAL).bold());
                        g.put_str(base_x, send_y, &info);
                    }
                }
            }
        }

        // Help text
        let help_y = rect.y + rect.height - 2;
        g.set_style(Style::new().fg(Color::DARK_GRAY));
        g.put_str(
            base_x,
            help_y,
            "[←/→] Select  [↑/↓] Level  [M]ute [S]olo [o]ut  [1-8] Send  [F2] Rack",
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn render_vertical_strip(
        &self,
        g: &mut dyn Graphics,
        x: u16,
        label: &str,
        name: &str,
        level: f32,
        mute: bool,
        solo: bool,
        output: Option<OutputTarget>,
        selected: bool,
        label_y: u16,
        name_y: u16,
        meter_top_y: u16,
        db_y: u16,
        indicator_y: u16,
        output_y: u16,
    ) {
        let strip_w = (STRIP_WIDTH - 1) as usize;

        // Label
        let label_style = if selected {
            Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold()
        } else if label.starts_with("BUS") {
            Style::new().fg(Color::PURPLE).bold()
        } else if label == "MASTER" {
            Style::new().fg(Color::GOLD).bold()
        } else {
            Style::new().fg(Color::CYAN)
        };
        g.set_style(label_style);
        let label_str: String = label.chars().take(strip_w).collect();
        g.put_str(x, label_y, &label_str);

        // Name
        let text_style = if selected {
            Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG)
        } else {
            Style::new().fg(Color::DARK_GRAY)
        };
        g.set_style(text_style);
        let name_display = if name.is_empty() && label.starts_with("CH") {
            "---"
        } else {
            name
        };
        let name_str: String = name_display.chars().take(strip_w).collect();
        g.put_str(x, name_y, &name_str);

        // Vertical meter (centered in strip)
        let meter_x = x + (STRIP_WIDTH / 2).saturating_sub(1);
        Self::render_vertical_meter(g, meter_x, meter_top_y, METER_HEIGHT, level);

        // Highlight the meter column if selected
        if selected {
            let sel_x = meter_x + 1;
            g.set_style(Style::new().fg(Color::WHITE).bold());
            g.put_char(sel_x, meter_top_y, '▼');
        }

        // dB value
        let db_style = if selected {
            Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG)
        } else {
            Style::new().fg(Color::SKY_BLUE)
        };
        g.set_style(db_style);
        let db_str = Self::level_to_db(level);
        g.put_str(x, db_y, &db_str);

        // Mute/solo indicator
        let (indicator, indicator_style) = if mute {
            ("M", Style::new().fg(Color::MUTE_COLOR).bold())
        } else if solo {
            ("S", Style::new().fg(Color::SOLO_COLOR).bold())
        } else {
            ("●", Style::new().fg(Color::DARK_GRAY))
        };
        g.set_style(indicator_style);
        g.put_str(x, indicator_y, indicator);

        // Output routing
        if let Some(target) = output {
            let routing_style = if selected {
                Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG)
            } else {
                Style::new().fg(Color::TEAL)
            };
            g.set_style(routing_style);
            g.put_str(x, output_y, Self::format_output(target));
        }
    }
}
