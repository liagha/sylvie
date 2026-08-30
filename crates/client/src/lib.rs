// sylvie: command line client for the Sylvie hub. Exposes the CLI logic so it
// can run in-process (tests, embedding) as well as from the `sylvie` binary.

mod ask;
mod commands;
mod config;
mod net;
mod session;

use clap::{Parser, Subcommand};
use std::ffi::OsString;

#[derive(Parser)]
#[command(name = "sylvie", version, about = "personal hub client")]
struct Cli {
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
enum Command {
    Register {
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        user: Option<String>,
        #[arg(long, hide = true)]
        password: Option<String>,
        #[arg(long)]
        name: Option<String>,
    },
    Login {
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        user: Option<String>,
        #[arg(long, hide = true)]
        password: Option<String>,
        #[arg(long)]
        name: Option<String>,
    },
    Logout,
    Status,
    Token,
    Passwd {
        #[arg(long, hide = true)]
        new: Option<String>,
    },
    Secret {
        #[command(subcommand)]
        cmd: commands::secret::Command,
    },
    File {
        #[command(subcommand)]
        cmd: commands::file::Command,
    },
    Device {
        #[command(subcommand)]
        cmd: commands::device::Command,
    },
}

pub async fn run_from_args<I, T>(args: I) -> Result<(), sylvie_core::error::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = Cli::parse_from(args);
    run(cli).await
}

pub fn exec() {
    let args: Vec<OsString> = std::env::args_os().collect();
    match tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(run_from_args(args))
    {
        Ok(()) => {}
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
    }
}

struct Credentials {
    url: String,
    user: String,
}

async fn run(cli: Cli) -> Result<(), sylvie_core::error::Error> {
    let http = net::http()?;
    match cli.cmd {
        Command::Register {
            url,
            user,
            password,
            name,
        } => {
            let input = credentials(url, user)?;
            commands::auth::register(&http, &input.url, &input.user, password, name, cli.json).await
        }
        Command::Login {
            url,
            user,
            password,
            name,
        } => {
            let input = credentials(url, user)?;
            commands::auth::login(&http, &input.url, &input.user, password, name, cli.json).await
        }
        Command::Logout => commands::auth::logout(&http, cli.json).await,
        Command::Status => commands::auth::status(&http, cli.json).await,
        Command::Token => commands::auth::token(cli.json),
        Command::Passwd { new } => commands::auth::passwd(&http, new, cli.json).await,
        Command::Secret { cmd } => commands::secret::run(&http, cmd, cli.json).await,
        Command::File { cmd } => commands::file::run(&http, cmd, cli.json).await,
        Command::Device { cmd } => commands::device::run(&http, cmd, cli.json).await,
    }
}

fn credentials(
    url: Option<String>,
    user: Option<String>,
) -> Result<Credentials, sylvie_core::error::Error> {
    let saved = config::load()?;
    Ok(Credentials {
        url: url
            .or_else(|| saved.as_ref().map(|c| c.url.clone()))
            .unwrap_or_else(|| ask::text("server url", None)),
        user: user
            .or_else(|| saved.as_ref().map(|c| c.user.clone()))
            .unwrap_or_else(|| ask::text("username", None)),
    })
}
