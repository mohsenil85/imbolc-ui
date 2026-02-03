use ratatui::buffer::Buffer;
use ratatui::layout::Rect as RatatuiRect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use super::InstrumentEditPane;
use crate::state::{AppState, Param, ParamValue};
use crate::ui::layout_helpers::center_rect;
use crate::ui::widgets::TextInput;
use crate::ui::{Color, Style};

impl InstrumentEditPane {
    pub(super) fn render_impl(&mut self, area: RatatuiRect, buf: &mut Buffer, _state: &AppState) {
        let rect = center_rect(area, 97, 29);

        let title = format!(" Edit: {} ({}) ", self.instrument_name, self.source.name());
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title.as_str())
            .border_style(ratatui::style::Style::from(Style::new().fg(Color::ORANGE)))
            .title_style(ratatui::style::Style::from(Style::new().fg(Color::ORANGE)));
        let inner = block.inner(rect);
        block.render(rect, buf);

        let content_x = inner.x + 1;
        let mut y = inner.y + 1;

        // Mode indicators in header
        let mode_x = rect.x + rect.width - 18;
        let poly_style = ratatui::style::Style::from(Style::new().fg(if self.polyphonic { Color::LIME } else { Color::DARK_GRAY }));
        let poly_str = if self.polyphonic { " POLY " } else { " MONO " };
        Paragraph::new(Line::from(Span::styled(poly_str, poly_style)))
            .render(RatatuiRect::new(mode_x, rect.y, 6, 1), buf);

        // Active/Inactive indicator for AudioIn instruments
        if self.source.is_audio_input() {
            let active_style = ratatui::style::Style::from(Style::new().fg(
                if self.active { Color::LIME } else { Color::new(220, 40, 40) }
            ));
            let active_str = if self.active { " ACTIVE " } else { " INACTIVE " };
            let active_x = mode_x.saturating_sub(active_str.len() as u16 + 1);
            Paragraph::new(Line::from(Span::styled(active_str, active_style)))
                .render(RatatuiRect::new(active_x, rect.y, active_str.len() as u16, 1), buf);
        }

        // Piano/Pad mode indicator
        if self.pad_keyboard.is_active() {
            let pad_str = self.pad_keyboard.status_label();
            let pad_style = ratatui::style::Style::from(Style::new().fg(Color::BLACK).bg(Color::KIT_COLOR));
            Paragraph::new(Line::from(Span::styled(pad_str.clone(), pad_style)))
                .render(RatatuiRect::new(rect.x + 1, rect.y, pad_str.len() as u16, 1), buf);
        } else if self.piano.is_active() {
            let piano_str = self.piano.status_label();
            let piano_style = ratatui::style::Style::from(Style::new().fg(Color::BLACK).bg(Color::PINK));
            Paragraph::new(Line::from(Span::styled(piano_str.clone(), piano_style)))
                .render(RatatuiRect::new(rect.x + 1, rect.y, piano_str.len() as u16, 1), buf);
        }

        let mut global_row = 0;

        // === SOURCE SECTION ===
        let source_header = if self.source.is_sample() {
            format!("SOURCE: {}  (o: load)", self.source.name())
        } else {
            format!("SOURCE: {}", self.source.name())
        };
        Paragraph::new(Line::from(Span::styled(
            source_header,
            ratatui::style::Style::from(Style::new().fg(Color::CYAN).bold()),
        ))).render(RatatuiRect::new(content_x, y, inner.width.saturating_sub(2), 1), buf);
        y += 1;

        // Sample name row for sampler instruments
        if self.source.is_sample() {
            let is_sel = self.selected_row == global_row;
            let display_name = self.sample_name.as_deref().unwrap_or("(no sample)");
            render_label_value_row_buf(buf, content_x, y, "Sample", display_name, Color::CYAN, is_sel);
            y += 1;
            global_row += 1;
        }

        if self.source_params.is_empty() {
            let is_sel = self.selected_row == global_row;
            let style = if is_sel {
                ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY).bg(Color::SELECTION_BG))
            } else {
                ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY))
            };
            Paragraph::new(Line::from(Span::styled("(no parameters)", style)))
                .render(RatatuiRect::new(content_x + 2, y, inner.width.saturating_sub(4), 1), buf);
            global_row += 1;
        } else {
            for param in &self.source_params {
                let is_sel = self.selected_row == global_row;
                render_param_row_buf(buf, content_x, y, param, is_sel, self.editing && is_sel, &mut self.edit_input);
                y += 1;
                global_row += 1;
            }
        }
        y += 1;

        // === FILTER SECTION ===
        let filter_label = if let Some(ref f) = self.filter {
            format!("FILTER: {}  (f: off, t: cycle)", f.filter_type.name())
        } else {
            "FILTER: OFF  (f: enable)".to_string()
        };
        Paragraph::new(Line::from(Span::styled(
            filter_label,
            ratatui::style::Style::from(Style::new().fg(Color::FILTER_COLOR).bold()),
        ))).render(RatatuiRect::new(content_x, y, inner.width.saturating_sub(2), 1), buf);
        y += 1;

        if let Some(ref f) = self.filter {
            // Type row
            {
                let is_sel = self.selected_row == global_row;
                render_label_value_row_buf(buf, content_x, y, "Type", &f.filter_type.name(), Color::FILTER_COLOR, is_sel);
                y += 1;
                global_row += 1;
            }
            // Cutoff row
            {
                let is_sel = self.selected_row == global_row;
                render_value_row_buf(buf, content_x, y, "Cutoff", f.cutoff.value, f.cutoff.min, f.cutoff.max, is_sel, self.editing && is_sel, &mut self.edit_input);
                y += 1;
                global_row += 1;
            }
            // Resonance row
            {
                let is_sel = self.selected_row == global_row;
                render_value_row_buf(buf, content_x, y, "Resonance", f.resonance.value, f.resonance.min, f.resonance.max, is_sel, self.editing && is_sel, &mut self.edit_input);
                y += 1;
                global_row += 1;
            }
            // Extra filter params (e.g. shape for Vowel, drive for ResDrive)
            for param in &f.extra_params {
                let is_sel = self.selected_row == global_row;
                render_param_row_buf(buf, content_x, y, param, is_sel, self.editing && is_sel, &mut self.edit_input);
                y += 1;
                global_row += 1;
            }
        } else {
            let is_sel = self.selected_row == global_row;
            let style = if is_sel {
                ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY).bg(Color::SELECTION_BG))
            } else {
                ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY))
            };
            Paragraph::new(Line::from(Span::styled("(disabled)", style)))
                .render(RatatuiRect::new(content_x + 2, y, inner.width.saturating_sub(4), 1), buf);
            y += 1;
            global_row += 1;
        }
        y += 1;

        // === EFFECTS SECTION ===
        Paragraph::new(Line::from(Span::styled(
            "EFFECTS  (a: add effect, d: remove)",
            ratatui::style::Style::from(Style::new().fg(Color::FX_COLOR).bold()),
        ))).render(RatatuiRect::new(content_x, y, inner.width.saturating_sub(2), 1), buf);
        y += 1;

        if self.effects.is_empty() {
            let is_sel = self.selected_row == global_row;
            let style = if is_sel {
                ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY).bg(Color::SELECTION_BG))
            } else {
                ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY))
            };
            Paragraph::new(Line::from(Span::styled("(no effects)", style)))
                .render(RatatuiRect::new(content_x + 2, y, inner.width.saturating_sub(4), 1), buf);
            global_row += 1;
        } else {
            for effect in &self.effects {
                let is_sel = self.selected_row == global_row;
                // Selection indicator
                if is_sel {
                    if let Some(cell) = buf.cell_mut((content_x, y)) {
                        cell.set_char('>').set_style(
                            ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold()),
                        );
                    }
                }

                let enabled_str = if effect.enabled { "ON " } else { "OFF" };
                let effect_text = format!("{:10} [{}]", effect.effect_type.name(), enabled_str);
                let effect_style = if is_sel {
                    ratatui::style::Style::from(Style::new().fg(Color::FX_COLOR).bg(Color::SELECTION_BG))
                } else {
                    ratatui::style::Style::from(Style::new().fg(Color::FX_COLOR))
                };
                Paragraph::new(Line::from(Span::styled(effect_text, effect_style)))
                    .render(RatatuiRect::new(content_x + 2, y, 18, 1), buf);

                // Params inline
                let params_str: String = effect.params.iter().take(3).map(|p| {
                    match &p.value {
                        ParamValue::Float(v) => format!("{}:{:.2}", p.name, v),
                        ParamValue::Int(v) => format!("{}:{}", p.name, v),
                        ParamValue::Bool(v) => format!("{}:{}", p.name, v),
                    }
                }).collect::<Vec<_>>().join("  ");
                let params_style = if is_sel {
                    ratatui::style::Style::from(Style::new().fg(Color::SKY_BLUE).bg(Color::SELECTION_BG))
                } else {
                    ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY))
                };
                Paragraph::new(Line::from(Span::styled(params_str, params_style)))
                    .render(RatatuiRect::new(content_x + 20, y, inner.width.saturating_sub(22), 1), buf);

                y += 1;
                global_row += 1;
            }
        }
        y += 1;

        // === LFO SECTION ===
        let lfo_status = if self.lfo.enabled { "ON" } else { "OFF" };
        Paragraph::new(Line::from(Span::styled(
            format!("LFO [{}]  (l: toggle, s: shape, m: target)", lfo_status),
            ratatui::style::Style::from(Style::new().fg(Color::PINK).bold()),
        ))).render(RatatuiRect::new(content_x, y, inner.width.saturating_sub(2), 1), buf);
        y += 1;

        // Row 0: Enabled
        {
            let is_sel = self.selected_row == global_row;
            let enabled_val = if self.lfo.enabled { "ON" } else { "OFF" };
            render_label_value_row_buf(buf, content_x, y, "Enabled", enabled_val, Color::PINK, is_sel);
            y += 1;
            global_row += 1;
        }

        // Row 1: Rate
        {
            let is_sel = self.selected_row == global_row;
            render_value_row_buf(buf, content_x, y, "Rate", self.lfo.rate, 0.1, 32.0, is_sel, self.editing && is_sel, &mut self.edit_input);
            // Hz label
            let hz_style = if is_sel {
                ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY).bg(Color::SELECTION_BG))
            } else {
                ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY))
            };
            for (j, ch) in "Hz".chars().enumerate() {
                if let Some(cell) = buf.cell_mut((content_x + 44 + j as u16, y)) {
                    cell.set_char(ch).set_style(hz_style);
                }
            }
            y += 1;
            global_row += 1;
        }

        // Row 2: Depth
        {
            let is_sel = self.selected_row == global_row;
            render_value_row_buf(buf, content_x, y, "Depth", self.lfo.depth, 0.0, 1.0, is_sel, self.editing && is_sel, &mut self.edit_input);
            y += 1;
            global_row += 1;
        }

        // Row 3: Shape and Target
        {
            let is_sel = self.selected_row == global_row;
            let shape_val = format!("{} → {}", self.lfo.shape.name(), self.lfo.target.name());
            render_label_value_row_buf(buf, content_x, y, "Shape/Dest", &shape_val, Color::PINK, is_sel);
            y += 1;
            global_row += 1;
        }
        y += 1;

        // === ENVELOPE SECTION === (hidden for VSTi — plugin has own envelope)
        if !self.source.is_vst() {
            Paragraph::new(Line::from(Span::styled(
                "ENVELOPE (ADSR)  (p: poly, r: track)",
                ratatui::style::Style::from(Style::new().fg(Color::ENV_COLOR).bold()),
            ))).render(RatatuiRect::new(content_x, y, inner.width.saturating_sub(2), 1), buf);
            y += 1;

            let env_labels = ["Attack", "Decay", "Sustain", "Release"];
            let env_values = [
                self.amp_envelope.attack,
                self.amp_envelope.decay,
                self.amp_envelope.sustain,
                self.amp_envelope.release,
            ];
            let env_maxes = [5.0, 5.0, 1.0, 5.0];

            for (label, (val, max)) in env_labels.iter().zip(env_values.iter().zip(env_maxes.iter())) {
                let is_sel = self.selected_row == global_row;
                render_value_row_buf(buf, content_x, y, label, *val, 0.0, *max, is_sel, self.editing && is_sel, &mut self.edit_input);
                y += 1;
                global_row += 1;
            }
        }

        // Suppress unused variable warning
        let _ = global_row;

        // Help text
        let help_y = rect.y + rect.height - 2;
        let help_text = if self.pad_keyboard.is_active() {
            "R T Y U / F G H J / V B N M: trigger pads | /: cycle | Esc: exit"
        } else if self.piano.is_active() {
            "Play keys | [/]: octave | \u{2190}/\u{2192}: adjust | \\: zero | /: cycle | Esc: exit"
        } else {
            "\u{2191}/\u{2193}: move | Tab/S-Tab: section | \u{2190}/\u{2192}: adjust | \\: zero | /: piano"
        };
        Paragraph::new(Line::from(Span::styled(
            help_text,
            ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY)),
        ))).render(RatatuiRect::new(content_x, help_y, inner.width.saturating_sub(2), 1), buf);
    }
}

fn render_slider(value: f32, min: f32, max: f32, width: usize) -> String {
    let normalized = (value - min) / (max - min);
    let pos = (normalized * width as f32) as usize;
    let pos = pos.min(width);
    let mut s = String::with_capacity(width + 2);
    s.push('[');
    for i in 0..width {
        if i == pos { s.push('|'); }
        else if i < pos { s.push('='); }
        else { s.push('-'); }
    }
    s.push(']');
    s
}

fn render_param_row_buf(
    buf: &mut Buffer,
    x: u16, y: u16,
    param: &Param,
    is_selected: bool,
    is_editing: bool,
    edit_input: &mut TextInput,
) {
    // Selection indicator
    if is_selected {
        if let Some(cell) = buf.cell_mut((x, y)) {
            cell.set_char('>').set_style(
                ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold()),
            );
        }
    }

    // Param name
    let name_style = if is_selected {
        ratatui::style::Style::from(Style::new().fg(Color::CYAN).bg(Color::SELECTION_BG))
    } else {
        ratatui::style::Style::from(Style::new().fg(Color::CYAN))
    };
    let name_str = format!("{:12}", param.name);
    for (j, ch) in name_str.chars().enumerate() {
        if let Some(cell) = buf.cell_mut((x + 2 + j as u16, y)) {
            cell.set_char(ch).set_style(name_style);
        }
    }

    // Slider
    let (val, min, max) = match &param.value {
        ParamValue::Float(v) => (*v, param.min, param.max),
        ParamValue::Int(v) => (*v as f32, param.min, param.max),
        ParamValue::Bool(v) => (if *v { 1.0 } else { 0.0 }, 0.0, 1.0),
    };
    let slider = render_slider(val, min, max, 16);
    let slider_style = if is_selected {
        ratatui::style::Style::from(Style::new().fg(Color::LIME).bg(Color::SELECTION_BG))
    } else {
        ratatui::style::Style::from(Style::new().fg(Color::LIME))
    };
    for (j, ch) in slider.chars().enumerate() {
        if let Some(cell) = buf.cell_mut((x + 15 + j as u16, y)) {
            cell.set_char(ch).set_style(slider_style);
        }
    }

    // Value or text input
    if is_editing {
        edit_input.render_buf(buf, x + 34, y, 10);
    } else {
        let value_str = match &param.value {
            ParamValue::Float(v) => format!("{:.2}", v),
            ParamValue::Int(v) => format!("{}", v),
            ParamValue::Bool(v) => format!("{}", v),
        };
        let val_style = if is_selected {
            ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG))
        } else {
            ratatui::style::Style::from(Style::new().fg(Color::WHITE))
        };
        let formatted = format!("{:10}", value_str);
        for (j, ch) in formatted.chars().enumerate() {
            if let Some(cell) = buf.cell_mut((x + 34 + j as u16, y)) {
                cell.set_char(ch).set_style(val_style);
            }
        }
    }
}

fn render_value_row_buf(
    buf: &mut Buffer,
    x: u16, y: u16,
    name: &str,
    value: f32, min: f32, max: f32,
    is_selected: bool,
    is_editing: bool,
    edit_input: &mut TextInput,
) {
    // Selection indicator
    if is_selected {
        if let Some(cell) = buf.cell_mut((x, y)) {
            cell.set_char('>').set_style(
                ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold()),
            );
        }
    }

    // Label
    let name_style = if is_selected {
        ratatui::style::Style::from(Style::new().fg(Color::CYAN).bg(Color::SELECTION_BG))
    } else {
        ratatui::style::Style::from(Style::new().fg(Color::CYAN))
    };
    let name_str = format!("{:12}", name);
    for (j, ch) in name_str.chars().enumerate() {
        if let Some(cell) = buf.cell_mut((x + 2 + j as u16, y)) {
            cell.set_char(ch).set_style(name_style);
        }
    }

    // Slider
    let slider = render_slider(value, min, max, 16);
    let slider_style = if is_selected {
        ratatui::style::Style::from(Style::new().fg(Color::LIME).bg(Color::SELECTION_BG))
    } else {
        ratatui::style::Style::from(Style::new().fg(Color::LIME))
    };
    for (j, ch) in slider.chars().enumerate() {
        if let Some(cell) = buf.cell_mut((x + 15 + j as u16, y)) {
            cell.set_char(ch).set_style(slider_style);
        }
    }

    // Value or text input
    if is_editing {
        edit_input.render_buf(buf, x + 34, y, 10);
    } else {
        let val_style = if is_selected {
            ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG))
        } else {
            ratatui::style::Style::from(Style::new().fg(Color::WHITE))
        };
        let formatted = format!("{:.2}", value);
        for (j, ch) in formatted.chars().enumerate() {
            if let Some(cell) = buf.cell_mut((x + 34 + j as u16, y)) {
                cell.set_char(ch).set_style(val_style);
            }
        }
    }
}

/// Render a label-value row (no slider, for type/enabled/shape rows)
fn render_label_value_row_buf(
    buf: &mut Buffer,
    x: u16, y: u16,
    label: &str,
    value: &str,
    color: Color,
    is_selected: bool,
) {
    // Selection indicator
    if is_selected {
        if let Some(cell) = buf.cell_mut((x, y)) {
            cell.set_char('>').set_style(
                ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold()),
            );
        }
    }

    let text = format!("{:12}  {}", label, value);
    let style = if is_selected {
        ratatui::style::Style::from(Style::new().fg(color).bg(Color::SELECTION_BG))
    } else {
        ratatui::style::Style::from(Style::new().fg(color))
    };
    for (j, ch) in text.chars().enumerate() {
        if let Some(cell) = buf.cell_mut((x + 2 + j as u16, y)) {
            cell.set_char(ch).set_style(style);
        }
    }
}
