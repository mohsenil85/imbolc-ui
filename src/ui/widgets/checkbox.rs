use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Span;
use ratatui::widgets::StatefulWidget;

use rat_event::{HandleEvent, Regular};
use rat_widget::checkbox::{Checkbox as RatCheckbox, CheckboxState};
use rat_widget::focus::HasFocus;

use crate::ui::input::InputEvent;
use crate::ui::rat_compat::{outcome_consumed, to_crossterm_key_event};
use crate::ui::theme::DawTheme;

/// A checkbox widget backed by rat-widget Checkbox.
pub struct CheckboxWidget {
    label: String,
    state: CheckboxState,
}

impl CheckboxWidget {
    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_string(),
            state: CheckboxState::new(),
        }
    }

    pub fn checked(&self) -> bool {
        self.state.checked()
    }

    pub fn set_checked(&mut self, checked: bool) {
        self.state.set_checked(checked);
    }

    pub fn toggle(&mut self) {
        self.state.flip_checked();
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.state.focus.set(focused);
    }

    pub fn is_focused(&self) -> bool {
        self.state.is_focused()
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

    /// Render the checkbox at the given position.
    pub fn render_buf(&mut self, buf: &mut Buffer, x: u16, y: u16, width: u16) -> u16 {
        let widget = RatCheckbox::new()
            .text(self.label.as_str())
            .true_str(Span::from("[x]"))
            .false_str(Span::from("[ ]"))
            .style(DawTheme::checkbox_style())
            .focus_style(DawTheme::checkbox_focus_style());

        let area = Rect::new(x, y, width, 1);
        widget.render(area, buf, &mut self.state);
        1
    }
}
