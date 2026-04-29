use rand::rng;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Serialize, Deserialize, Clone)]
pub struct Record {
    pub phrase: String,
    pub command: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Corpus {
    pub items: Vec<Record>,
}

impl Corpus {
    pub fn load(path: &str) -> Self {
        let content = fs::read_to_string(path).unwrap();
        serde_json::from_str(&content).unwrap()
    }

    pub fn split(mut self) -> (Self, Self) {
        let mut gen = rng();
        self.items.shuffle(&mut gen);
        let point = (self.items.len() as f32 * 0.9) as usize;
        let validation = self.items.split_off(point);
        (Self { items: self.items }, Self { items: validation })
    }
}
