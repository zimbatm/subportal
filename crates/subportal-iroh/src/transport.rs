//! Bridge functions for Varlink messages over QUIC streams.
//!
//! Wraps iroh/quinn send/recv streams with the NUL-delimited JSON protocol
//! from `subportal_lib::protocol`.

use anyhow::Context;
use iroh::endpoint::{RecvStream, SendStream};
use subportal_lib::protocol::{read_message, write_message, VarlinkRequest, VarlinkResponse};
use tokio::io::BufReader;

/// Send a Varlink request over a QUIC send stream and finish it.
pub async fn send_request(send: &mut SendStream, req: &VarlinkRequest) -> anyhow::Result<()> {
    write_message(send, req)
        .await
        .context("failed to send request")?;
    send.finish().context("failed to finish send stream")?;
    Ok(())
}

/// Receive a Varlink request from a QUIC recv stream.
pub async fn recv_request(recv: &mut RecvStream) -> anyhow::Result<VarlinkRequest> {
    let mut reader = BufReader::new(recv);
    read_message(&mut reader)
        .await
        .context("failed to receive request")
}

/// Send a Varlink response over a QUIC send stream and finish it.
pub async fn send_response(send: &mut SendStream, resp: &VarlinkResponse) -> anyhow::Result<()> {
    write_message(send, resp)
        .await
        .context("failed to send response")?;
    send.finish().context("failed to finish send stream")?;
    Ok(())
}

/// Receive a Varlink response from a QUIC recv stream.
pub async fn recv_response(recv: &mut RecvStream) -> anyhow::Result<VarlinkResponse> {
    let mut reader = BufReader::new(recv);
    read_message(&mut reader)
        .await
        .context("failed to receive response")
}
