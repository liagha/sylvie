use std::io::{self, Write};

pub fn text(label: &str, default: Option<&str>) -> String {
    match default {
        Some(value) => print!("{label} [{value}]: "),
        None => print!("{label}: "),
    }
    io::stdout().flush().expect("stdout");
    let mut line = String::new();
    io::stdin().read_line(&mut line).expect("stdin readable");
    let answer = line.trim();
    match (answer.is_empty(), default) {
        (true, Some(value)) => value.to_string(),
        _ => answer.to_string(),
    }
}

pub fn hidden(label: &str) -> String {
    rpassword::prompt_password(format!("{label}: ")).expect("terminal")
}

pub fn hidden_twice(label: &str) -> String {
    let first = hidden(label);
    let second = hidden("repeat");
    if first != second {
        eprintln!("error: passwords differ");
        std::process::exit(1);
    }
    first
}
