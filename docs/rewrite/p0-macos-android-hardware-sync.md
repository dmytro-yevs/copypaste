# P0 macOS ↔ Android hardware sync validation

This is the release gate for peer sync that cannot be established by Rust or
browser tests. It covers a physical Mac and a physical Android phone on the
same v2 build. Do not record a pairing code, QR image, clipboard content, local
paths, or device identifiers in the evidence.

## Exit criteria

Pass the two pairing routes, both transfer directions, the remembered-address
case, macOS Firewall, multicast-free networking, VPN, and Android lifecycle.
Expected network denials must preserve the local item and leave no half-pairing.
Any other failure is P0 until classified below.

## Setup

1. Use disposable v2 profiles on a physical Mac and phone; neither has a prior
   pairing. Install artifacts built from the same commit and record their
   version/build hashes.
2. Put both devices on the same ordinary Wi-Fi, with peer-to-peer client access.
   Keep the Mac firewall enabled and permit incoming connections for CopyPaste.
   Verify that both Devices screens load and that sync is enabled.
3. Give each test a unique ordinary-text marker, for example
   `p0-qr-mac-to-phone-YYYYMMDD-HHMM`. Confirm that it is not classified as
   sensitive. Use **Sync now** for every assertion; automatic cadence is not
   timing evidence.
4. Before the Android lifecycle test, enable its foreground capture service and
   confirm the ongoing capture notification. This is what is intended to keep
   the process containing the embedded peer listener alive.

## Test matrix

| ID | Action | Expected result | Evidence |
| --- | --- | --- | --- |
| P0-1 | On Mac, choose **Pair a new device**. On Android, choose **Add device** then **Scan a code** and scan the Mac QR. | Android's system scanner returns to CopyPaste without a `CAMERA` permission prompt. Pairing completes, both Devices screens show the peer, a non-empty connection address, and a successful initial sync. | Screenshots of both peer rows with code/content redacted; Android permission screen, if any; timestamps and build hashes. |
| P0-2 | With the P0-1 pairing, create a Mac marker, run Android **Sync now**, then create an Android marker and run Mac **Sync now**. | Each target receives exactly the corresponding marker; each initiating row reports a successful run with non-zero receive/send as applicable. No duplicate appears after repeating either sync. | Before/after history screenshots and both run-result screenshots. |
| P0-3 | Start a fresh pairing from Android. On Mac select **Add device**, type the Android code and `host:port` manually, and submit. Do not select a discovered device. | Pairing completes; the Mac records Android's address returned in authenticated Hello. | Redacted form/peer-row screenshots. Never capture the entered code. |
| P0-4 | With the P0-3 pairing, create a Mac marker and run Android **Sync now**; then create an Android marker and run Mac **Sync now**. | Both directions succeed. In particular Android, the creator that accepted the inbound connection, can dial Mac later without mDNS; this proves the reverse address was stored. | History and run-result screenshots; peer rows showing an address on each side. |
| P0-5 | Keep macOS Firewall on and repeat an Android-initiated sync to Mac. Then, on a disposable test Mac, temporarily block CopyPaste incoming connections (or enable block-all incoming), retry, restore the allow rule, and retry again. | Allow rule: inbound sync succeeds. Blocked state: Android reports a friendly retryable failure, no remote item is removed, and no new pairing is retained. Restoring the rule lets the same stored pairing sync successfully. | Firewall state screenshots, Android failure/success states, and marker history. |
| P0-6 | Use a network profile that suppresses multicast but still permits unicast between clients. Pair once while the devices can reach each other, verify they no longer discover each other, then run **Sync now** in each direction. | Discovery may say offline/empty; direct sync succeeds through the saved address. A false offline indicator is not a sync failure. | Network profile name, Devices state, peer addresses, and two successful runs. |
| P0-7 | Use a guest SSID or router profile with client isolation that blocks device-to-device TCP. Attempt a new manual pairing using the phone's LAN address. | Discovery can be absent. Pairing fails without persisting a peer and without moving history; the failure is a network-topology result, not an authentication success. | Router/SSID setting, friendly failure, and empty/unchanged peer lists. |
| P0-8 | Connect both devices to the same peer-to-peer VPN. With ordinary Wi-Fi disabled or client isolation still active, make a fresh manual pairing using the VPN `host:port`, then sync a marker each way. | Both transfers succeed over the VPN. The saved address used after the first session is reachable on the VPN; a LAN address that breaks the follow-up sync is a defect in address selection/persistence. | Redacted VPN address family/prefix, peer rows, and both run results. |
| P0-9 | With a proven pairing and Android foreground capture notification visible, press Home on Android for two minutes. On Mac add a marker and run **Sync now** to Android. Then force-stop Android, retry from Mac, relaunch Android, and retry. | Backgrounded Android still receives while its foreground service is active. Force-stop causes a friendly failure and no data loss. After relaunch the pairing and prior history remain, and retry converges. | Android notification, timing notes, Mac failure/success states, and Android history after relaunch. |

## Evidence record

For every matrix row, record: ID, date/time and timezone, Mac model/macOS,
phone model/Android version, app build hashes, network profile, initiator,
result, and links to redacted screenshots. Include a short daemon/app log
extract only after checking it contains no code, marker payload, path, or full
address. A failed row must include the exact user-visible error and whether
the peer list, saved address, and both marker counts changed.

## Failure classification

| Classification | Condition | Required next action |
| --- | --- | --- |
| Product defect | A required success row fails, a duplicate/loss occurs, a successful pair has no usable reverse address, or a blocked route retains a half-pairing. | File P0 with row ID, redacted evidence, builds, and reproducible network conditions. Do not claim sync validation. |
| Network topology | P0-7 fails exactly as expected, or a managed network blocks the required TCP/multicast path and VPN has not yet been tried. | Record policy and route. Run P0-8 before treating it as an environment-only block. |
| Build/install block | No matching APK or Mac app can launch, or Android native compilation is blocked by the missing Tauri Gradle settings script/JDK mismatch. | Record toolchain versions and first failing command; this blocks all hardware claims, not P2P correctness. |
| Device/OS policy | Android kills the process despite the visible foreground service, VPN policy forbids peer TCP, or macOS has a managed firewall rule. | Repeat once on a second supported device/network. If reproducible, file P0 with policy details; if not, retain as environment evidence. |
| Test setup | Different build hashes, old pairing/history, sensitive marker, missing foreground-service notification, or an unrecorded network change. | Invalidate the row and rerun from clean disposable profiles. |

The protocol reason for P0-4, P0-6, and P0-8 is deliberate: each successful
Noise session carries the peer's listening address in authenticated Hello and
persists it for future dialing. mDNS is only discovery; it must not be required
after a successful pair.
