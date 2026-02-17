//! Shared constants for the subportal protocol.

use std::path::PathBuf;

/// Protocol version string.
pub const VERSION: &str = "0.1.0";

/// Maximum file size for OpenFile (5 MB).
pub const MAX_FILE_SIZE: usize = 5 * 1024 * 1024;

/// Maximum wire message size (~8 MB, enough for base64-encoded 5MB file + JSON overhead).
pub const MAX_MESSAGE_SIZE: usize = 8 * 1024 * 1024;

/// Directory name under `$XDG_RUNTIME_DIR` for the socket.
pub const SOCKET_DIR: &str = "subportal";

/// Socket file name.
pub const SOCKET_NAME: &str = "subportal.sock";

/// Environment variable to override the socket path.
pub const SOCKET_PATH_ENV: &str = "SUBPORTAL_SOCKET";

/// Return the default socket path: `$XDG_RUNTIME_DIR/subportal/subportal.sock`.
///
/// Falls back to `/tmp/subportal-<uid>/subportal.sock` if `XDG_RUNTIME_DIR` is
/// not set.
pub fn default_socket_path() -> PathBuf {
    let base = match std::env::var("XDG_RUNTIME_DIR") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => {
            let uid = unsafe { libc::getuid() };
            PathBuf::from(format!("/tmp/subportal-{uid}"))
        }
    };
    base.join(SOCKET_DIR).join(SOCKET_NAME)
}
