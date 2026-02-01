#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub value: ParamValue,
    pub min: f32,
    pub max: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParamValue {
    Float(f32),
    Int(i32),
    Bool(bool),
}
