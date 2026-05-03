// FILE: src/main.rs
mod config;
mod data;
mod gen;
mod infer;
mod model;
mod tokens;
mod train;

use candle_core::{Device, Result, Tensor};
use candle_nn::VarMap;
use config::Config;
use data::Corpus;
use std::collections::HashSet;
use std::env;
use std::io::{self, BufRead, Write};
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

fn pick_device() -> Device {
    #[cfg(feature = "cuda")]
    {
        if let Ok(dev) = Device::new_cuda(0) {
            return dev;
        }
    }
    Device::Cpu
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("infer");

    let device = if args.iter().any(|a| a == "--cpu") {
        Device::Cpu
    } else {
        pick_device()
    };

    let allowlist: HashSet<&str> = [
        "ls", "pwd", "df", "free", "date", "ip", "ping",
        "docker", "ss", "find", "grep", "cat", "uname",
        "hostname", "who", "uptime", "tail", "head", "wc",
        "env", "echo", "whoami", "ps", "pstree", "dmesg",
        "lscpu", "lshw", "lsblk", "mount", "ufw",
        "systemctl", "journalctl", "last", "du", "sensors",
        "iostat", "vmstat", "top", "whois", "dig", "nslookup",
        "curl", "wget", "traceroute", "mtr", "netstat",
        "lsusb", "lspci", "hwinfo", "dmidecode", "passwd",
        "id", "groups", "crontab", "stat", "file",
    ]
        .iter()
        .cloned()
        .collect();

    match mode {
        "gen" => {
            gen::execute();
        }
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

            let (train_set, valid_set) = corpus.split();

            let (train_in, train_out) = generate(&train_set, &tokenizer, length, &device)?;
            let (valid_in, valid_out) = generate(&valid_set, &tokenizer, length, &device)?;

            let config = Config {
                vocab: tokenizer.size(),
                dim: 512,
                heads: 8,
                layers: 8,
                limit: 512,
                drop: 0.1,
            };

            config.save("config.json");

            train::execute(&train_in, &train_out, &valid_in, &valid_out, &map, &device, &config)?;
        }
        "infer" => {
            let tokenizer = Tokenizer::load("tokenizer.json");
            let config = Config::load("config.json");
            let mut corpus = Corpus::load("dataset.json");

            let stdin = io::stdin();
            let mut handle = stdin.lock();

            loop {
                print!("query: ");
                io::stdout().flush().unwrap();

                let mut query = String::new();
                handle.read_line(&mut query).unwrap();
                let query = query.trim();

                if query.eq_ignore_ascii_case("quit") || query.eq_ignore_ascii_case("exit") {
                    corpus.save("dataset.json");
                    break;
                }

                if query.eq_ignore_ascii_case("train") {
                    let mut map = VarMap::new();
                    if std::path::Path::new("weights.safetensors").exists() {
                        map.load("weights.safetensors")?;
                    }

                    let mut length = 0;
                    for item in &corpus.items {
                        let phrase = tokenizer.encode(&item.phrase);
                        let command = tokenizer.encode(&item.command);
                        let total = phrase.len() + command.len() + 3;
                        if total > length {
                            length = total;
                        }
                    }

                    let (train_set, valid_set) = corpus.clone().split();
                    let (train_in, train_out) = generate(&train_set, &tokenizer, length, &device)?;
                    let (valid_in, valid_out) = generate(&valid_set, &tokenizer, length, &device)?;

                    train::execute(&train_in, &train_out, &valid_in, &valid_out, &map, &device, &config)?;
                    println!("training finished");
                    continue;
                }

                if query.is_empty() {
                    continue;
                }

                let mut encoded = Vec::new();
                encoded.push(1);
                encoded.extend(tokenizer.encode(query));
                encoded.push(3);

                let output = infer::execute(&encoded, &device, &config)?;
                let text = tokenizer.decode(&output);

                println!("sylvie: {}", text);
                print!("feedback (y/n/command): ");
                io::stdout().flush().unwrap();

                let mut answer = String::new();
                handle.read_line(&mut answer).unwrap();
                let answer = answer.trim();

                if answer.eq_ignore_ascii_case("y") {
                    let parts: Vec<&str> = text.split_whitespace().collect();
                    if parts.is_empty() {
                        continue;
                    }
                    if !allowlist.contains(parts[0]) {
                        println!("sylvie: command blocked for safety");
                        continue;
                    }
                    let mut process = Command::new(parts[0]);
                    if parts.len() > 1 {
                        process.args(&parts[1..]);
                    }
                    process.status().unwrap();
                } else if !answer.eq_ignore_ascii_case("n") && !answer.is_empty() {
                    corpus.items.push(data::Record {
                        phrase: query.to_string(),
                        command: answer.to_string(),
                    });
                }
            }
        }
        _ => panic!("invalid"),
    }

    Ok(())
}