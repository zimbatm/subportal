//! xdg-open -- drop-in replacement that forwards to subportal.
//!
//! When placed earlier in `$PATH` than the real `xdg-open`, this binary
//! transparently forwards URL and file open requests through the subportal
//! tunnel to the client's desktop.

use std::path::Path;
use std::process::ExitCode;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use subportal_lib::client::Client;
use subportal_lib::consts::MAX_FILE_SIZE;
use subportal_lib::protocol::{Request, SubportalError};

fn is_url(target: &str) -> bool {
    // Match scheme://... patterns (http://, https://, ftp://, etc.)
    target
        .find("://")
        .map(|pos| pos > 0 && target[..pos].chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.'))
        .unwrap_or(false)
}

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() || args[0] == "--help" {
        eprintln!(
            "xdg-open (subportal {}) - forwards to your desktop via SSH",
            subportal_lib::consts::VERSION,
        );
        eprintln!();
        eprintln!("Usage: xdg-open {{file | URL}}");
        eprintln!("       xdg-open {{--help | --version}}");
        eprintln!();
        eprintln!("Opens a file or URL on the client desktop via the subportal");
        eprintln!("SSH tunnel. Files up to 5 MB are transferred and opened in");
        eprintln!("the client's preferred application. URLs are opened in the");
        eprintln!("client's browser. Both require user confirmation.");
        return if args.is_empty() {
            ExitCode::from(1)
        } else {
            ExitCode::SUCCESS
        };
    }

    if args[0] == "--version" {
        eprintln!("xdg-open (subportal {})", subportal_lib::consts::VERSION);
        return ExitCode::SUCCESS;
    }

    let target = &args[0];
    let client = Client::new();

    // Exit codes follow xdg-open convention:
    // 1 = syntax error, 2 = file not found, 3 = tool missing, 4 = action failed
    let request = if is_url(target) {
        Request::OpenURI {
            uri: target.clone(),
        }
    } else {
        // It's a file path -- read, encode, and send
        let path = Path::new(target);
        let data = match std::fs::read(path) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("xdg-open: cannot read '{}': {}", target, e);
                return ExitCode::from(2);
            }
        };

        if data.len() > MAX_FILE_SIZE {
            eprintln!(
                "xdg-open: file '{}' is too large ({} bytes, max {} bytes)",
                target,
                data.len(),
                MAX_FILE_SIZE
            );
            return ExitCode::from(4);
        }

        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "file".to_string());

        let mime = mime_guess::from_path(path)
            .first_or_octet_stream()
            .to_string();

        let content = BASE64.encode(&data);

        Request::OpenFile {
            name,
            mime,
            content,
        }
    };

    match client.call(&request).await {
        Ok(_) => ExitCode::SUCCESS,
        Err(SubportalError::NoClient) => {
            let path = client.path();
            eprintln!("xdg-open: subportal daemon is not reachable");
            if path.exists() {
                eprintln!("  socket: {} (exists but not responding)", path.display());
            } else {
                eprintln!("  socket: {} (not found)", path.display());
            }
            ExitCode::from(3)
        }
        Err(SubportalError::UserDenied) => {
            eprintln!("xdg-open: request was denied by the user");
            ExitCode::from(4)
        }
        Err(e) => {
            eprintln!("xdg-open: {}", e);
            ExitCode::from(4)
        }
    }
}
