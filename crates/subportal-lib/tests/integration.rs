use subportal_lib::client::Client;
use subportal_lib::protocol::{Request, Response, SubportalError};
use subportal_lib::server::Server;

/// Create a temp directory and return a socket path inside it, along with
/// a server bound to that path and a client connected to it.
async fn spawn_server(
    handler: impl Fn(Request) -> Result<Response, SubportalError> + Send + 'static,
) -> (Client, tokio::task::JoinHandle<()>) {
    let tmp = tempfile::tempdir().unwrap();
    let sock = tmp.path().join("test.sock");
    let server = Server::bind(&sock).await.unwrap();
    let client = Client::with_path(&sock);
    let handle = tokio::spawn(async move {
        let (req, _host, responder) = server.accept().await.unwrap();
        match handler(req) {
            Ok(resp) => responder.send_ok(resp).await.unwrap(),
            Err(err) => responder.send_error(err).await.unwrap(),
        }
        // Keep tmp alive until the handler finishes so the socket isn't removed.
        drop(tmp);
    });
    (client, handle)
}

#[tokio::test]
async fn ping_round_trip() {
    let (client, handle) = spawn_server(|req| {
        assert_eq!(req, Request::Ping);
        Ok(Response::Ping {
            capabilities: vec!["open_uri".into()],
            version: "0.1.0".into(),
        })
    })
    .await;

    let resp = client.call(&Request::Ping).await.unwrap();
    assert_eq!(
        resp,
        Response::Ping {
            capabilities: vec!["open_uri".into()],
            version: "0.1.0".into(),
        }
    );
    handle.await.unwrap();
}

#[tokio::test]
async fn open_uri_round_trip() {
    let (client, handle) = spawn_server(|req| {
        assert_eq!(
            req,
            Request::OpenURI {
                uri: "https://example.com".into()
            }
        );
        Ok(Response::Ok)
    })
    .await;

    let resp = client
        .call(&Request::OpenURI {
            uri: "https://example.com".into(),
        })
        .await
        .unwrap();
    assert_eq!(resp, Response::Ok);
    handle.await.unwrap();
}

#[tokio::test]
async fn open_file_round_trip() {
    let (client, handle) = spawn_server(|req| {
        assert_eq!(
            req,
            Request::OpenFile {
                name: "doc.pdf".into(),
                mime: "application/pdf".into(),
                content: "JVBER".into(),
            }
        );
        Ok(Response::Ok)
    })
    .await;

    let resp = client
        .call(&Request::OpenFile {
            name: "doc.pdf".into(),
            mime: "application/pdf".into(),
            content: "JVBER".into(),
        })
        .await
        .unwrap();
    assert_eq!(resp, Response::Ok);
    handle.await.unwrap();
}

#[tokio::test]
async fn notify_round_trip_full() {
    let (client, handle) = spawn_server(|req| {
        assert_eq!(
            req,
            Request::Notify {
                title: "Build done".into(),
                body: Some("All 42 tests passed".into()),
                urgency: Some("normal".into()),
                icon: Some("dialog-information".into()),
            }
        );
        Ok(Response::Ok)
    })
    .await;

    let resp = client
        .call(&Request::Notify {
            title: "Build done".into(),
            body: Some("All 42 tests passed".into()),
            urgency: Some("normal".into()),
            icon: Some("dialog-information".into()),
        })
        .await
        .unwrap();
    assert_eq!(resp, Response::Ok);
    handle.await.unwrap();
}

#[tokio::test]
async fn notify_round_trip_minimal() {
    let (client, handle) = spawn_server(|req| {
        assert_eq!(
            req,
            Request::Notify {
                title: "Hi".into(),
                body: None,
                urgency: None,
                icon: None,
            }
        );
        Ok(Response::Ok)
    })
    .await;

    let resp = client
        .call(&Request::Notify {
            title: "Hi".into(),
            body: None,
            urgency: None,
            icon: None,
        })
        .await
        .unwrap();
    assert_eq!(resp, Response::Ok);
    handle.await.unwrap();
}

#[tokio::test]
async fn error_user_denied() {
    let (client, handle) = spawn_server(|_| Err(SubportalError::UserDenied)).await;

    let err = client.call(&Request::Ping).await.unwrap_err();
    assert_eq!(err, SubportalError::UserDenied);
    handle.await.unwrap();
}

#[tokio::test]
async fn error_not_supported() {
    let (client, handle) = spawn_server(|_| {
        Err(SubportalError::NotSupported {
            capability: "OpenFile".into(),
        })
    })
    .await;

    let err = client.call(&Request::Ping).await.unwrap_err();
    assert_eq!(
        err,
        SubportalError::NotSupported {
            capability: "OpenFile".into()
        }
    );
    handle.await.unwrap();
}

#[tokio::test]
async fn error_file_too_large() {
    let (client, handle) = spawn_server(|_| {
        Err(SubportalError::FileTooLarge {
            max_bytes: 5_242_880,
        })
    })
    .await;

    let err = client.call(&Request::Ping).await.unwrap_err();
    assert_eq!(
        err,
        SubportalError::FileTooLarge {
            max_bytes: 5_242_880
        }
    );
    handle.await.unwrap();
}

#[tokio::test]
async fn no_server_returns_no_client() {
    let tmp = tempfile::tempdir().unwrap();
    let sock = tmp.path().join("nonexistent.sock");
    let client = Client::with_path(sock);
    let err = client.call(&Request::Ping).await.unwrap_err();
    assert_eq!(err, SubportalError::NoClient);
}

#[tokio::test]
async fn multiple_sequential_requests() {
    let tmp = tempfile::tempdir().unwrap();
    let sock = tmp.path().join("test.sock");
    let server = Server::bind(&sock).await.unwrap();
    let client = Client::with_path(&sock);

    let handle = tokio::spawn(async move {
        for _ in 0..3 {
            let (_, _, responder) = server.accept().await.unwrap();
            responder.send_ok(Response::Ok).await.unwrap();
        }
    });

    for _ in 0..3 {
        let resp = client
            .call(&Request::OpenURI {
                uri: "https://example.com".into(),
            })
            .await
            .unwrap();
        assert_eq!(resp, Response::Ok);
    }

    handle.await.unwrap();
}
