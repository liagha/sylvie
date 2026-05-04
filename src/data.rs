use crate::message::Message;
use rand::rngs::SmallRng;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use std::fs;
use rand::SeedableRng;

#[derive(Serialize, Deserialize, Clone)]
pub struct Record {
    pub phrase: Message,
    pub command: Message,
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

    pub fn save(&self, path: &str) {
        let content = serde_json::to_string_pretty(self).unwrap();
        fs::write(path, content).unwrap();
    }

    pub fn split(mut self) -> (Self, Self) {
        let total = self.items.len();
        if total == 0 {
            return (Self { items: vec![] }, Self { items: vec![] });
        }
        let mut rng = SmallRng::seed_from_u64(42);
        self.items.shuffle(&mut rng);
        if total == 1 {
            let item = self.items[0].clone();
            return (
                Self { items: vec![item.clone()] },
                Self { items: vec![item] },
            );
        }
        let point = (total as f32 * 0.9) as usize;
        let point = if point == 0 { 1 } else { point.min(total - 1) };
        let valid_items = self.items.split_off(point);
        (
            Self { items: self.items },
            Self { items: valid_items },
        )
    }
}