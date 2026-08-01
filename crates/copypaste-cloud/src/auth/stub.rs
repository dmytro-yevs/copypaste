//! Wiremock fixtures shared by the auth and REST tests.
//!
//! Requests are matched and counted by wiremock itself. Tests can also inspect
//! wiremock's captured [`Request`] values when an assertion is clearer than a
//! matcher (for example, checking every row in a chunked JSON body).

use std::sync::atomic::{AtomicUsize, Ordering};

use wiremock::matchers::any;
use wiremock::{Mock, MockBuilder, MockServer, Request, Respond, ResponseTemplate, Times};

/// One scripted response.
#[derive(Debug, Clone)]
pub(crate) struct Reply(ResponseTemplate);

impl Reply {
    pub fn json(status: u16, body: &str) -> Self {
        Self(ResponseTemplate::new(status).set_body_raw(body, "application/json"))
    }

    pub fn text(status: u16, body: &str) -> Self {
        Self(ResponseTemplate::new(status).set_body_raw(body, "text/html"))
    }

    pub fn empty(status: u16) -> Self {
        Self(ResponseTemplate::new(status))
    }

    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.0 = self.0.insert_header(name, value);
        self
    }
}

/// A responder that serves the script in order and then repeats its last
/// response. Repeating the last response lets persistent-failure retry tests
/// describe the failure once while wiremock still verifies the call count.
#[derive(Debug)]
struct SequentialResponder {
    replies: Vec<ResponseTemplate>,
    next: AtomicUsize,
}

impl SequentialResponder {
    fn new(replies: Vec<Reply>) -> Self {
        assert!(!replies.is_empty(), "a stub needs at least one reply");
        Self {
            replies: replies.into_iter().map(|reply| reply.0).collect(),
            next: AtomicUsize::new(0),
        }
    }
}

impl Respond for SequentialResponder {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        let index = self.next.fetch_add(1, Ordering::Relaxed);
        self.replies
            .get(index)
            .unwrap_or_else(|| self.replies.last().expect("non-empty response script"))
            .clone()
    }
}

pub(crate) struct Stub {
    server: MockServer,
    /// Base URL to hand to `CloudConfig::url`.
    pub base_url: String,
}

impl Stub {
    /// Serve every request with the script and verify `expected` calls.
    pub async fn start<T>(replies: Vec<Reply>, expected: T) -> Self
    where
        T: Into<Times>,
    {
        Self::start_matching(Mock::given(any()), replies, expected).await
    }

    /// Serve only requests accepted by `mock` and verify `expected` matches.
    pub async fn start_matching<T>(mock: MockBuilder, replies: Vec<Reply>, expected: T) -> Self
    where
        T: Into<Times>,
    {
        let server = MockServer::start().await;
        mock.respond_with(SequentialResponder::new(replies))
            .expect(expected)
            .mount(&server)
            .await;
        let base_url = server.uri();
        Self { server, base_url }
    }

    pub async fn requests(&self) -> Vec<Request> {
        self.server
            .received_requests()
            .await
            .expect("request recording is enabled")
    }

    pub async fn request_count(&self) -> usize {
        self.requests().await.len()
    }

    /// The single request the test expected to be made.
    pub async fn only_request(&self) -> Request {
        let requests = self.requests().await;
        assert_eq!(requests.len(), 1, "expected exactly one request");
        requests.into_iter().next().expect("one request")
    }
}

pub(crate) fn header<'a>(request: &'a Request, name: &str) -> Option<&'a str> {
    request.headers.get(name)?.to_str().ok()
}

pub(crate) fn json(request: &Request) -> serde_json::Value {
    request
        .body_json()
        .expect("request body should be valid JSON")
}
