//! Bridge functions for Varlink messages over QUIC streams.
//!
//! Wraps iroh/quinn send/recv streams with the NUL-delimited JSON protocol
//! from `subportal_lib::protocol`.

use anyhow::Context;
use iroh::endpoint::{RecvStream, SendStream};
use serde::{de::DeserializeOwned, Serialize};
use subportal_lib::protocol::{read_message, write_message};
use tokio::io::BufReader;

/// Send a request over a QUIC send stream and finish it.
pub async fn send_request(send: &mut SendStream, req: &impl Serialize) -> anyhow::Result<()> {
    write_message(send, req)
        .await
        .context("failed to send request")?;
    send.finish().context("failed to finish send stream")?;
    Ok(())
}

/// Receive a request from a QUIC recv stream.
pub async fn recv_request<T: DeserializeOwned>(recv: &mut RecvStream) -> anyhow::Result<T> {
    let mut reader = BufReader::new(recv);
    read_message(&mut reader)
        .await
        .context("failed to receive request")
}

/// Send a response over a QUIC send stream and finish it.
pub async fn send_response(send: &mut SendStream, resp: &impl Serialize) -> anyhow::Result<()> {
    write_message(send, resp)
        .await
        .context("failed to send response")?;
    send.finish().context("failed to finish send stream")?;
    Ok(())
}

/// Receive a response from a QUIC recv stream.
pub async fn recv_response<T: DeserializeOwned>(recv: &mut RecvStream) -> anyhow::Result<T> {
    let mut reader = BufReader::new(recv);
    read_message(&mut reader)
        .await
        .context("failed to receive response")
}
