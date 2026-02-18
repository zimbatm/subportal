//! Shared constants for the subportal protocol.

use std::path::PathBuf;

/// Protocol version string.
pub const VERSION: &str = "0.1.0";

/// Maximum file size for OpenFile (5 MB).
pub const MAX_FILE_SIZE: usize = 5 * 1024 * 1024;

/// Maximum wire message size (~8 MB, enough for base64-encoded 5MB file + JSON overhead).
pub const MAX_MESSAGE_SIZE: usize = 8 * 1024 * 1024;

/// Socket file name placed directly in `$XDG_RUNTIME_DIR`.
pub const SOCKET_NAME: &str = "subportal.sock";

/// Environment variable to override the socket path.
pub const SOCKET_PATH_ENV: &str = "SUBPORTAL_SOCKET";

/// Return the default socket path: `$XDG_RUNTIME_DIR/subportal.sock`.
///
/// Falls back to `/run/user/<uid>/subportal.sock` if `XDG_RUNTIME_DIR` is not
/// set. This assumes a systemd-based system where `/run/user/<uid>` exists.
pub fn default_socket_path() -> PathBuf {
    match std::env::var("XDG_RUNTIME_DIR") {
        Ok(dir) => PathBuf::from(dir).join(SOCKET_NAME),
        Err(_) => {
            let uid = unsafe { libc::getuid() };
            PathBuf::from(format!("/run/user/{uid}")).join(SOCKET_NAME)
        }
    }
}
