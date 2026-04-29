use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    pub vocab: usize,
    pub dim: usize,
    pub heads: usize,
    pub limit: usize,
    pub drop: f32,
}

impl Config {
    pub fn load(path: &str) -> Self {
        let content = fs::read_to_string(path).unwrap();
        serde_json::from_str(&content).unwrap()
    }

    pub fn save(&self, path: &str) {
        let content = serde_json::to_string_pretty(self).unwrap();
        fs::write(path, content).unwrap();
    }
}
