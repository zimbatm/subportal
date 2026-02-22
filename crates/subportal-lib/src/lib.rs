//! Shared library for the subportal protocol.
//!
//! subportal bridges a headless server to the user's local desktop by
//! forwarding requests like "open URL" and "show notification" via iroh
//! (peer-to-peer QUIC). This crate provides the building blocks used by the
//! agent, the client daemon (`subportald`), and the server-side CLI tools.
//!
//! # Modules
//!
//! - [`protocol`] -- Varlink wire types, typed request/response enums, and
//!   NUL-delimited JSON I/O.
//! - [`client`] -- Unix socket client that connects to the daemon and sends
//!   requests.
//! - [`server`] -- Unix socket server that accepts connections and dispatches
//!   requests, with peer identity resolution via `SO_PEERCRED`.
//! - [`consts`] -- Shared constants (socket path, size limits, version).
//!
//! # Wire format
//!
//! The protocol uses [Varlink](https://varlink.org/) framing: each Unix socket
//! connection carries exactly one JSON request followed by one JSON response,
//! both terminated by a NUL byte (`0x00`).

pub mod client;
pub mod consts;
pub mod protocol;
pub mod server;
