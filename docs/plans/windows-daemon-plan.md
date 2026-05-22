# Windows Daemon Implementation Plan

Status: PLANNING
Branch: feature/windows-daemon
Author: worker-windows-daemon

---

## 1. Current State Analysis

The daemon crate already has a partial platform abstraction layer:

- `platform/mod.rs` — defines `ClipboardBackend` and `KeystoreBackend` traits
- `platform/macos.rs` — full implementation wrapping `clipboard.rs` + `keychain.rs`
- `platform/windows.rs` — stub with `unimplemented!()` bodies
- `platform/linux.rs` — stub with `unimplemented!()` bodies
- `clipboard.rs` — macOS-only polling via `NSPasteboard` (guarded by `#[cfg(target_os = "macos")]`)
- `ipc.rs` — Unix socket only (`tokio::net::UnixListener`) — does not compile on Windows
- `paths.rs` — hard-codes macOS `Library/Application Support` path
- `daemon.rs` — uses `tokio::signal::unix` under `cfg(target_os = "macos")` block

Key blocker: `ipc.rs` imports `UnixListener` and `UnixStream` unconditionally — this prevents `cargo check` on Windows entirely. The IPC transport must be abstracted behind a trait first (Phase 1).

---

## 2. New Module Layout (target state)

```
crates/copypaste-daemon/src/
├── clipboard/
│   ├── mod.rs          # ClipboardMonitor trait + ClipboardContent + ClipboardError
│   ├── macos.rs        # NSPasteboard polling (moved from clipboard.rs)
│   └── windows.rs      # WM_CLIPBOARDUPDATE hidden-window loop (stub → Phase 2)
├── ipc/
│   ├── transport.rs    # IpcTransport trait (accept/connect, object-safe)
│   ├── unix.rs         # UnixListener impl (moved from ipc.rs, gated cfg unix)
│   └── windows.rs      # NamedPipe impl (stub → Phase 3)
├── platform/
│   ├── mod.rs          # ClipboardBackend + KeystoreBackend traits (unchanged)
│   ├── macos.rs        # (unchanged)
│   ├── windows.rs      # Full impl in Phase 2-5
│   └── linux.rs        # (unchanged stub)
├── daemon.rs           # updated to use IpcTransport + platform-agnostic signal handling
├── paths.rs            # updated with Windows path (AppData\Roaming\CopyPaste)
├── keychain.rs         # unchanged (macOS only; Windows uses DPAPI)
├── main.rs
└── protocol.rs         # unchanged
```

---

## 3. Dependency Changes (Cargo.toml)

### Add to workspace Cargo.toml `[workspace.dependencies]`

```toml
# Windows-specific
winreg = "0.52"

# Cross-platform keyring (wraps DPAPI on Windows, Keychain on macOS, SecretService on Linux)
keyring = "2"
```

### Add to `crates/copypaste-daemon/Cargo.toml`

```toml
[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "0.52", features = [
    "Win32_Foundation",
    "Win32_System_DataObject",
    "Win32_System_Ole",
    "Win32_UI_WindowsAndMessaging",
    "Win32_System_Threading",
    "Win32_Security",
    "Win32_System_Registry",
] }
winreg = { workspace = true }
tokio = { workspace = true, features = ["net"] }   # includes named_pipe on Windows

[target.'cfg(not(target_os = "macos"))'.dependencies]
keyring = { workspace = true }
```

Existing macOS deps remain under `[target.'cfg(target_os = "macos")'.dependencies]`.

---

## 4. Phased Implementation

### Phase 1 — Abstract IpcTransport trait (unblocks Windows CI)

**Goal:** `cargo check --target x86_64-pc-windows-msvc` succeeds for the daemon crate.

Changes:
- Create `ipc/transport.rs` with `IpcTransport` trait
- Create `ipc/unix.rs` — move existing `UnixListener`/`UnixStream` code from `ipc.rs`, gate with `#[cfg(unix)]`
- Create `ipc/windows.rs` — stub returning `unimplemented!()`, gate with `#[cfg(windows)]`
- Update `ipc/mod.rs` to re-export the right impl based on platform
- Update `daemon.rs` to call `IpcTransport::serve()` via dynamic dispatch or conditional import
- Update `paths.rs` with Windows-aware path resolution

**IpcTransport trait shape:**

```rust
pub trait IpcTransport: Send + 'static {
    type Stream: AsyncRead + AsyncWrite + Unpin + Send + 'static;

    async fn accept(&mut self) -> anyhow::Result<Self::Stream>;
}
```

Because associated types with async make object-safety hard, use an enum or concrete wrapper instead of `dyn IpcTransport`:

```rust
pub enum IpcServer {
    #[cfg(unix)]
    Unix(unix::UnixIpcServer),
    #[cfg(windows)]
    Windows(windows::NamedPipeIpcServer),
}

impl IpcServer {
    pub async fn serve(self, path: &Path, db: Arc<Mutex<Database>>) -> anyhow::Result<()> {
        match self {
            #[cfg(unix)]
            IpcServer::Unix(s) => s.serve(path, db).await,
            #[cfg(windows)]
            IpcServer::Windows(s) => s.serve(path, db).await,
        }
    }
}
```

### Phase 2 — Windows Clipboard Monitor

**Goal:** Clipboard changes on Windows are detected and stored to SQLite.

Implementation details:

```
Thread model:
  Tokio runtime thread (async daemon loop)
      │
      └── OS thread: win32_clipboard_thread()
              │  CreateWindowExW (hidden HWND, WNDCLASS="CopyPasteClip")
              │  AddClipboardFormatListener(hwnd)
              └── GetMessage loop:
                    WM_CLIPBOARDUPDATE → read clipboard → send to mpsc::Sender<ClipboardContent>

Tokio side: mpsc::Receiver<ClipboardContent> polled via tokio::sync::mpsc
```

Key Win32 calls:
- `AddClipboardFormatListener(hwnd)` — register for `WM_CLIPBOARDUPDATE` (Vista+, preferred over `SetClipboardViewer`)
- `RemoveClipboardFormatListener(hwnd)` on shutdown
- `OpenClipboard(None)` / `GetClipboardData(CF_UNICODETEXT)` / `CloseClipboard()`
- `GlobalLock` / `GlobalUnlock` for the data handle

Clipboard formats to support:
- `CF_UNICODETEXT` (13) — UTF-16 text, convert to `String`
- `CF_DIB` / `CF_DIBV5` — image (store as `content_type = "image"`, Phase 4+)

Error handling: if `OpenClipboard` fails (another app holds it), retry after 10ms up to 5 times.

### Phase 3 — Named Pipe IPC

**Goal:** CLI can connect to daemon on Windows via `\\.\pipe\copypaste-daemon`.

Named pipe vs Unix socket: semantically identical for our use case (local IPC, newline-delimited JSON).

```rust
// Server side (tokio::net::windows::named_pipe)
use tokio::net::windows::named_pipe::ServerOptions;

let server = ServerOptions::new()
    .first_pipe_instance(true)
    .create(r"\\.\pipe\copypaste-daemon")?;

// Accept loop: server.connect().await? → creates one instance
// For multiple clients: create new ServerOptions instance after each connect
```

Security: restrict pipe access to current user via SDDL string `"D:(A;;GRGW;;;WD)"` initially; tighten to `"D:(A;;GRGW;;;CU)"` (current user) in production.

CLI-side connection (in `copypaste-cli`):
```rust
use tokio::net::windows::named_pipe::ClientOptions;
let client = ClientOptions::new().open(r"\\.\pipe\copypaste-daemon")?;
```

The existing `dispatch()` logic in `ipc.rs` is transport-agnostic (reads lines, writes JSON) and can be reused as-is.

### Phase 4 — Registry Autostart

**Goal:** Daemon starts automatically on Windows login.

```rust
use winreg::enums::*;
use winreg::RegKey;

fn set_autostart(exe_path: &str) -> anyhow::Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run_key = hkcu.open_subkey_with_flags(
        r"Software\Microsoft\Windows\CurrentVersion\Run",
        KEY_SET_VALUE,
    )?;
    run_key.set_value("CopyPaste", &exe_path)?;
    Ok(())
}

fn remove_autostart() -> anyhow::Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run_key = hkcu.open_subkey_with_flags(
        r"Software\Microsoft\Windows\CurrentVersion\Run",
        KEY_SET_VALUE,
    )?;
    run_key.delete_value("CopyPaste")?;
    Ok(())
}
```

Expose as `--install` / `--uninstall` CLI flags on the daemon binary.

Alternative (Task Scheduler): more robust for autostart with specific triggers (e.g. on session lock), but adds XML complexity. Lower priority — use Registry for Phase 4, Task Scheduler as Phase 6+.

### Phase 5 — Windows Credential Manager (DPAPI)

**Goal:** Encryption key persists across daemon restarts on Windows.

Use the `keyring` crate (cross-platform):

```rust
let entry = keyring::Entry::new("com.copypaste.daemon", "device-secret-key")?;

// Load existing
match entry.get_password() {
    Ok(hex) => { /* decode hex to [u8;32] */ }
    Err(keyring::Error::NoEntry) => {
        // Generate and store
        let kp = DeviceKeypair::generate();
        entry.set_password(&hex::encode(kp.secret_key_bytes()))?;
    }
}
```

`keyring` uses DPAPI (`CryptProtectData`) on Windows under the hood. No additional Win32 binding needed.

The `keyring` crate replaces direct `security-framework` calls for all platforms except macOS (where `security-framework` is more battle-tested). Gate the `keyring` dep under `cfg(not(target_os = "macos"))`.

---

## 5. Paths on Windows

Update `paths.rs`:

```rust
pub fn app_support_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        // %APPDATA%\CopyPaste  (e.g. C:\Users\<name>\AppData\Roaming\CopyPaste)
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| home::home_dir().unwrap().join("AppData").join("Roaming"))
            .join("CopyPaste")
    }
    #[cfg(target_os = "macos")]
    {
        home::home_dir()
            .expect("HOME must exist")
            .join("Library/Application Support/CopyPaste")
    }
    #[cfg(target_os = "linux")]
    {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home::home_dir().unwrap().join(".local/share"))
            .join("copypaste")
    }
}

pub fn socket_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    return PathBuf::from(r"\\.\pipe\copypaste-daemon");
    #[cfg(not(target_os = "windows"))]
    app_support_dir().join("daemon.sock")
}
```

---

## 6. Signal Handling on Windows

`daemon.rs` currently uses `tokio::signal::unix::signal(SignalKind::terminate())` gated by `cfg(target_os = "macos")`. The `cfg(not(target_os = "macos"))` block already handles Windows (only `ctrl_c`).

For Windows, `tokio::signal::ctrl_c()` is sufficient. `SIGTERM` does not exist on Windows; the equivalent is `SetConsoleCtrlHandler` for `CTRL_SHUTDOWN_EVENT`, but this is rarely needed for a user-space daemon.

The existing `#[cfg(not(target_os = "macos"))]` block in `daemon.rs` already covers Windows correctly.

---

## 7. Tray Icon

The `tray-icon` crate already works on Windows (uses `Shell_NotifyIconW` internally). No Windows-specific changes needed — the same API compiles and runs on both macOS and Windows.

This is handled separately in the Tauri/tray-icon task.

---

## 8. Testing Strategy

| Phase | Test type | Location |
|-------|-----------|----------|
| 1 (IpcTransport) | `cargo check` on Windows CI target | CI |
| 2 (Clipboard) | Integration test with mock `WM_CLIPBOARDUPDATE` dispatch | `tests/clipboard_win.rs` |
| 3 (Named pipe) | Adapt existing `ipc.rs` tests — swap `UnixStream` for `ClientOptions` | `ipc/tests.rs` |
| 4 (Autostart) | Unit test reading registry key after `set_autostart()` | `platform/windows.rs` tests |
| 5 (DPAPI) | `keyring` crate has its own tests; integration test: store + retrieve | `keychain/windows.rs` tests |

CI matrix addition needed (`ci.yml`):
```yaml
- target: x86_64-pc-windows-msvc
  os: windows-latest
```

---

## 9. Implementation Order (recommended)

```
Phase 1  ──────────────────────────────────────── unblocks CI
Phase 2 + Phase 3  ──────────────────────── can be parallel (different files)
Phase 4 + Phase 5  ──────────────────────── can be parallel (different files)
```

Estimated effort:
- Phase 1: 2-3 hours (mostly refactoring)
- Phase 2: 4-6 hours (Win32 message loop is subtle)
- Phase 3: 2-3 hours (tokio named pipe is well-documented)
- Phase 4: 1 hour (winreg API is simple)
- Phase 5: 1-2 hours (keyring crate abstracts complexity)

---

## 10. Known Risks

| Risk | Mitigation |
|------|------------|
| Win32 thread + Tokio runtime interaction | Use `std::sync::mpsc` to bridge Win32 thread → Tokio; never call Win32 from async context |
| Clipboard held by other process | Retry `OpenClipboard` up to 5× with 10ms back-off |
| Named pipe security | Default ACL restricts to local machine; explicitly set `DACL` to current user SID in Phase 3 |
| `GlobalLock` panic on null handle | Check `IsClipboardFormatAvailable(CF_UNICODETEXT)` before `GetClipboardData` |
| WASM/cross-compile | `windows` crate only compiles on MSVC toolchain; CI must use `x86_64-pc-windows-msvc` not GNU |
| `cargo check` on macOS for windows target | Use `cargo check --target x86_64-pc-windows-msvc` with Windows SDK — needs cross-compilation setup or Windows runner |
