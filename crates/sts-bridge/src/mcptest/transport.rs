//! MCPTest wire transport: newline-delimited JSON-RPC over TCP, one connection
//! per request.

use std::collections::HashMap;
use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::Duration;

use serde_json::{json, Value};

use crate::backend::{BridgeError, ProtocolError};

pub const DEFAULT_PORT: u16 = 21337;
/// Must exceed the bridge's 10 s main-thread dispatch timeout.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(12);
pub const MAX_RESPONSE_BYTES: usize = 10 * 1024 * 1024;

/// Sends one request and returns the raw response line, undecoded.
pub trait Transport {
    fn send(&mut self, method: &str, params: Option<&Value>) -> Result<Vec<u8>, BridgeError>;
}

#[derive(Debug, Clone)]
pub struct TcpTransport {
    host: String,
    port: u16,
    timeout: Duration,
    next_id: u64,
}

impl TcpTransport {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
            timeout: DEFAULT_TIMEOUT,
            next_id: 1,
        }
    }

    /// `STS2_BRIDGE_HOST` (default `127.0.0.1`) on the default port.
    pub fn from_env() -> Self {
        let host = std::env::var("STS2_BRIDGE_HOST")
            .ok()
            .filter(|h| !h.is_empty())
            .unwrap_or_else(|| "127.0.0.1".to_string());
        Self::new(host, DEFAULT_PORT)
    }

    /// Applies to both connect and read.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    /// Tries each resolved address in turn, like `socket.create_connection`.
    fn connect(&self, method: &str) -> Result<TcpStream, BridgeError> {
        let addrs: Vec<SocketAddr> = (self.host.as_str(), self.port)
            .to_socket_addrs()
            .map_err(|source| BridgeError::Io {
                method: method.to_string(),
                source,
            })?
            .collect();
        let mut last = std::io::Error::new(
            ErrorKind::NotFound,
            format!("{} did not resolve", self.addr()),
        );
        for addr in addrs {
            match TcpStream::connect_timeout(&addr, self.timeout) {
                Ok(stream) => return Ok(stream),
                Err(e) => last = e,
            }
        }
        Err(self.io_error(method, last))
    }

    fn io_error(&self, method: &str, source: std::io::Error) -> BridgeError {
        match source.kind() {
            ErrorKind::ConnectionRefused => BridgeError::Refused { addr: self.addr() },
            ErrorKind::TimedOut | ErrorKind::WouldBlock => BridgeError::Timeout {
                method: method.to_string(),
                after: self.timeout,
            },
            _ => BridgeError::Io {
                method: method.to_string(),
                source,
            },
        }
    }
}

impl Transport for TcpTransport {
    fn send(&mut self, method: &str, params: Option<&Value>) -> Result<Vec<u8>, BridgeError> {
        let mut request = json!({ "method": method, "id": self.next_id });
        self.next_id += 1;
        if let Some(params) = params {
            request["params"] = params.clone();
        }
        let mut line = request.to_string();
        line.push('\n');

        let mut stream = self.connect(method)?;
        let err = |e| self.io_error(method, e);
        stream.set_read_timeout(Some(self.timeout)).map_err(err)?;
        stream.set_write_timeout(Some(self.timeout)).map_err(err)?;
        stream.write_all(line.as_bytes()).map_err(err)?;

        let mut data = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            let n = stream.read(&mut buf).map_err(err)?;
            if n == 0 {
                break;
            }
            data.extend_from_slice(&buf[..n]);
            if data.len() > MAX_RESPONSE_BYTES {
                return Err(BridgeError::Protocol {
                    method: method.to_string(),
                    kind: ProtocolError::TooLarge {
                        limit: MAX_RESPONSE_BYTES,
                    },
                });
            }
            if buf[..n].iter().any(|&b| b == b'\n' || b == b'\r') {
                break;
            }
        }
        Ok(data)
    }
}

/// Serves canned response lines per method; for offline tests and replay.
#[derive(Debug, Clone, Default)]
pub struct ReplayTransport {
    responses: HashMap<String, Vec<u8>>,
}

impl ReplayTransport {
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads every `<method>.json` in `dir` as the response to `<method>`.
    pub fn from_dir(dir: impl AsRef<Path>) -> std::io::Result<Self> {
        let mut replay = Self::new();
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().is_some_and(|e| e == "json") {
                if let Some(method) = path.file_stem().and_then(|s| s.to_str()) {
                    replay.insert(method, std::fs::read(&path)?);
                }
            }
        }
        Ok(replay)
    }

    pub fn insert(&mut self, method: &str, response: impl Into<Vec<u8>>) -> &mut Self {
        self.responses.insert(method.to_string(), response.into());
        self
    }

    pub fn get(&self, method: &str) -> Option<&[u8]> {
        self.responses.get(method).map(Vec::as_slice)
    }
}

impl Transport for ReplayTransport {
    fn send(&mut self, method: &str, _params: Option<&Value>) -> Result<Vec<u8>, BridgeError> {
        self.responses
            .get(method)
            .cloned()
            .ok_or_else(|| BridgeError::Io {
                method: method.to_string(),
                source: std::io::Error::new(
                    ErrorKind::NotFound,
                    format!("replay has no response for `{method}`"),
                ),
            })
    }
}

/// Decodes a raw response line into the method's result value.
///
/// Handles a UTF-8 BOM, a double-encoded (JSON string) body and the
/// `{"result": …, "id": …}` envelope; an `error` key at either level is a
/// [`BridgeError::Remote`].
pub(crate) fn decode(method: &str, raw: &[u8]) -> Result<Value, BridgeError> {
    let protocol = |kind| BridgeError::Protocol {
        method: method.to_string(),
        kind,
    };
    let text = std::str::from_utf8(raw).map_err(|_| protocol(ProtocolError::NonUtf8))?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text).trim();
    if text.is_empty() {
        return Err(protocol(ProtocolError::Empty));
    }
    let parse = |s: &str| {
        serde_json::from_str::<Value>(s)
            .map_err(|e| protocol(ProtocolError::InvalidJson(e.to_string())))
    };
    let mut value = parse(text)?;
    if let Value::String(inner) = &value {
        value = parse(inner)?;
    }

    for _ in 0..2 {
        check_remote_error(method, &value)?;
        match value {
            Value::Object(mut obj)
                if obj.contains_key("result")
                    && obj
                        .keys()
                        .all(|k| matches!(k.as_str(), "result" | "id" | "jsonrpc" | "error")) =>
            {
                value = obj.remove("result").unwrap_or(Value::Null);
            }
            other => {
                value = other;
                break;
            }
        }
    }
    check_remote_error(method, &value)?;
    Ok(value)
}

fn check_remote_error(method: &str, value: &Value) -> Result<(), BridgeError> {
    match value.get("error") {
        None | Some(Value::Null) => Ok(()),
        Some(err) => Err(BridgeError::Remote {
            method: method.to_string(),
            message: err.as_str().map_or_else(|| err.to_string(), str::to_string),
        }),
    }
}
