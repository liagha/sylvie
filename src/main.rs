mod config;
mod data;
mod generate;
mod infer;
mod message;
mod model;
mod tokens;
mod train;

use candle_core::{Device, Result, Tensor};
use candle_nn::VarMap;
use config::Config;
use data::Corpus;
use message::Message;
use std::collections::HashSet;
use std::env;
use std::io::{self, BufRead, Write};
use std::process::Command;
use tokens::Tokenizer;

fn generate(
    corpus: &Corpus,
    tokenizer: &Tokenizer,
    length: usize,
    device: &Device,
) -> Result<(Tensor, Tensor, Tensor)> {
    let count = corpus.items.len();
    let mut sequences = Vec::with_capacity(count * length);
    let mut masks = Vec::with_capacity(count * length);

    for item in &corpus.items {
        let mut row = Vec::with_capacity(length);
        row.push(1);
        row.extend(tokenizer.encode(&item.phrase.to_string()));
        row.push(3);
        row.extend(tokenizer.encode(&item.command.to_string()));
        row.push(2);

        let mut mask_row = vec![0.0f32; length];
        let sep_pos = 1 + tokenizer.encode(&item.phrase.to_string()).len();
        let eos_pos = sep_pos + 1 + tokenizer.encode(&item.command.to_string()).len();
        for i in sep_pos + 1..=eos_pos {
            if i < length {
                mask_row[i] = 1.0;
            }
        }

        while row.len() < length {
            row.push(0);
        }

        sequences.extend(row);
        masks.extend(mask_row);
    }

    let inputs = Tensor::from_vec(sequences.clone(), (count, length), device)?;
    let targets = Tensor::from_vec(sequences, (count, length), device)?;
    let mask = Tensor::from_vec(masks, (count, length), device)?;

    Ok((inputs, targets, mask))
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
            generate::execute();
        }
        "train" => {
            let corpus = Corpus::load("dataset.json");
            let tokenizer = if std::path::Path::new("tokenizer.json").exists() {
                Tokenizer::load("tokenizer.json")
            } else {
                let texts = corpus
                    .items
                    .iter()
                    .flat_map(|item| vec![item.phrase.to_string(), item.command.to_string()]);
                let tokenizer = Tokenizer::train(texts);
                tokenizer.save("tokenizer.json");
                tokenizer
            };

            let mut length = 0;
            for item in &corpus.items {
                let phrase = tokenizer.encode(&item.phrase.to_string());
                let command = tokenizer.encode(&item.command.to_string());
                let total = phrase.len() + command.len() + 3;
                if total > length {
                    length = total;
                }
            }

            let (train_set, valid_set) = corpus.split();

            let (train_in, train_out, train_mask) =
                generate(&train_set, &tokenizer, length, &device)?;
            let (valid_in, valid_out, valid_mask) =
                generate(&valid_set, &tokenizer, length, &device)?;

            let config = Config {
                vocab: tokenizer.size(),
                dim: 512,
                heads: 8,
                layers: 8,
                limit: 512,
                drop: 0.1,
            };

            config.save("config.json");

            let mut map = VarMap::new();
            if std::path::Path::new("weights.safetensors").exists() {
                map.load("weights.safetensors")?;
            }

            train::execute(
                &train_in,
                &train_out,
                &train_mask,
                &valid_in,
                &valid_out,
                &valid_mask,
                &map,
                &device,
                &config,
            )?;
        }
        "infer" => {
            let tokenizer = Tokenizer::load("tokenizer.json");
            let config = Config::load("config.json");
            let mut corpus = Corpus::load("dataset.json");

            let stdin = io::stdin();
            let mut handle = stdin.lock();

            loop {
                print!("query: ");
                io::stdout().flush()?;

                let mut query = String::new();
                handle.read_line(&mut query)?;
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
                        let phrase = tokenizer.encode(&item.phrase.to_string());
                        let command = tokenizer.encode(&item.command.to_string());
                        let total = phrase.len() + command.len() + 3;
                        if total > length {
                            length = total;
                        }
                    }

                    let (train_set, valid_set) = corpus.clone().split();
                    let (train_in, train_out, train_mask) =
                        generate(&train_set, &tokenizer, length, &device)?;
                    let (valid_in, valid_out, valid_mask) =
                        generate(&valid_set, &tokenizer, length, &device)?;

                    train::execute(
                        &train_in,
                        &train_out,
                        &train_mask,
                        &valid_in,
                        &valid_out,
                        &valid_mask,
                        &map,
                        &device,
                        &config,
                    )?;
                    println!("training finished");
                    continue;
                }

                if query.is_empty() {
                    continue;
                }

                let mut encoded = Vec::new();
                encoded.push(1);
                let phrase = Message::Text(query.to_string());
                encoded.extend(tokenizer.encode(&phrase.to_string()));
                encoded.push(3);

                let output = infer::execute(&encoded, &device, &config)?;
                let text = tokenizer.decode(&output);

                let message: Message = match text.parse() {
                    Ok(m) => m,
                    Err(_) => {
                        println!("sylvie (raw): {}", text);
                        Message::Text(text.clone())
                    }
                };

                println!("sylvie: {}", message.to_string());

                match &message {
                    Message::Command(cmd) => {
                        print!("execute? (y/n/override): ");
                        io::stdout().flush()?;
                        let mut answer = String::new();
                        handle.read_line(&mut answer)?;
                        let answer = answer.trim();

                        if answer.eq_ignore_ascii_case("y") {
                            let parts: Vec<&str> = cmd.split_whitespace().collect();
                            if parts.is_empty() {
                                continue;
                            }
                            if !allowlist.contains(parts[0]) {
                                println!("blocked for safety");
                                continue;
                            }
                            let args: Vec<&str> = if parts.len() > 1 {
                                parts[1..].to_vec()
                            } else {
                                vec![]
                            };
                            let mut process = Command::new(parts[0]);
                            if !args.is_empty() {
                                process.args(&args);
                            }
                            process.status()?;
                        } else if !answer.eq_ignore_ascii_case("n") && !answer.is_empty() {
                            corpus.items.push(data::Record {
                                phrase: Message::Text(query.to_string()),
                                command: Message::Command(answer.to_string()),
                            });
                        }
                    }
                    Message::Trigger(name) => {
                        println!("trigger '{}' not implemented yet", name);
                    }
                    Message::Text(txt) => {
                        println!("sylvie says: {}", txt);
                    }
                }
            }
        }
        _ => panic!("invalid"),
    }

    Ok(())
}