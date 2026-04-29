// src/main.rs
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
use tokens::Tokenizer;

fn generate(corpus: &Corpus, tokenizer: &Tokenizer, length: usize, device: &Device) -> Result<(Tensor, Tensor)> {
    let mut sequences = Vec::new();
    let count = corpus.items.len();

    for item in &corpus.items {
        let mut row = Vec::new();
        row.push(1);
        row.extend(tokenizer.encode(&item.phrase));
        row.push(3);
        row.extend(tokenizer.encode(&item.command));
        row.push(2);

        while row.len() < length {
            row.push(0);
        }
        sequences.extend(row);
    }

    let inputs = Tensor::from_vec(sequences.clone(), (count, length), device)?;
    let targets = Tensor::from_vec(sequences, (count, length), device)?;

    Ok((inputs, targets))
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("infer");
    let device = Device::Cpu;

    match mode {
        "train" => {
            let map = VarMap::new();
            let corpus = Corpus::load("dataset.json");

            let texts = corpus
                .items
                .iter()
                .flat_map(|item| vec![item.phrase.clone(), item.command.clone()]);
            let tokenizer = Tokenizer::train(texts);
            tokenizer.save("tokenizer.json");

            let mut length = 0;
            for item in &corpus.items {
                let phrase = tokenizer.encode(&item.phrase);
                let command = tokenizer.encode(&item.command);
                let total = phrase.len() + command.len() + 3;
                if total > length {
                    length = total;
                }
            }

            let (train_data, valid_data) = corpus.split();

            let (train_in, train_out) = generate(&train_data, &tokenizer, length, &device)?;
            let (valid_in, valid_out) = generate(&valid_data, &tokenizer, length, &device)?;

            let config = Config {
                vocab: tokenizer.size(),
                dim: 128,
                heads: 8,
                limit: 256,
            };
            config.save("config.json");

            train::execute(&train_in, &train_out, &valid_in, &valid_out, &map, &device, &config)?;
        }
        "infer" => {
            let default = String::from("");
            let query = args.get(2).unwrap_or(&default);
            let tokenizer = Tokenizer::load("tokenizer.json");
            let config = Config::load("config.json");

            let mut encoded = Vec::new();
            encoded.push(1);
            encoded.extend(tokenizer.encode(query));
            encoded.push(3);

            let output = infer::execute(&encoded, &device, &config)?;
            let text = tokenizer.decode(&output);

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
