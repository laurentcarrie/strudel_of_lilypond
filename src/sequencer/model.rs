use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Pattern {
    pub description: String,
    pub voices: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Bar {
    pub pattern_name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum EBarSequence {
    Single(Bar),
    Group(Vec<EBarSequence>),
    RepeatBar(u32, Bar),
    RepeatGroup(u32, Vec<EBarSequence>),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SequenceItem {
    pub item: EBarSequence,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BarSequence {
    pub tempo: u32,
    pub sequence: Vec<SequenceItem>,
}
