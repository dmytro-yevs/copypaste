//! A scripted HTTP/1.1 server, so the tests exercise real `reqwest` requests
//! without touching the network.
//!
//! Shared with [`crate::rest`]'s tests: one stub for the crate, not one per
//! HTTP client. It lives under `auth` because `auth` is the module `rest`
//! already depends on, so sharing it here adds no edge that was not there.

use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// One request as the stub saw it on the wire.
#[derive(Debug, Clone)]
pub(crate) struct Captured {
    pub method: String,
    /// Path plus query, exactly as sent.
    pub target: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Captured {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    pub fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body).expect("request body should be JSON")
    }
}

/// One scripted response.
#[derive(Debug, Clone)]
pub(crate) struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Reply {
    pub fn json(status: u16, body: &str) -> Self {
        Self {
            status,
            headers: vec![("content-type".into(), "application/json".into())],
            body: body.to_string(),
        }
    }

    pub fn text(status: u16, body: &str) -> Self {
        Self {
            status,
            headers: vec![("content-type".into(), "text/html".into())],
            body: body.to_string(),
        }
    }

    pub fn empty(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: String::new(),
        }
    }

    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }
}

pub(crate) struct Stub {
    /// Base URL to hand to `CloudConfig::url`.
    pub base_url: String,
    captured: Arc<Mutex<Vec<Captured>>>,
}

impl Stub {
    /// Serve `replies` in order; once exhausted the last one repeats, so a
    /// test that only cares about the first response need not script the
    /// retries.
    pub async fn start(replies: Vec<Reply>) -> Self {
        assert!(!replies.is_empty(), "a stub needs at least one reply");
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind loopback");
        let addr = listener.local_addr().expect("local addr");
        let captured = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&captured);

        tokio::spawn(async move {
            let mut served = 0usize;
            while let Ok((mut socket, _)) = listener.accept().await {
                let request = match read_request(&mut socket).await {
                    Some(request) => request,
                    None => continue,
                };
                sink.lock().expect("stub mutex").push(request);

                let reply = replies
                    .get(served)
                    .cloned()
                    .unwrap_or_else(|| replies.last().expect("non-empty").clone());
                served += 1;

                let mut head = format!(
                    "HTTP/1.1 {} X\r\ncontent-length: {}\r\nconnection: close\r\n",
                    reply.status,
                    reply.body.len()
                );
                for (name, value) in &reply.headers {
                    head.push_str(&format!("{name}: {value}\r\n"));
                }
                head.push_str("\r\n");
                let _ = socket.write_all(head.as_bytes()).await;
                let _ = socket.write_all(reply.body.as_bytes()).await;
                let _ = socket.flush().await;
                let _ = socket.shutdown().await;
            }
        });

        Self {
            base_url: format!("http://{addr}"),
            captured,
        }
    }

    pub fn requests(&self) -> Vec<Captured> {
        self.captured.lock().expect("stub mutex").clone()
    }

    pub fn request_count(&self) -> usize {
        self.captured.lock().expect("stub mutex").len()
    }

    /// The single request the test expected to be made.
    pub fn only_request(&self) -> Captured {
        let requests = self.requests();
        assert_eq!(requests.len(), 1, "expected exactly one request");
        requests.into_iter().next().expect("one request")
    }
}

async fn read_request(socket: &mut tokio::net::TcpStream) -> Option<Captured> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];

    let header_end = loop {
        let read = socket.read(&mut chunk).await.ok()?;
        if read == 0 {
            return None;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(at) = find_double_crlf(&buffer) {
            break at;
        }
    };

    let head = String::from_utf8_lossy(&buffer[..header_end]).to_string();
    let mut lines = head.split("\r\n");
    let request_line = lines.next()?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?.to_string();

    let mut headers = Vec::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers.push((name.trim().to_lowercase(), value.trim().to_string()));
        }
    }

    let content_length: usize = headers
        .iter()
        .find(|(k, _)| k == "content-length")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(0);

    let mut body = buffer[header_end + 4..].to_vec();
    while body.len() < content_length {
        let read = socket.read(&mut chunk).await.ok()?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
    }

    Some(Captured {
        method,
        target,
        headers,
        body: String::from_utf8_lossy(&body).to_string(),
    })
}

fn find_double_crlf(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|w| w == b"\r\n\r\n")
}
