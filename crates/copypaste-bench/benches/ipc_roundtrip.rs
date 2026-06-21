//! Beta-bonus benchmark: IPC wire roundtrip — serialize Request → bytes →
//! deserialize Response.
//!
//! Measures the end-to-end serde cost of one daemon call as observed by the
//! UI / CLI client, across the three typical payload classes:
//!
//!   * **small**  — `Ping` (no params, minimal envelope)
//!   * **medium** — `HistoryList { limit: 100 }` with a 100-row mocked response
//!   * **large**  — `Import { items: [..1000..] }` (bulk JSON-import payload)
//!
//! This crate is read-only against `copypaste-ipc` — we go through the public
//! [`Request`] / [`Response`] types only, never poke at private serde
//! internals.
//!
//! Reports both Bytes throughput (Criterion auto-derives MB/s) and Elements
//! throughput (req/resp pairs per second) so the perf dashboard can answer
//! "how many IPC calls per second can a single CPU push?" without manual math.

// The bench builds the import payload directly as `serde_json::Value`, which
// is exactly the shape that goes over the wire today. A typed `ImportItem`
// was scoped for a later wave but is not currently planned; the untyped
// value is the production contract.
use copypaste_ipc::{Request, Response, PROTOCOL_VERSION};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use serde_json::json;

// ---------------------------------------------------------------------------
// Fixture builders — keep payload shape close to what the daemon actually
// produces. We intentionally avoid randomness so criterion's variance numbers
// reflect serde overhead, not RNG jitter.
// ---------------------------------------------------------------------------

fn build_ping_request() -> Request {
    Request {
        id: "1".to_string(),
        method: "ping".into(),
        params: serde_json::Value::Null,
        protocol_version: PROTOCOL_VERSION,
    }
}

fn build_ping_response() -> Response {
    Response::ok("1".to_string(), json!({"pong": true}))
}

fn build_history_list_request(limit: u64) -> Request {
    Request {
        id: "42".to_string(),
        method: "list".into(),
        params: json!({"limit": limit, "offset": 0}),
        protocol_version: PROTOCOL_VERSION,
    }
}

fn build_history_list_response(rows: usize) -> Response {
    // Mirrors the daemon's `list` response shape: array of row objects, each
    // with id / content_type / preview / wall_time / pinned.
    let items: Vec<serde_json::Value> = (0..rows)
        .map(|i| {
            json!({
                "id": format!("00000000-0000-0000-0000-{:012x}", i),
                "content_type": "text",
                "preview": format!("preview row #{i}"),
                "wall_time": 1_700_000_000_000_i64 + i as i64,
                "pinned": false,
            })
        })
        .collect();
    Response::ok("42".to_string(), json!({"items": items, "total": rows}))
}

fn build_import_request(n: usize) -> Request {
    // ~32 bytes of base64 ≈ 24 raw bytes per item — small enough to keep the
    // bench focused on envelope/array overhead rather than payload size alone.
    // Shape matches the wire format daemon expects for `method = "import"`.
    let items: Vec<serde_json::Value> = (0..n)
        .map(|i| {
            json!({
                "content_type": "text",
                "content_bytes_b64": "aGVsbG8td29ybGQtZnJvbS1iZW5jaA==",
                "created_at_ms": 1_700_000_000_000_i64 + i as i64,
                "metadata": {"source": "bench", "seq": i},
            })
        })
        .collect();
    Request {
        id: "99".to_string(),
        method: "import".into(),
        params: json!({"items": items, "dedup": true}),
        protocol_version: PROTOCOL_VERSION,
    }
}

fn build_import_response(inserted: usize, skipped: usize) -> Response {
    Response::ok(
        "99".to_string(),
        json!({"inserted": inserted, "skipped": skipped}),
    )
}

// ---------------------------------------------------------------------------
// Benches — three groups so flamegraphs / criterion output stay readable.
// ---------------------------------------------------------------------------

fn bench_serialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("ipc_serialize");
    for (label, req, resp) in [
        ("small_ping", build_ping_request(), build_ping_response()),
        (
            "medium_history_100",
            build_history_list_request(100),
            build_history_list_response(100),
        ),
        (
            "large_import_1000",
            build_import_request(1000),
            build_import_response(1000, 0),
        ),
    ] {
        // Pre-serialize once so we can report a byte-size for throughput.
        let req_bytes = serde_json::to_vec(&req).expect("pre-serialize req");
        let resp_bytes = serde_json::to_vec(&resp).expect("pre-serialize resp");
        let total = (req_bytes.len() + resp_bytes.len()) as u64;
        group.throughput(Throughput::Bytes(total));
        group.bench_with_input(
            BenchmarkId::from_parameter(label),
            &(req, resp),
            |b, (rq, rs)| {
                b.iter(|| {
                    let rb = serde_json::to_vec(black_box(rq)).expect("ser req");
                    let sb = serde_json::to_vec(black_box(rs)).expect("ser resp");
                    black_box((rb, sb));
                });
            },
        );
    }
    group.finish();
}

fn bench_deserialize(c: &mut Criterion) {
    // Note: `Response::error_code` is `Option<&'static str>` which cannot
    // borrow from a non-`'static` byte slice. We deserialize the response
    // bytes into `serde_json::Value` — the JSON parser does the same work
    // either way (tokenizer, alloc, structure build), only the final
    // struct-binding step differs. That is a faithful proxy for the cost
    // a real client pays.
    let mut group = c.benchmark_group("ipc_deserialize");
    for (label, req, resp) in [
        ("small_ping", build_ping_request(), build_ping_response()),
        (
            "medium_history_100",
            build_history_list_request(100),
            build_history_list_response(100),
        ),
        (
            "large_import_1000",
            build_import_request(1000),
            build_import_response(1000, 0),
        ),
    ] {
        let req_bytes = serde_json::to_vec(&req).expect("pre-serialize req");
        let resp_bytes = serde_json::to_vec(&resp).expect("pre-serialize resp");
        let total = (req_bytes.len() + resp_bytes.len()) as u64;
        group.throughput(Throughput::Bytes(total));
        group.bench_with_input(
            BenchmarkId::from_parameter(label),
            &(req_bytes, resp_bytes),
            |b, (rb, sb)| {
                b.iter(|| {
                    let rq: Request = serde_json::from_slice(black_box(rb)).expect("de req");
                    let rs: serde_json::Value =
                        serde_json::from_slice(black_box(sb)).expect("de resp");
                    black_box((rq, rs));
                });
            },
        );
    }
    group.finish();
}

fn bench_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("ipc_roundtrip");
    for (label, req, resp) in [
        ("small_ping", build_ping_request(), build_ping_response()),
        (
            "medium_history_100",
            build_history_list_request(100),
            build_history_list_response(100),
        ),
        (
            "large_import_1000",
            build_import_request(1000),
            build_import_response(1000, 0),
        ),
    ] {
        // One "element" = one complete client call (request + response).
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::from_parameter(label),
            &(req, resp),
            |b, (rq, rs)| {
                b.iter(|| {
                    let rb = serde_json::to_vec(black_box(rq)).expect("ser req");
                    let sb = serde_json::to_vec(black_box(rs)).expect("ser resp");
                    let rq2: Request = serde_json::from_slice(&rb).expect("de req");
                    // See bench_deserialize: Response cannot deserialize from
                    // non-`'static` bytes — deserialize into Value instead.
                    let rs2: serde_json::Value = serde_json::from_slice(&sb).expect("de resp");
                    black_box((rq2, rs2));
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_serialize, bench_deserialize, bench_roundtrip);
criterion_main!(benches);
