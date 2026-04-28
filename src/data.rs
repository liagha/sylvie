use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Serialize, Deserialize)]
pub struct Record {
    pub phrase: String,
    pub command: String,
}

#[derive(Serialize, Deserialize)]
pub struct Corpus {
    pub items: Vec<Record>,
}

impl Corpus {
    pub fn load(path: &str) -> Self {
        let content = fs::read_to_string(path).unwrap();
        serde_json::from_str(&content).unwrap()
    }
}
