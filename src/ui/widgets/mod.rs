#[allow(dead_code)]
mod checkbox;
#[allow(dead_code)]
mod number_input;
#[allow(dead_code)]
mod slider;
mod text_input;

#[allow(unused_imports)]
pub use checkbox::CheckboxWidget;
#[allow(unused_imports)]
pub use number_input::NumericInput;
#[allow(unused_imports)]
pub use slider::SliderWidget;
pub use text_input::TextInput;
