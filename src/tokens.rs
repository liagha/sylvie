use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

#[derive(Serialize, Deserialize)]
pub struct Lexicon {
    forward: HashMap<String, u32>,
    reverse: HashMap<u32, String>,
    counter: u32,
}

impl Lexicon {
    pub fn new() -> Self {
        let mut forward = HashMap::new();
        let mut reverse = HashMap::new();

        forward.insert(String::from("<pad>"), 0);
        forward.insert(String::from("<bos>"), 1);
        forward.insert(String::from("<eos>"), 2);
        forward.insert(String::from("<sep>"), 3);

        reverse.insert(0, String::from("<pad>"));
        reverse.insert(1, String::from("<bos>"));
        reverse.insert(2, String::from("<eos>"));
        reverse.insert(3, String::from("<sep>"));

        Self {
            forward,
            reverse,
            counter: 4,
        }
    }

    pub fn learn(&mut self, text: &str) {
        for word in text.split_whitespace() {
            let lower = word.to_lowercase();
            if !self.forward.contains_key(&lower) {
                self.forward.insert(lower.clone(), self.counter);
                self.reverse.insert(self.counter, lower);
                self.counter += 1;
            }
        }
    }

    pub fn process(&self, text: &str) -> Vec<u32> {
        let mut result = Vec::new();
        for word in text.split_whitespace() {
            let lower = word.to_lowercase();
            let value = self.forward.get(&lower).copied().unwrap_or(0);
            result.push(value);
        }
        result
    }

    pub fn decode(&self, tokens: &[u32]) -> String {
        let mut result = Vec::new();
        let mut in_command = false;

        for token in tokens {
            if *token == 2 {
                break;
            }
            if in_command && *token > 3 {
                if let Some(word) = self.reverse.get(token) {
                    result.push(word.clone());
                }
            }
            if *token == 3 { // <sep>
                in_command = true;
            }
        }
        result.join(" ")
    }

    pub fn size(&self) -> usize {
        self.counter as usize
    }

    pub fn save(&self, path: &str) {
        let content = serde_json::to_string(self).unwrap();
        fs::write(path, content).unwrap();
    }

    pub fn load(path: &str) -> Self {
        let content = fs::read_to_string(path).unwrap();
        serde_json::from_str(&content).unwrap()
    }
}
