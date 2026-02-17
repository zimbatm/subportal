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

    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        eprintln!("Usage: xdg-open <URL|file>");
        return if args.is_empty() {
            ExitCode::from(1)
        } else {
            ExitCode::SUCCESS
        };
    }

    let target = &args[0];
    let client = Client::new();

    let request = if is_url(target) {
        Request::OpenURI {
            uri: target.clone(),
        }
    } else {
        // It's a file path — read, encode, and send
        let path = Path::new(target);
        let data = match std::fs::read(path) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("xdg-open: cannot read '{}': {}", target, e);
                return ExitCode::from(1);
            }
        };

        if data.len() > MAX_FILE_SIZE {
            eprintln!(
                "xdg-open: file '{}' is too large ({} bytes, max {} bytes)",
                target,
                data.len(),
                MAX_FILE_SIZE
            );
            return ExitCode::from(1);
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
            eprintln!("xdg-open: subportal daemon is not reachable");
            ExitCode::from(1)
        }
        Err(SubportalError::UserDenied) => {
            eprintln!("xdg-open: request was denied by the user");
            ExitCode::from(1)
        }
        Err(e) => {
            eprintln!("xdg-open: {}", e);
            ExitCode::from(1)
        }
    }
}
