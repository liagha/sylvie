use crate::data::{Corpus, Record};
use std::collections::HashSet;
use std::fs;

pub fn execute() {
    let mut existing = if let Ok(corpus) = fs::read_to_string("dataset.json") {
        serde_json::from_str::<Corpus>(&corpus).unwrap_or(Corpus { items: vec![] })
    } else {
        Corpus { items: vec![] }
    };

    let mut seen = HashSet::new();
    existing.items.retain(|record| seen.insert((record.phrase.clone(), record.command.clone())));

    let mut new = Vec::new();

    let folders = ["/tmp/test", "/var/www", "build", "dist", "src", ".git", "node_modules"];
    let actions = ["delete folder", "remove directory", "wipe", "delete"];
    for folder in &folders {
        for action in &actions {
            let record = Record {
                phrase: format!("{} {}", action, folder),
                command: format!("rm -rf {}", folder),
            };
            if seen.insert((record.phrase.clone(), record.command.clone())) {
                new.push(record);
            }
        }
    }

    let ips = ["8.8.8.8", "1.1.1.1", "127.0.0.1", "192.168.1.1", "10.0.0.1"];
    let checks = ["ping", "check connection to", "reach"];
    for ip in &ips {
        for check in &checks {
            let record = Record {
                phrase: format!("{} {}", check, ip),
                command: format!("ping {}", ip),
            };
            if seen.insert((record.phrase.clone(), record.command.clone())) {
                new.push(record);
            }
        }
    }

    let ports = ["80", "443", "8080", "22", "3306"];
    for port in &ports {
        for prefix in ["who is on port", "check port"] {
            let record = Record {
                phrase: format!("{} {}", prefix, port),
                command: format!("lsof -i :{}", port),
            };
            if seen.insert((record.phrase.clone(), record.command.clone())) {
                new.push(record);
            }
        }
    }

    let services = ["docker", "nginx", "apache2", "sshd", "mysql"];
    let states = ["start", "stop", "restart", "reload", "status"];
    for service in &services {
        for state in &states {
            let record = Record {
                phrase: format!("{} {}", state, service),
                command: format!("systemctl {} {}", state, service),
            };
            if seen.insert((record.phrase.clone(), record.command.clone())) {
                new.push(record);
            }
        }
    }

    existing.items.extend(new);

    let content = serde_json::to_string_pretty(&existing).unwrap();
    fs::write("dataset.json", content).unwrap();

    println!("generated dataset ({} items)", existing.items.len());
}