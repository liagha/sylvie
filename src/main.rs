mod config;
mod data;
mod infer;
mod model;
mod tokens;
mod train;

use candle_core::{Device, Result, Tensor};
use candle_nn::VarMap;
use config::Config;
use data::Corpus;
use std::env;
use std::io::{self, Write};
use std::process::Command;
use tokens::Lexicon;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("infer");
    let device = Device::Cpu;

    match mode {
        "train" => {
            let map = VarMap::new();
            let corpus = Corpus::load("dataset.json");
            let mut lexicon = Lexicon::new();

            for item in &corpus.items {
                lexicon.learn(&item.phrase);
                lexicon.learn(&item.command);
            }

            lexicon.save("lexicon.json");

            let mut length = 0;
            for item in &corpus.items {
                let phrase = lexicon.process(&item.phrase);
                let command = lexicon.process(&item.command);
                let total = phrase.len() + command.len() + 3;
                if total > length {
                    length = total;
                }
            }

            let mut sequences = Vec::new();

            for item in &corpus.items {
                let mut row = Vec::new();
                row.push(1);
                row.extend(lexicon.process(&item.phrase));
                row.push(3);
                row.extend(lexicon.process(&item.command));
                row.push(2);

                while row.len() < length {
                    row.push(0);
                }
                sequences.extend(row);
            }

            let count = corpus.items.len();
            let inputs = Tensor::from_vec(sequences.clone(), (count, length), &device)?;
            let targets = Tensor::from_vec(sequences, (count, length), &device)?;

            let config = Config {
                vocab: lexicon.size(),
                dim: 128,
                heads: 8,
                limit: 256,
            };
            config.save("config.json");

            train::execute(&inputs, &targets, &map, &device, &config)?;
        }
        "infer" => {
            let default = String::from("");
            let query = args.get(2).unwrap_or(&default);
            let lexicon = Lexicon::load("lexicon.json");
            let config = Config::load("config.json");

            let mut encoded = Vec::new();
            encoded.push(1);
            encoded.extend(lexicon.process(query));
            encoded.push(3);

            let output = infer::execute(&encoded, &device, &config)?;
            let text = lexicon.decode(&output);

            println!("sylvie: {}", text);
            print!("execute? (y/n): ");
            io::stdout().flush().unwrap();

            let mut answer = String::new();
            io::stdin().read_line(&mut answer).unwrap();

            if answer.trim().eq_ignore_ascii_case("y") {
                let parts: Vec<&str> = text.split_whitespace().collect();
                if !parts.is_empty() {
                    let mut process = Command::new(parts[0]);
                    if parts.len() > 1 {
                        process.args(&parts[1..]);
                    }
                    process.status().unwrap();
                }
            }
        }
        _ => panic!("invalid"),
    }

    Ok(())
}
