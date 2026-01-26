use std::any::Any;

use crate::state::{MixerSelection, MixerState, ModuleId, RackState, MAX_BUSES, MAX_CHANNELS};
use crate::ui::{Action, Color, Graphics, InputEvent, KeyCode, Keymap, Pane, Rect, Style};

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
                .bind_key(KeyCode::Tab, "section", "Cycle section"),
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
            Some("last") => Action::MixerJump(-1), // -1 signals "last"
            Some("level_up") => Action::MixerAdjustLevel(0.05),
            Some("level_down") => Action::MixerAdjustLevel(-0.05),
            Some("level_up_big") => Action::MixerAdjustLevel(0.10),
            Some("level_down_big") => Action::MixerAdjustLevel(-0.10),
            Some("mute") => Action::MixerToggleMute,
            Some("solo") => Action::MixerToggleSolo,
            Some("section") => Action::MixerCycleSection,
            _ => Action::None,
        }
    }

    fn render(&self, g: &mut dyn Graphics) {
        // This will be called with rack state passed via a different mechanism
        // For now just show a placeholder - the actual rendering happens in render_with_state
        let (width, height) = g.size();
        let rect = Rect::centered(width, height, 80, 20);

        g.set_style(Style::new().fg(Color::BLACK));
        g.draw_box(rect, Some(" Mixer "));

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
    /// Render the mixer with access to rack state
    pub fn render_with_state(&self, g: &mut dyn Graphics, rack: &RackState) {
        let (width, height) = g.size();
        let rect = Rect::centered(width, height, 90, 22);

        g.set_style(Style::new().fg(Color::BLACK));
        g.draw_box(rect, Some(" Mixer "));

        let x = rect.x + 2;
        let mut y = rect.y + 2;

        // Header
        g.set_style(Style::new().fg(Color::BLACK));
        g.put_str(x, y, "CHANNELS");
        y += 1;

        // Show active channels (those with modules assigned)
        let active_channels: Vec<_> = rack.mixer.channels.iter()
            .filter(|ch| ch.module_id.is_some())
            .collect();

        if active_channels.is_empty() {
            g.set_style(Style::new().fg(Color::GRAY));
            g.put_str(x, y, "(no OUTPUT modules - add one to see mixer channels)");
            y += 2;
        } else {
            // Draw channel strips
            let strip_width = 10;
            let mut strip_x = x;

            for ch in active_channels.iter().take(8) {
                let is_selected = matches!(rack.mixer.selection, MixerSelection::Channel(id) if id == ch.id);

                self.render_channel_strip(g, strip_x, y, strip_width, ch.id, ch.module_id,
                    ch.level, ch.pan, ch.mute, ch.solo, is_selected, rack);

                strip_x += strip_width + 1;
            }
            y += 8;
        }

        y += 1;

        // Buses section
        g.set_style(Style::new().fg(Color::BLACK));
        g.put_str(x, y, "BUSES");
        y += 1;

        let bus_x = x;
        for (i, bus) in rack.mixer.buses.iter().enumerate().take(8) {
            let is_selected = matches!(rack.mixer.selection, MixerSelection::Bus(id) if id == bus.id);

            if is_selected {
                g.set_style(Style::new().fg(Color::WHITE).bg(Color::BLACK));
            } else {
                g.set_style(Style::new().fg(Color::GRAY));
            }

            let bus_str = format!("B{}: {:4.0}% {}{}",
                bus.id,
                bus.level * 100.0,
                if bus.mute { "M" } else { " " },
                if bus.solo { "S" } else { " " }
            );
            g.put_str(bus_x + (i as u16 * 12), y, &bus_str);
        }
        y += 2;

        // Master
        g.set_style(Style::new().fg(Color::BLACK));
        g.put_str(x, y, "MASTER: ");

        let is_master_selected = matches!(rack.mixer.selection, MixerSelection::Master);
        if is_master_selected {
            g.set_style(Style::new().fg(Color::WHITE).bg(Color::BLACK));
        } else {
            g.set_style(Style::new().fg(Color::BLACK));
        }
        g.put_str(x + 8, y, &format!("{:.0}%", rack.mixer.master_level * 100.0));

        // Help text
        let help_y = rect.y + rect.height - 2;
        g.set_style(Style::new().fg(Color::GRAY));
        g.put_str(x, help_y, "Arrows: select/level | PgUp/Dn: +/-10% | m: mute | s: solo | Tab: section | Esc: back");
    }

    fn render_channel_strip(
        &self,
        g: &mut dyn Graphics,
        x: u16,
        y: u16,
        _width: u16,
        channel_id: u8,
        module_id: Option<ModuleId>,
        level: f32,
        pan: f32,
        mute: bool,
        solo: bool,
        selected: bool,
        rack: &RackState,
    ) {
        let style = if selected {
            Style::new().fg(Color::WHITE).bg(Color::BLACK)
        } else {
            Style::new().fg(Color::BLACK)
        };
        g.set_style(style);

        // Channel number
        g.put_str(x, y, &format!("CH{:<2}", channel_id));

        // Module name (truncated)
        let module_name = module_id
            .and_then(|id| rack.modules.get(&id))
            .map(|m| m.name.as_str())
            .unwrap_or("---");
        let name_display: String = module_name.chars().take(8).collect();
        g.put_str(x, y + 1, &format!("{:<8}", name_display));

        // Level bar
        let bar_height = 4;
        let filled = (level * bar_height as f32) as u16;
        for i in 0..bar_height {
            let bar_y = y + 2 + (bar_height - 1 - i);
            let ch = if i < filled { '|' } else { '.' };
            g.put_str(x, bar_y, &format!("{}{}{}", ch, ch, ch));
        }

        // Level percentage
        g.put_str(x, y + 2 + bar_height, &format!("{:3.0}%", level * 100.0));

        // Pan
        let pan_str = if pan < -0.1 {
            format!("L{:.0}", -pan * 100.0)
        } else if pan > 0.1 {
            format!("R{:.0}", pan * 100.0)
        } else {
            "C".to_string()
        };
        g.put_str(x, y + 3 + bar_height, &format!("{:<4}", pan_str));

        // Mute/Solo
        let ms_str = format!("{}{}",
            if mute { "M" } else { " " },
            if solo { "S" } else { " " }
        );
        g.put_str(x + 5, y + 3 + bar_height, &ms_str);
    }
}
