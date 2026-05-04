use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum Message {
    Text(String),
    Command(String),
    Trigger(String),
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let json = serde_json::to_string(self).unwrap();
        write!(f, "{}", json)
    }
}

impl FromStr for Message {
    type Err = serde_json::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
}