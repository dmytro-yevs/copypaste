//! Bounded cross-device file-transfer probe for engineering validation.
//!
//! This is intentionally not a product server. Both modes require an explicit,
//! one-time pairing code supplied out of band, accept only private-LAN endpoints, generate their own
//! deterministic fixture, and keep at most one 64 KiB plaintext block resident.
//! The source exits after the configured deadline; the receiver removes its
//! temporary output after verifying the byte count and SHA-256.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use tokio::net::TcpListener;
use tokio::sync::watch;

use copypaste_p2p::node::listen;
use copypaste_p2p::peers::{Peer, PeerStore};
use copypaste_p2p::protocol::{ItemSummary, SyncItem, FILE_CHUNK_BYTES};
use copypaste_p2p::sync::{SyncError, SyncSource};
use copypaste_p2p::{Node, PairingToken};

const DEFAULT_MIB: u64 = 64;
const MAX_MIB: u64 = 128;
const DEFAULT_TIMEOUT_SECONDS: u64 = 180;
const MIN_TIMEOUT_SECONDS: u64 = 15;
const MAX_TIMEOUT_SECONDS: u64 = 600;
const ITEM_ID: &str = "file-transfer-probe-v1";

#[derive(Clone, Copy)]
enum Mode {
    Serve,
    Receive,
}

struct Options {
    mode: Mode,
    endpoint: SocketAddr,
    work_dir: PathBuf,
    bytes: usize,
    timeout: Duration,
    pairing_code: Option<String>,
}

struct FixtureSource {
    file: Mutex<File>,
    bytes: usize,
    hash: String,
}

impl FixtureSource {
    fn create(work_dir: &Path, bytes: usize) -> Result<(Self, PathBuf), String> {
        fs::create_dir_all(work_dir)
            .map_err(|error| format!("could not create probe directory: {error}"))?;
        let path = work_dir.join(format!(
            "file-transfer-probe-source-{}.bin",
            std::process::id()
        ));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| format!("could not create probe fixture: {error}"))?;
        let mut hasher = Sha256::new();
        for index in 0..bytes.div_ceil(FILE_CHUNK_BYTES) {
            let block = fixture_block(index as u64, expected_len(bytes, index as u64));
            file.write_all(&block)
                .map_err(|error| format!("could not write probe fixture: {error}"))?;
            hasher.update(&block);
        }
        file.flush()
            .map_err(|error| format!("could not flush probe fixture: {error}"))?;
        drop(file);
        let file = File::open(&path)
            .map_err(|error| format!("could not reopen probe fixture: {error}"))?;
        Ok((
            Self {
                file: Mutex::new(file),
                bytes,
                hash: hex::encode(hasher.finalize()),
            },
            path,
        ))
    }
}

impl SyncSource for FixtureSource {
    fn file_size(&self, id: &str, hash: &str) -> Result<u64, SyncError> {
        if id == ITEM_ID && hash == self.hash {
            Ok(self.bytes as u64)
        } else {
            Err(SyncError::Source("The file is unavailable.".into()))
        }
    }

    fn file_chunk(&self, id: &str, hash: &str, index: u64) -> Result<Vec<u8>, SyncError> {
        if id != ITEM_ID
            || hash != self.hash
            || index as usize >= self.bytes.div_ceil(FILE_CHUNK_BYTES)
        {
            return Err(SyncError::Source("The file is unavailable.".into()));
        }
        let length = expected_len(self.bytes, index);
        let offset = index
            .checked_mul(FILE_CHUNK_BYTES as u64)
            .ok_or_else(|| SyncError::Source("The file is unavailable.".into()))?;
        let mut block = vec![0; length];
        let mut file = self
            .file
            .lock()
            .map_err(|_| SyncError::Source("The probe fixture is unavailable.".into()))?;
        file.seek(SeekFrom::Start(offset))
            .and_then(|_| file.read_exact(&mut block))
            .map_err(|_| SyncError::Source("The probe fixture is unavailable.".into()))?;
        Ok(block)
    }

    fn device_id(&self) -> String {
        "file-transfer-probe-source".into()
    }
    fn device_name(&self) -> String {
        "File transfer probe source".into()
    }
    fn summaries(&self, _: i64) -> Result<Vec<ItemSummary>, SyncError> {
        Ok(Vec::new())
    }
    fn local_summaries(&self, _: &[String]) -> Result<Vec<ItemSummary>, SyncError> {
        Ok(Vec::new())
    }
    fn fetch(&self, _: &[String]) -> Result<Vec<SyncItem>, SyncError> {
        Ok(Vec::new())
    }
    fn apply(&self, _: SyncItem) -> Result<bool, SyncError> {
        Ok(false)
    }
}

#[tokio::main]
async fn main() {
    let options = match parse_options(std::env::args().skip(1).collect()) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("probe failed: {error}");
            std::process::exit(2);
        }
    };
    if let Err(error) = run(options).await {
        eprintln!("probe failed: {error}");
        std::process::exit(1);
    }
}

async fn run(options: Options) -> Result<(), String> {
    match options.mode {
        Mode::Serve => serve(options).await,
        Mode::Receive => receive(options).await,
    }
}

async fn serve(options: Options) -> Result<(), String> {
    let (source, fixture_path) = FixtureSource::create(&options.work_dir, options.bytes)?;
    let source = Arc::new(source);
    let pairing_code = options
        .pairing_code
        .as_deref()
        .ok_or_else(|| "--pairing-code is required for serve".to_string())?;
    let token =
        PairingToken::parse(pairing_code).map_err(|_| "invalid pairing code".to_string())?;
    let store_key = PairingToken::generate().psk();
    let store_dir = tempfile::tempdir()
        .map_err(|error| format!("could not create isolated peer store: {error}"))?;
    let store = PeerStore::open(&store_dir.path().join("peers"), &store_key)
        .map_err(|error| format!("could not open isolated peer store: {error}"))?;
    let node = Arc::new(Node::new(store, None, options.endpoint.port(), false));
    node.peers()
        .upsert(peer(&token, "probe receiver", None))
        .map_err(|error| format!("could not install isolated probe pairing: {error}"))?;
    let listener = TcpListener::bind(options.endpoint)
        .await
        .map_err(|error| format!("could not bind private-LAN probe listener: {error}"))?;
    let (shutdown, shutdown_rx) = watch::channel(false);
    let task = tokio::spawn(listen(
        node,
        listener,
        source,
        |_, _| {},
        || {},
        shutdown_rx,
    ));
    println!(
        "READY mode=serve bind={} bytes={} timeout_seconds={}",
        options.endpoint,
        options.bytes,
        options.timeout.as_secs(),
    );
    // The hash is deterministic and keeps the fixture line machine-readable.
    println!(
        "FIXTURE path={} sha256={}",
        fixture_path.display(),
        fixture_hash(options.bytes)
    );
    tokio::time::sleep(options.timeout).await;
    let _ = shutdown.send(true);
    task.await
        .map_err(|error| format!("probe listener task failed: {error}"))?;
    fs::remove_file(&fixture_path)
        .map_err(|error| format!("could not remove probe fixture: {error}"))?;
    println!("STOPPED mode=serve");
    Ok(())
}

async fn receive(options: Options) -> Result<(), String> {
    let expected_hash = fixture_hash(options.bytes);
    fs::create_dir_all(&options.work_dir)
        .map_err(|error| format!("could not create probe directory: {error}"))?;
    let output_path = options.work_dir.join(format!(
        "file-transfer-probe-receive-{}.bin",
        std::process::id()
    ));
    let output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output_path)
        .map_err(|error| format!("could not create receiver output: {error}"))?;
    let output = Arc::new(Mutex::new(output));
    let store_dir = tempfile::tempdir()
        .map_err(|error| format!("could not create isolated peer store: {error}"))?;
    let store_key = PairingToken::generate().psk();
    let store = PeerStore::open(&store_dir.path().join("peers"), &store_key)
        .map_err(|error| format!("could not open isolated peer store: {error}"))?;
    let pairing_code = options
        .pairing_code
        .as_deref()
        .ok_or_else(|| "--pairing-code is required for receive".to_string())?;
    let token =
        PairingToken::parse(pairing_code).map_err(|_| "invalid pairing code".to_string())?;
    let node = Node::new(store, None, 0, false);
    let remote = peer(&token, "probe source", Some(options.endpoint));
    node.peers()
        .upsert(remote.clone())
        .map_err(|error| format!("could not install isolated probe pairing: {error}"))?;
    let written = Arc::new(Mutex::new(0_u64));
    let written_for_receive = Arc::clone(&written);
    let hasher = Arc::new(Mutex::new(Sha256::new()));
    let hasher_for_receive = Arc::clone(&hasher);
    let output_for_receive = Arc::clone(&output);
    let started = Instant::now();
    let transfer = node.download_file(
        &remote,
        ITEM_ID,
        &expected_hash,
        options.bytes as u64,
        move |bytes| {
            let mut output = output_for_receive
                .lock()
                .map_err(|_| SyncError::Source("The probe output is unavailable.".into()))?;
            output
                .write_all(bytes)
                .map_err(|_| SyncError::Source("The probe output could not be written.".into()))?;
            hasher_for_receive
                .lock()
                .map_err(|_| SyncError::Source("The probe output is unavailable.".into()))?
                .update(bytes);
            *written_for_receive
                .lock()
                .map_err(|_| SyncError::Source("The probe output is unavailable.".into()))? +=
                bytes.len() as u64;
            Ok(())
        },
    );
    let result = tokio::time::timeout(options.timeout, transfer)
        .await
        .map_err(|_| "probe transfer timed out".to_string())?
        .map_err(|error| format!("probe transfer failed: {error}"))?;
    output
        .lock()
        .map_err(|_| "The probe output is unavailable.".to_string())?
        .flush()
        .map_err(|error| format!("could not flush probe output: {error}"))?;
    let elapsed = started.elapsed();
    let bytes = *written
        .lock()
        .map_err(|_| "The probe output is unavailable.".to_string())?;
    let actual_hash = hex::encode(
        hasher
            .lock()
            .map_err(|_| "The probe output is unavailable.".to_string())?
            .clone()
            .finalize(),
    );
    if bytes != options.bytes as u64 || result != expected_hash || actual_hash != expected_hash {
        return Err("probe integrity check failed".into());
    }
    let mb_per_second = bytes as f64 / elapsed.as_secs_f64() / 1_000_000.0;
    println!(
        "RESULT mode=receive bytes={} elapsed_seconds={:.3} payload_mb_per_second={:.3} sha256={}",
        bytes,
        elapsed.as_secs_f64(),
        mb_per_second,
        actual_hash,
    );
    drop(output);
    fs::remove_file(&output_path)
        .map_err(|error| format!("could not remove probe output: {error}"))?;
    Ok(())
}

fn peer(token: &PairingToken, name: &str, last_addr: Option<SocketAddr>) -> Peer {
    Peer {
        pairing_id: token.pairing_id(),
        device_id: None,
        name: name.into(),
        psk: token.psk(),
        last_addr,
        last_seen_ms: 0,
        profile: None,
        profile_observed_at_ms: 0,
    }
}

fn fixture_hash(bytes: usize) -> String {
    let mut hasher = Sha256::new();
    for index in 0..bytes.div_ceil(FILE_CHUNK_BYTES) {
        hasher.update(fixture_block(
            index as u64,
            expected_len(bytes, index as u64),
        ));
    }
    hex::encode(hasher.finalize())
}

fn fixture_block(index: u64, length: usize) -> Vec<u8> {
    let start = index as usize * FILE_CHUNK_BYTES;
    (0..length)
        .map(|offset| ((start + offset).wrapping_mul(31).wrapping_add(17) & 0xff) as u8)
        .collect()
}

fn expected_len(bytes: usize, index: u64) -> usize {
    bytes
        .saturating_sub(index as usize * FILE_CHUNK_BYTES)
        .min(FILE_CHUNK_BYTES)
}

fn parse_options(arguments: Vec<String>) -> Result<Options, String> {
    let Some((command, rest)) = arguments.split_first() else {
        return Err(usage());
    };
    let mode = match command.as_str() {
        "serve" => Mode::Serve,
        "receive" => Mode::Receive,
        _ => return Err(usage()),
    };
    let mut endpoint = None;
    let mut work_dir = None;
    let mut mib = DEFAULT_MIB;
    let mut timeout_seconds = DEFAULT_TIMEOUT_SECONDS;
    let mut pairing_code = None;
    let mut values = rest.iter();
    while let Some(flag) = values.next() {
        let value = values.next().ok_or_else(usage)?;
        match flag.as_str() {
            "--bind" if matches!(mode, Mode::Serve) => {
                endpoint = Some(parse_private_endpoint(value)?)
            }
            "--source" if matches!(mode, Mode::Receive) => {
                endpoint = Some(parse_private_endpoint(value)?)
            }
            "--work-dir" => work_dir = Some(PathBuf::from(value)),
            "--size-mib" => {
                mib = value
                    .parse()
                    .map_err(|_| "--size-mib must be an integer".to_string())?
            }
            "--timeout-seconds" => {
                timeout_seconds = value
                    .parse()
                    .map_err(|_| "--timeout-seconds must be an integer".to_string())?
            }
            "--pairing-code" => pairing_code = Some(value.to_string()),
            _ => return Err(usage()),
        }
    }
    if !(1..=MAX_MIB).contains(&mib)
        || !(MIN_TIMEOUT_SECONDS..=MAX_TIMEOUT_SECONDS).contains(&timeout_seconds)
    {
        return Err(usage());
    }
    Ok(Options {
        mode,
        endpoint: endpoint.ok_or_else(usage)?,
        work_dir: work_dir.ok_or_else(usage)?,
        bytes: usize::try_from(mib.saturating_mul(1024 * 1024)).map_err(|_| usage())?,
        timeout: Duration::from_secs(timeout_seconds),
        pairing_code,
    })
}

fn parse_private_endpoint(value: &str) -> Result<SocketAddr, String> {
    let endpoint: SocketAddr = value
        .parse()
        .map_err(|_| "endpoint must be an IP address and port".to_string())?;
    let private = match endpoint.ip() {
        IpAddr::V4(address) => address.is_private(),
        IpAddr::V6(address) => address.is_unique_local() || address.is_unicast_link_local(),
    };
    if endpoint.port() == 0 || !private {
        return Err("the probe accepts only a private-LAN address and nonzero port".into());
    }
    Ok(endpoint)
}

fn usage() -> String {
    "usage: file_transfer_probe serve --bind PRIVATE_IP:PORT --work-dir PATH --pairing-code CODE [--size-mib 1..128] [--timeout-seconds 15..600] | file_transfer_probe receive --source PRIVATE_IP:PORT --work-dir PATH --pairing-code CODE [--size-mib 1..128] [--timeout-seconds 15..600]".into()
}
