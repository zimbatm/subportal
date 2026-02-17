//! notify-send -- drop-in replacement that forwards to subportal.
//!
//! When placed earlier in `$PATH` than the real `notify-send`, this binary
//! transparently forwards desktop notifications through the subportal tunnel
//! to the client's desktop. Parses standard `notify-send` flags for
//! compatibility.

use std::process::ExitCode;

use subportal_lib::client::Client;
use subportal_lib::protocol::{Request, SubportalError};

fn print_usage() {
    eprintln!(
        "notify-send (subportal {}) - forwards to your desktop via SSH",
        subportal_lib::consts::VERSION,
    );
    eprintln!();
    eprintln!("Usage: notify-send [OPTIONS] <TITLE> [BODY]");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -u, --urgency LEVEL   Urgency level (low, normal, critical)");
    eprintln!("  -i, --icon ICON       Icon name");
    eprintln!("  -t, --expire-time MS  Not supported by desktop portal");
    eprintln!("  -a, --app-name NAME   Not supported by desktop portal");
    eprintln!("  -c, --category TYPE   Not supported by desktop portal");
    eprintln!("  -h, --help            Show this help");
    eprintln!("  --version             Show version");
    eprintln!();
    eprintln!("Unsupported notify-send flags (--action, --replace-id, --print-id,");
    eprintln!("--wait, --transient, --hint) are silently ignored for compatibility.");
}

struct Args {
    title: String,
    body: Option<String>,
    urgency: Option<String>,
    icon: Option<String>,
}

fn parse_args() -> Option<Args> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut urgency = None;
    let mut icon = None;
    let mut positional = Vec::new();
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            "--version" => {
                eprintln!("notify-send (subportal {})", subportal_lib::consts::VERSION);
                std::process::exit(0);
            }
            "-u" | "--urgency" => {
                i += 1;
                urgency = args.get(i).cloned();
            }
            "-i" | "--icon" => {
                i += 1;
                icon = args.get(i).cloned();
            }
            // Flags that take an argument: accept but ignore
            "-t" | "--expire-time" | "-a" | "--app-name" | "-c" | "--category"
            | "-A" | "--action" | "-r" | "--replace-id" | "--hint" => {
                i += 1;
            }
            // Flags that are standalone: accept but ignore
            "-p" | "--print-id" | "-w" | "--wait" | "-e" | "--transient" => {}
            other => {
                // Handle --flag=value style
                if let Some(val) = other.strip_prefix("--urgency=") {
                    urgency = Some(val.to_string());
                } else if let Some(val) = other.strip_prefix("--icon=") {
                    icon = Some(val.to_string());
                } else if other.starts_with("--") && other.contains('=') {
                    // Unknown --key=value flag, ignore
                } else if other.starts_with('-') {
                    // Unknown flag, ignore
                } else {
                    positional.push(other.to_string());
                }
            }
        }
        i += 1;
    }

    if positional.is_empty() {
        return None;
    }

    let title = positional.remove(0);
    let body = if positional.is_empty() {
        None
    } else {
        Some(positional.join(" "))
    };

    Some(Args {
        title,
        body,
        urgency,
        icon,
    })
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = match parse_args() {
        Some(a) => a,
        None => {
            print_usage();
            return ExitCode::from(1);
        }
    };

    let client = Client::new();
    let request = Request::Notify {
        title: args.title,
        body: args.body,
        urgency: args.urgency,
        icon: args.icon,
    };

    match client.call(&request).await {
        Ok(_) => ExitCode::SUCCESS,
        Err(SubportalError::NoClient) => {
            let path = client.path();
            eprintln!("notify-send: subportal daemon is not reachable");
            if path.exists() {
                eprintln!("  socket: {} (exists but not responding)", path.display());
            } else {
                eprintln!("  socket: {} (not found)", path.display());
            }
            ExitCode::from(1)
        }
        Err(e) => {
            eprintln!("notify-send: {}", e);
            ExitCode::from(1)
        }
    }
}
