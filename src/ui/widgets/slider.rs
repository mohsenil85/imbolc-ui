use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::StatefulWidget;

use rat_event::{HandleEvent, Regular};
use rat_widget::focus::HasFocus;
use rat_widget::slider::{Slider as RatSlider, SliderState};

use crate::ui::input::InputEvent;
use crate::ui::rat_compat::{outcome_consumed, to_crossterm_key_event};
use crate::ui::theme::DawTheme;

/// A horizontal slider widget backed by rat-widget Slider.
pub struct SliderWidget {
    state: SliderState<f32>,
}

impl SliderWidget {
    pub fn new(min: f32, max: f32, step: f32) -> Self {
        let mut state = SliderState::<f32>::new_range((min, max), step);
        state.set_value(min);
        Self { state }
    }

    /// Create a 0.0-1.0 slider (e.g., for pan, level)
    pub fn unit() -> Self {
        Self::new(0.0, 1.0, 0.01)
    }

    pub fn value(&self) -> f32 {
        self.state.value()
    }

    pub fn set_value(&mut self, val: f32) {
        self.state.set_value(val);
    }

    pub fn set_range(&mut self, min: f32, max: f32) {
        self.state.set_range((min, max));
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

    /// Render the slider at the given position.
    pub fn render_buf(&mut self, buf: &mut Buffer, x: u16, y: u16, width: u16) -> u16 {
        let widget = RatSlider::<f32>::new()
            .style(DawTheme::slider_style())
            .focus_style(DawTheme::slider_focus_style())
            .knob_style(DawTheme::slider_knob_style());

        let area = Rect::new(x, y, width, 1);
        widget.render(area, buf, &mut self.state);
        1
    }
}
