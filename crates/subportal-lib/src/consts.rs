/// Default TCP port for the subportal daemon.
pub const DEFAULT_PORT: u16 = 19494;

/// Protocol version string.
pub const VERSION: &str = "0.1.0";

/// Maximum file size for OpenFile (5 MB).
pub const MAX_FILE_SIZE: usize = 5 * 1024 * 1024;

/// Maximum wire message size (~8 MB, enough for base64-encoded 5MB file + JSON overhead).
pub const MAX_MESSAGE_SIZE: usize = 8 * 1024 * 1024;

/// Environment variable to override the port.
pub const PORT_ENV: &str = "SUBPORTAL_PORT";
