use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::StatefulWidget;

use rat_event::{HandleEvent, Regular};
use rat_widget::focus::HasFocus;
use rat_widget::number_input::{NumberInput as RatNumberInput, NumberInputState};

use crate::ui::input::InputEvent;
use crate::ui::rat_compat::{outcome_consumed, to_crossterm_key_event};
use crate::ui::theme::DawTheme;

/// A numeric input widget backed by rat-widget NumberInput.
///
/// Wraps `rat_widget::number_input` for use in DAW parameter editing.
pub struct NumericInput {
    state: NumberInputState,
}

impl NumericInput {
    pub fn new(pattern: &str) -> Self {
        Self {
            state: NumberInputState::new_pattern(pattern)
                .expect("valid number format pattern"),
        }
    }

    /// Create a numeric input for float values (e.g., "###0.00")
    pub fn float() -> Self {
        Self::new("###0.00")
    }

    /// Create a numeric input for integer values (e.g., "###0")
    pub fn integer() -> Self {
        Self::new("###0")
    }

    pub fn value_f32(&self) -> Option<f32> {
        self.state.value::<f32>().ok()
    }

    pub fn value_i32(&self) -> Option<i32> {
        self.state.value::<i32>().ok()
    }

    pub fn set_value_f32(&mut self, val: f32) {
        let _ = self.state.set_value(val);
    }

    pub fn set_value_i32(&mut self, val: i32) {
        let _ = self.state.set_value(val);
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.state.focus().set(focused);
    }

    pub fn is_focused(&self) -> bool {
        self.state.is_focused()
    }

    pub fn select_all(&mut self) {
        self.state.select_all();
    }

    /// Handle input, returns true if the event was consumed
    pub fn handle_input(&mut self, event: &InputEvent) -> bool {
        if !self.state.is_focused() {
            return false;
        }
        let ct_event = to_crossterm_key_event(event);
        let outcome: rat_event::Outcome = self.state.handle(&ct_event, Regular).into();
        outcome_consumed(outcome)
    }

    /// Render the number input into a ratatui buffer at the given position.
    pub fn render_buf(&mut self, buf: &mut Buffer, x: u16, y: u16, width: u16) -> u16 {
        let widget = RatNumberInput::new()
            .style(DawTheme::number_input_style())
            .focus_style(DawTheme::number_input_focus_style())
            .select_style(DawTheme::number_input_select_style());

        let area = Rect::new(x, y, width, 1);
        widget.render(area, buf, &mut self.state);
        1
    }
}
