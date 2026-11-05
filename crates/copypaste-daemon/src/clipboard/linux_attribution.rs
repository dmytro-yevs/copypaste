//! X11 selection-owner source attribution.
//!
//! The selection owner is the writer.  Foreground windows and titles are never
//! consulted: they describe the reader's current desktop state and may contain
//! user content.  All process paths remain transient too.

use std::collections::VecDeque;
use std::os::unix::fs::MetadataExt;

use x11rb::connection::Connection;
use x11rb::protocol::res::{ClientIdMask, ClientIdSpec, ConnectionExt as _};
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _};

const MAX_CLIENT_WINDOWS: usize = 256;
const MAX_CLIENT_TREE_DEPTH: usize = 3;
const MAX_WM_CLASS_BYTES: u32 = 512;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SourceApp {
    pub(crate) id: String,
    pub(crate) name: String,
}

/// Resolve only evidence owned by the X11 selection writer. Absence is an
/// ordinary result: callers with exclusions then fail closed before TARGETS.
pub(crate) fn resolve_owner(
    connection: &x11rb::rust_connection::RustConnection,
    window: u32,
) -> Option<SourceApp> {
    source_for_window(connection, window, None)
        .or_else(|| resolve_same_client_window(connection, window))
}

/// Compare server-assigned client identities, never window titles or focus.
pub(super) fn same_client(
    connection: &x11rb::rust_connection::RustConnection,
    first: u32,
    second: u32,
) -> Option<bool> {
    if first == second {
        return Some(true);
    }
    Some(xres_client(connection, first)? == xres_client(connection, second)?)
}

/// GTK commonly owns CLIPBOARD through an internal, unmapped window with no
/// EWMH metadata. XRes identifies its X client; only a bounded listed or
/// reparented window from that same client and local process can supply the
/// desktop class.
fn resolve_same_client_window(
    connection: &x11rb::rust_connection::RustConnection,
    owner: u32,
) -> Option<SourceApp> {
    let owner_client = xres_client(connection, owner)?;
    same_uid_regular_executable(owner_client.pid)?;
    let root = connection.query_tree(owner).ok()?.reply().ok()?.root;
    let mut sources = client_window_sources(
        connection,
        owner,
        owner_client,
        client_windows(connection, root)?,
    );
    // A compositor can deliver focus before its EWMH client list reflects a
    // new GTK window. The server tree still exposes that writer's resources.
    if sources.is_empty() {
        sources = client_window_sources(
            connection,
            owner,
            owner_client,
            tree_windows(connection, root)?,
        );
    }
    (sources.len() == 1).then(|| sources.remove(0))
}

fn client_window_sources(
    connection: &x11rb::rust_connection::RustConnection,
    owner: u32,
    owner_client: XresClient,
    windows: Vec<u32>,
) -> Vec<SourceApp> {
    let mut sources = Vec::new();
    for window in windows {
        let source = (window != owner && xres_client(connection, window) == Some(owner_client))
            .then(|| source_for_window(connection, window, Some(owner_client.pid)))
            .flatten();
        if let Some(source) = source.filter(|source| !sources.contains(source)) {
            sources.push(source);
        }
    }
    sources
}

/// Prefer EWMH's client list because a window manager can reparent application
/// windows below its own frames. A bounded tree walk covers bare X11 servers
/// and window managers that do not publish EWMH metadata.
fn client_windows(
    connection: &x11rb::rust_connection::RustConnection,
    root: u32,
) -> Option<Vec<u32>> {
    let client_list = connection
        .intern_atom(true, b"_NET_CLIENT_LIST")
        .ok()
        .and_then(|cookie| cookie.reply().ok())
        .map(|reply| reply.atom)
        .filter(|atom| *atom != 0);
    if let Some(client_list) = client_list {
        if let Some(reply) = connection
            .get_property(
                false,
                root,
                client_list,
                AtomEnum::WINDOW,
                0,
                MAX_CLIENT_WINDOWS as u32,
            )
            .ok()
            .and_then(|cookie| cookie.reply().ok())
        {
            if let Some(windows) = reply
                .value32()
                .map(|values| values.take(MAX_CLIENT_WINDOWS).collect::<Vec<_>>())
            {
                if !windows.is_empty() {
                    return (reply.bytes_after == 0).then_some(windows);
                }
            }
        }
    }

    tree_windows(connection, root)
}

fn tree_windows(
    connection: &x11rb::rust_connection::RustConnection,
    root: u32,
) -> Option<Vec<u32>> {
    let mut windows = Vec::new();
    let mut parents = VecDeque::from([root]);
    for _ in 0..MAX_CLIENT_TREE_DEPTH {
        let mut next = VecDeque::new();
        while let Some(parent) = parents.pop_front() {
            let Ok(cookie) = connection.query_tree(parent) else {
                continue;
            };
            let Ok(reply) = cookie.reply() else {
                continue;
            };
            for window in reply.children {
                if windows.len() >= MAX_CLIENT_WINDOWS {
                    return None;
                }
                windows.push(window);
                next.push_back(window);
            }
        }
        parents = next;
    }
    Some(windows)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct XresClient {
    xid: u32,
    pid: u32,
}

/// XRes 1.2 reports a server-assigned client XID and the PID of a local client.
/// Servers without that optional extension, remote clients, and malformed
/// replies are intentionally unattributed.
fn xres_client(
    connection: &x11rb::rust_connection::RustConnection,
    window: u32,
) -> Option<XresClient> {
    let reply = connection
        .res_query_client_ids(&[
            ClientIdSpec {
                client: window,
                mask: ClientIdMask::CLIENT_XID,
            },
            ClientIdSpec {
                client: window,
                mask: ClientIdMask::LOCAL_CLIENT_PID,
            },
        ])
        .ok()?
        .reply()
        .ok()?;
    let mut xid = None;
    let mut pid = None;
    for identity in reply.ids {
        if identity.spec.mask == ClientIdMask::CLIENT_XID {
            xid = Some(identity.spec.client);
        } else if identity.spec.mask == ClientIdMask::LOCAL_CLIENT_PID {
            pid = identity
                .value
                .first()
                .copied()
                .filter(|_| identity.value.len() == 1);
        }
    }
    Some(XresClient {
        xid: xid?,
        pid: pid?,
    })
}

fn source_for_window(
    connection: &x11rb::rust_connection::RustConnection,
    window: u32,
    expected_pid: Option<u32>,
) -> Option<SourceApp> {
    let pid_atom = connection
        .intern_atom(false, b"_NET_WM_PID")
        .ok()?
        .reply()
        .ok()?
        .atom;
    let pid = connection
        .get_property(false, window, pid_atom, AtomEnum::CARDINAL, 0, 1)
        .ok()?
        .reply()
        .ok()?
        .value32()?
        .next()?;
    (expected_pid.is_none_or(|expected| expected == pid)).then_some(())?;
    same_uid_regular_executable(pid)?;
    let class = connection
        .get_property(
            false,
            window,
            AtomEnum::WM_CLASS,
            AtomEnum::STRING,
            0,
            MAX_WM_CLASS_BYTES / 4,
        )
        .ok()?
        .reply()
        .ok()?
        .value;
    let class = wm_class(&class)?;
    let (id, name) = desktop_for_class(&class)?;
    Some(SourceApp { id, name })
}

/// Resolve a compositor-provided desktop application ID to its installed
/// desktop entry. The ID is evidence from the compositor; this only supplies
/// its display label and never derives an identity from desktop state.
pub(crate) fn resolve_desktop_id(id: &str) -> Option<SourceApp> {
    valid_desktop_id(id).then_some(())?;
    let (_, name) = desktop_for_class(id)?;
    Some(SourceApp {
        id: id.to_owned(),
        name,
    })
}

/// ICCCM WM_CLASS is `instance NUL class NUL`. The class (not the instance)
/// is the desktop-file convention and is usable only as a bounded package ID.
fn wm_class(bytes: &[u8]) -> Option<String> {
    let mut parts = bytes.split(|byte| *byte == 0);
    let _instance = parts.next()?;
    let class = std::str::from_utf8(parts.next()?).ok()?.trim();
    valid_desktop_id(class).then(|| class.to_owned())
}

fn same_uid_regular_executable(pid: u32) -> Option<()> {
    let process = std::fs::metadata(format!("/proc/{pid}")).ok()?;
    let current = std::fs::metadata("/proc/self").ok()?;
    let executable = std::fs::metadata(format!("/proc/{pid}/exe")).ok()?;
    (process.uid() == current.uid() && executable.is_file()).then_some(())
}

fn valid_desktop_id(value: &str) -> bool {
    (1..=255).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn desktop_for_class(class: &str) -> Option<(String, String)> {
    copypaste_source_app::linux_desktop_application(class).map(|(id, name)| (id, name.into_inner()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wm_class_rejects_titles_paths_and_malformed_values() {
        assert_eq!(
            wm_class(b"editor\0org.example.Editor\0"),
            Some("org.example.Editor".into())
        );
        assert_eq!(wm_class(b"editor\0/opt/editor\0"), None);
        assert_eq!(wm_class(b"editor\0firefox\0"), Some("firefox".into()));
        assert_eq!(wm_class(b"editor"), None);
    }
}
