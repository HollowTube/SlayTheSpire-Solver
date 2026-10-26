//! Transport failures surface as typed errors through `Backend`, against a
//! local stub listener (no game).

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use sts_bridge::mcptest::{TcpTransport, MAX_RESPONSE_BYTES};
use sts_bridge::{Backend, BridgeError, McpTestBackend, ProtocolError};

const TIMEOUT: Duration = Duration::from_millis(500);

fn backend(port: u16) -> McpTestBackend<TcpTransport> {
    McpTestBackend::new(TcpTransport::new("127.0.0.1", port).with_timeout(TIMEOUT))
}

/// Accepts one connection, reads the request line, then hands the socket to `reply`.
fn stub(reply: impl FnOnce(TcpStream, String) + Send + 'static) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut request = String::new();
        BufReader::new(stream.try_clone().unwrap())
            .read_line(&mut request)
            .unwrap();
        reply(stream, request);
    });
    port
}

fn replying(bytes: Vec<u8>) -> u16 {
    stub(move |mut s, _| {
        let _ = s.write_all(&bytes);
    })
}

fn protocol_kind(result: Result<impl std::fmt::Debug, BridgeError>) -> ProtocolError {
    match result {
        Err(BridgeError::Protocol { kind, .. }) => kind,
        other => panic!("expected a Protocol error, got {other:?}"),
    }
}

#[test]
fn sends_a_newline_terminated_json_rpc_request_and_decodes_the_reply() {
    let (tx, rx) = std::sync::mpsc::channel();
    let port = stub(move |mut s, request| {
        tx.send(request).unwrap();
        s.write_all(
            b"\xef\xbb\xbf{\"result\":{\"status\":\"ok\",\"mod\":\"MCPTest\"},\"id\":1}\r\n",
        )
        .unwrap();
    });
    let health = backend(port).health().unwrap();
    assert!(health.ok);
    let request: serde_json::Value = serde_json::from_str(rx.recv().unwrap().trim_end()).unwrap();
    assert_eq!(request["method"], "ping");
    assert!(request["id"].is_u64());
}

#[test]
fn refused_connection_is_refused() {
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    assert!(matches!(
        backend(port).state(),
        Err(BridgeError::Refused { .. })
    ));
}

#[test]
fn silent_listener_times_out_within_the_configured_timeout() {
    let port = stub(|s, _| {
        thread::sleep(TIMEOUT * 4);
        drop(s);
    });
    let started = Instant::now();
    match backend(port).state() {
        Err(BridgeError::Timeout { method, after }) => {
            assert_eq!(method, "get_available_actions");
            assert_eq!(after, TIMEOUT);
        }
        other => panic!("expected a Timeout, got {other:?}"),
    }
    assert!(
        started.elapsed() < TIMEOUT * 3,
        "took {:?}",
        started.elapsed()
    );
}

#[test]
fn empty_reply_is_a_protocol_error() {
    assert_eq!(
        protocol_kind(backend(replying(Vec::new())).health()),
        ProtocolError::Empty
    );
}

#[test]
fn non_json_reply_is_a_protocol_error() {
    let kind = protocol_kind(backend(replying(b"Bridge says hi\n".to_vec())).health());
    assert!(matches!(kind, ProtocolError::InvalidJson(_)), "{kind:?}");
}

#[test]
fn non_utf8_reply_is_a_protocol_error() {
    assert_eq!(
        protocol_kind(backend(replying(b"\xff\xfe{}\n".to_vec())).health()),
        ProtocolError::NonUtf8
    );
}

#[test]
fn oversize_reply_is_a_protocol_error() {
    let port = replying(vec![b' '; MAX_RESPONSE_BYTES + 8192]);
    assert_eq!(
        protocol_kind(backend(port).health()),
        ProtocolError::TooLarge {
            limit: MAX_RESPONSE_BYTES
        }
    );
}

#[test]
fn error_reply_is_a_remote_error() {
    let port = replying(b"{\"result\":{\"error\":\"No run in progress\"},\"id\":1}\n".to_vec());
    match backend(port).actions() {
        Err(BridgeError::Remote { method, message }) => {
            assert_eq!(method, "get_available_actions");
            assert_eq!(message, "No run in progress");
        }
        other => panic!("expected a Remote error, got {other:?}"),
    }
}

#[test]
fn hostname_tries_every_resolved_address() {
    let port =
        replying(b"{\"result\":{\"status\":\"ok\",\"mod\":\"MCPTest\"},\"id\":1}\n".to_vec());
    let mut backend =
        McpTestBackend::new(TcpTransport::new("localhost", port).with_timeout(TIMEOUT));
    assert!(backend.health().unwrap().ok);
}
