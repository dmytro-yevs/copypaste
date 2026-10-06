# 0015 — Pairing UI requires a bound SAS ceremony

**Status:** Accepted — 2026-08-01; protected macOS WebView revision — 2026-09-28; screenshot policy amendment — 2026-10-06

Pairing still creates a memory-only invite, joins an authenticated handshake, compares the common SAS derived from that handshake, records both peers' decisions before persistence, and supports idempotent cancellation. A locally generated six-digit display code remains prohibited. The app offers no pairing-credential clipboard action or command.

The ordinary app and Quick Paste WebViews receive only `PairingCeremony`: role, state, sanitized semantics and errors, presentation availability, and confirmed device metadata. They do not receive an invite, QR, code, address, SAS, peer address or key material. Android and Windows retain their protected native presenters. macOS uses a dedicated first-party `pairing` WebView whose window is created hidden and follows the device-wide Block screenshots setting; the current screenshot policy is applied before showing it. The secure window has no plugin permissions. Its Rust commands require the injected window label, the current session generation and ceremony identity; the ordinary app and Quick Paste windows cannot call them successfully.

The protected pairing surface loads local bundled content under the app CSP. A newly created invitation renders its QR immediately after the configured screenshot policy is applied. When the authenticated handshake reaches `awaiting_confirmation`, the surface automatically reveals the backend-bound SAS and offers **Accept** and **Reject**. Neither scanning nor displaying the SAS submits a decision. The QR contains one versioned `copypaste://pair/v1` URI with the short-lived pairing token and advertised LAN address. Android, macOS and Windows register that same custom URI and apply the configured screenshot policy before forwarding an external invocation into the Rust-backed pairing controller. Rust validates the URI before starting `PairJoin`; neither the native adapters nor Flutter mint a second payload format. The invite is rejected after its Rust monotonic deadline. SAS reveal and accept/reject require the backend's current `awaiting_confirmation` state, a positive remaining lifetime, the same ceremony and generation, and the exact SAS previously revealed in that protected surface. A decision is sent through the backend pairing protocol; the UI cannot mint a local SAS or persist a device itself. Once the user submits Accept or Reject, Rust holds a decision permit and denies OS close, Escape and the app close command until the backend returns; cancellation cannot undo a persisted Accept. The permit is released on success or error, so a failed submission can be retried or closed. The protected surface also owns secure manual join entry. Its secret nodes and references are removed on phase change, hide, cancel, expiration and teardown; Rust-held invite and SAS copies use zeroizing buffers. Closing the surface before a decision cancels an active ceremony. Expiry and cancellation are enforced again in Rust even if the Flutter timer is late.

This revision intentionally permits the automatically displayed QR, automatically revealed SAS and invite fields in the capture-protected Flutter surface and its accessibility tree. Their appearance there is part of the user-visible ceremony. No screenshots or accessibility dumps of that state may be collected as evidence. A custom URI scheme can be claimed by another installed application; the accepted mitigation is the 120-second CSPRNG invite plus mandatory handshake-bound SAS confirmation before persistence. Removing UI nodes cannot guarantee erasure of copies held by the Dart engine; this is the accepted residual risk of sharing the application UI. The pairing surface denies ordinary copy/clipboard commands, makes the QR non-draggable and disables text selection for revealed values. Manual join fields remain editable and pasteable while their copy/cut events are blocked. External navigation is denied. Release evidence checks capture protection before any AX dump or screenshot and never records protected content.

A pairing change is acceptable only if macOS, Android and Windows still provide the same versioned invite URI, immediate QR display after applying the configured screenshot policy, automatic display of the common bound SAS when confirmation is ready, explicit Accept/Reject before persistence, idempotent abort for every close/cancel path and finite expiry. When Block screenshots is enabled, Windows uses `WDA_EXCLUDEFROMCAPTURE` and Android uses `FLAG_SECURE` for every application window or activity. System file pickers and permission surfaces are separate OS integrations and remain.

Dependency disposition after this change: `qrcode` still renders macOS protected-window QR and Windows native QR; `objc2`/AppKit still serve shell, active-window and accessibility integrations; `dispatch2` remains in macOS accessibility evidence support. `tauri-plugin-dialog` remains for Rust-owned file pickers and Android quit-failure presentation. `swift-rs` remains transitive through `tauri-utils`/Tauri build tooling (`cargo tree -p copypaste-ui -i swift-rs --locked`); deleting AppKit pairing dialogs does not remove it. These dependencies are retained for current owners rather than removed by name.


## Screenshot policy amendment, 2026-10-06

Settings exposes one Security section with Block screenshots, off by default.
The device-local native policy persists the choice and applies it to the main
window, Quick Paste, pairing, QR and SAS presentation, and newly created
application windows or activities. Pairing lifecycle requests cannot override
this choice. Off permits capture everywhere in CopyPaste, including pairing.
The handshake, identity checks, finite expiry, and explicit SAS decision remain
independent of this user-controlled capture policy.

On macOS, protected Flutter IOSurfaces are presented through
`AVSampleBufferDisplayLayer.preventsCapture`. Core Image converts Flutter's
wide-gamut surfaces to the renderer's supported format on the GPU, with a reused
pixel-buffer pool. Off detaches the protected layers and restores the original
Flutter compositor. `NSWindow.sharingType` is not used as capture protection.
Native OS controls and system-owned permission/file-picker surfaces remain OS
integrations.

Capture verification uses synthetic content only. Actual pairing credentials,
QR tokens, SAS values, clipboard content, and accessibility trees must never be
recorded as evidence, regardless of the user's screenshot preference.
