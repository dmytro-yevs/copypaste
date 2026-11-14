//! Long-lived local connections into one loaded module instance. A session
//! keeps its instance loaded; closing it releases the queue and its lease.
use crate::{native::ModuleInstance, ModuleError};
use copypaste_module_sdk::{
    HostError, HostErrorCode, ModuleDelivery, MAX_ENDPOINT_CONNECTIONS, MAX_ENDPOINT_FRAME_BYTES,
};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
use tokio::sync::mpsc;

/// Bytes a connection may have queued for a client that is not reading;
/// further `endpoint_send` requests return `busy`.
pub(crate) const MAX_QUEUED_BYTES: usize = 2 * MAX_ENDPOINT_FRAME_BYTES;

/// The connections of one module instance.
#[derive(Default)]
pub(crate) struct Endpoints {
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    next: u64,
    closed: bool,
    connections: BTreeMap<String, Slot>,
}

struct Slot {
    outbound: mpsc::UnboundedSender<String>,
    queued: Arc<AtomicUsize>,
}

/// Frames the module sent to one client, in order.
pub(crate) struct Outbound {
    frames: mpsc::UnboundedReceiver<String>,
    queued: Arc<AtomicUsize>,
}

impl Outbound {
    async fn recv(&mut self) -> Option<String> {
        let frame = self.frames.recv().await?;
        self.queued.fetch_sub(frame.len(), Ordering::AcqRel);
        Some(frame)
    }
}

impl Endpoints {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn open(&self) -> Result<(String, Outbound), ModuleError> {
        let mut state = self.lock();
        if state.closed {
            return Err(ModuleError::Disabled);
        }
        if state.connections.len() >= MAX_ENDPOINT_CONNECTIONS {
            return Err(ModuleError::Invalid(
                "This module has too many open connections.".into(),
            ));
        }
        state.next = state.next.wrapping_add(1);
        let connection = format!("connection-{}", state.next);
        let (outbound, frames) = mpsc::unbounded_channel();
        let queued = Arc::new(AtomicUsize::new(0));
        state.connections.insert(
            connection.clone(),
            Slot {
                outbound,
                queued: Arc::clone(&queued),
            },
        );
        Ok((connection, Outbound { frames, queued }))
    }

    pub(crate) fn is_open(&self, connection: &str) -> bool {
        self.lock().connections.contains_key(connection)
    }

    /// Never waits on the module or the client.
    pub(crate) fn send(&self, connection: &str, data: String) -> Result<(), HostError> {
        let state = self.lock();
        let slot = state
            .connections
            .get(connection)
            .ok_or_else(|| HostError::new(HostErrorCode::NotFound, "The connection is closed."))?;
        let len = data.len();
        slot.queued
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |queued| {
                queued
                    .checked_add(len)
                    .filter(|total| *total <= MAX_QUEUED_BYTES)
            })
            .map_err(|_| {
                HostError::new(
                    HostErrorCode::Busy,
                    "The client is not reading; retry later.",
                )
            })?;
        slot.outbound.send(data).map_err(|_| {
            slot.queued.fetch_sub(len, Ordering::AcqRel);
            HostError::new(HostErrorCode::NotFound, "The connection is closed.")
        })
    }

    /// Ends the client side; the session then delivers `EndpointClose`.
    pub(crate) fn close(&self, connection: &str) -> bool {
        self.lock().connections.remove(connection).is_some()
    }

    /// Refuses new connections and ends every open one, returning their ids.
    pub(crate) fn close_all(&self) -> Vec<String> {
        let mut state = self.lock();
        state.closed = true;
        std::mem::take(&mut state.connections).into_keys().collect()
    }
}

/// One open connection, returned by `ModuleHost::connect`.
pub struct EndpointSession {
    link: EndpointLink,
    outbound: Outbound,
}

/// The module-facing half of a session; clone it to deliver from a blocking
/// worker while the session keeps forwarding outbound frames.
#[derive(Clone)]
pub struct EndpointLink {
    inner: Arc<Link>,
}

struct Link {
    connection: String,
    instance: Arc<ModuleInstance>,
    closed: AtomicBool,
}

impl EndpointSession {
    pub(crate) fn new(
        connection: String,
        instance: Arc<ModuleInstance>,
        outbound: Outbound,
    ) -> Self {
        Self {
            link: EndpointLink {
                inner: Arc::new(Link {
                    connection,
                    instance,
                    closed: AtomicBool::new(false),
                }),
            },
            outbound,
        }
    }

    pub fn connection(&self) -> &str {
        &self.link.inner.connection
    }

    pub fn link(&self) -> EndpointLink {
        self.link.clone()
    }

    /// The next frame for the client; `None` once the module or host closed
    /// the connection.
    pub async fn outbound(&mut self) -> Option<String> {
        self.outbound.recv().await
    }
}

impl EndpointLink {
    /// Delivers one client frame. Blocks while the module runs another call.
    pub fn deliver(&self, data: String) -> Result<(), ModuleError> {
        if data.len() > MAX_ENDPOINT_FRAME_BYTES {
            return Err(ModuleError::Invalid(
                "The message exceeds its size limit.".into(),
            ));
        }
        let link = &self.inner;
        link.instance.deliver(
            &link.connection,
            &ModuleDelivery::EndpointFrame {
                connection: link.connection.clone(),
                data,
            },
        )
    }

    /// Ends the connection and tells the module once. Blocks like `deliver`.
    pub fn close(&self) {
        let link = &self.inner;
        if link.closed.swap(true, Ordering::AcqRel) {
            return;
        }
        link.instance.endpoints().close(&link.connection);
        link.instance.deliver_close(&link.connection);
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        // A session dropped without `close` still frees its slot and queue.
        if !*self.closed.get_mut() {
            self.instance.endpoints().close(&self.connection);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn connections_are_bounded_and_queues_refuse_a_reader_that_stalls() {
        let endpoints = Endpoints::default();
        let mut open: Vec<_> = (0..MAX_ENDPOINT_CONNECTIONS)
            .map(|_| endpoints.open().unwrap())
            .collect();
        assert!(endpoints.open().is_err());
        let (connection, outbound) = &mut open[0];
        let frame = "x".repeat(MAX_ENDPOINT_FRAME_BYTES);
        endpoints.send(connection, frame.clone()).unwrap();
        endpoints.send(connection, frame.clone()).unwrap();
        assert_eq!(
            endpoints.send(connection, "y".into()).unwrap_err().code,
            HostErrorCode::Busy
        );
        assert_eq!(outbound.recv().await.unwrap().len(), frame.len());
        endpoints.send(connection, "y".into()).unwrap();
        assert!(endpoints.close(connection));
        assert_eq!(
            endpoints.send(connection, "z".into()).unwrap_err().code,
            HostErrorCode::NotFound
        );
        assert_eq!(outbound.recv().await.unwrap().len(), frame.len());
        assert_eq!(outbound.recv().await.as_deref(), Some("y"));
        assert_eq!(outbound.recv().await, None, "the client sees the close");
        assert_eq!(endpoints.close_all().len(), MAX_ENDPOINT_CONNECTIONS - 1);
        assert!(endpoints.open().is_err(), "a stopped instance takes none");
    }
}
