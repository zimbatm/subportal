//! Shared library for the subportal protocol.
//!
//! subportal bridges a headless SSH server to the user's local desktop by
//! forwarding requests like "open URL" and "show notification" through an SSH
//! tunnel. This crate provides the building blocks used by both the client
//! daemon (`subportald`) and the server-side CLI tools.
//!
//! # Modules
//!
//! - [`protocol`] -- Varlink wire types, typed request/response enums, and
//!   NUL-delimited JSON I/O over TCP.
//! - [`client`] -- TCP client that connects to the daemon and sends requests.
//! - [`server`] -- TCP server that accepts connections and dispatches requests.
//! - [`consts`] -- Shared constants (default port, size limits, version).
//!
//! # Wire format
//!
//! The protocol uses [Varlink](https://varlink.org/) framing: each TCP
//! connection carries exactly one JSON request followed by one JSON response,
//! both terminated by a NUL byte (`0x00`).

pub mod client;
pub mod consts;
pub mod protocol;
pub mod server;
