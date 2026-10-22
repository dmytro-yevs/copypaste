# Personal device group

CopyPaste has one personal trust circle. Every active member may admit and
remove devices. There is no owner, administrator, cloud relay, or mandatory
always-on member. The same Rust membership implementation runs in the desktop
daemon and Android runtime, and the shared Flutter Devices screen lists its
transport peers on macOS, Windows and Android.

## Admission and direct connections

A confirmed SAS ceremony authorizes admission of the other device and its
existing group. Existing encrypted pairings are admission authorities during
migration: their first protocol-10 session exchanges signed rosters and learns
missing stable identities. Existing working pairing keys are preserved while
their certified admission generations remain current. SAS protocol 3 records
both admission generations only after bilateral storage acknowledgement; an
unconfirmed pairing cannot authorize membership through a second connection. An
authenticated Supabase account together with the correct sync passphrase is
also an admission authority, matching the account's existing trust boundary.
Account passwords, sessions and sync keys are never transferred by pairing.

Each device creates persistent independent Ed25519 and X25519 secrets inside
the existing encrypted peer file. Only public keys enter the signed roster.
For a newly learned member, the endpoints derive their Noise PSK with X25519
and HKDF-SHA256, binding the sorted device identities, exchange public keys and
admission generations. The deterministic pairing identifier is advertised by
the existing mDNS service. A third device never receives the endpoints' PSK. A stable public-key fingerprint
in discovery is an endpoint hint, not an authentication credential.

For Mac--Android followed by Android--Windows, membership converges through
the available authenticated links. Mac and Windows can then synchronize
directly even after Android stops, provided they can reach each other locally.
Names and endpoint hints follow descriptor revisions. Relayed profiles do not
establish direct authenticated observations, online status or latency. Existing
authenticated probes and completed sessions remain the observation owners.

## Removal and offline reconciliation

Removal records the highest excluded admission generation. These tombstones
are merged by maximum and are never pruned to make room. A stale roster cannot
restore the same admission. A removed issuer's retained signed snapshot cannot
author more changes. A received roster that marks its own issuer inactive is
refused before merging any removal or admission. Fresh SAS confirmation permits
deliberate readmission at
a higher generation, and derived content keys rotate with the generation.
Credential certificates prevent an older SAS key from becoming current again.
A device certifies at most the next generation of its own identity during SAS;
peer proposals are checked before credential persistence. Relayed rosters cannot
advance that local identity or remove a generation the device never certified.
The maximum integer generation is refused before it can make readmission overflow.
Legacy peers without a known group identity are retained during initial migration
but cannot survive a later local readmission without enrollment.

Unpair removes the member from the group. Revoke additionally retains the
existing permanent ban on that particular pairing credential. A successful
local action persists membership and transport changes atomically. Receiving
a removal drops affected transport keys, cancels and drains active history
operations, forgets cursors and invalidates reachability. An excluded local
device clears its peers and stops ordinary cloud history synchronization.
The module host revokes running sync providers on an exclusion wake, including
a provider command queued behind an active network round. Account workers and
Realtime tasks are stopped while encrypted credentials remain stored.

Changes reach offline members when an authenticated path becomes available.
When content keys have rotated while a member was offline, a separate
X25519/HKDF membership credential can reconcile the signed roster. Its explicit
transport prefix accepts only membership frames, never probes, files or history.
Only active members are candidates; removal invalidates this path too.
Group reconciliation obeys the shared synchronization switch. Disabled sync
does not run membership traffic. Membership changes require an issuer still
authorized by the receiver; mutually disconnected trust circles can be merged
again through fresh SAS confirmation.
Disconnected devices cannot learn a removal instantly. Membership removal does
not revoke a Supabase account session or change its passphrase. A malicious
holder of both account credentials and the sync key remains inside the cloud
account's pre-existing trust boundary; rotate/revoke those credentials for a
compromised device. Group signatures do not replace cloud account security.

## Supabase

`device_group_states` stores one encrypted signed snapshot per writer under
account RLS, independently of clipboard pagination and retention. Deploy
`20261022120000_device_group_states.sql` and rebuild the Supabase module with
the updated application. An older module synchronizes history only; it does
not implement cloud membership. Missing schema is surfaced as a sync error.

Rows use the existing passphrase-derived key and authenticated row encryption,
with a group-specific associated-data identifier. Decrypted issuer identity
must match the row's writer identity, and its Ed25519 signature must verify.
Unknown-account, wrong-key, malformed and removed-writer snapshots are refused.
Invalid rows are isolated; storage failures must remain actionable.

The driver reads a small paginated writer/ciphertext-digest catalog and
downloads only changed encrypted snapshots before uploading clipboard history,
then publishes changed local membership. The bounded digest cache is an optimization, never
membership or revocation evidence, and is discarded with the account worker.
The driver runs inside the existing single-flight
sync worker and adaptive cadence. History and membership Realtime filters share
one socket; both filtered registrations and both PostgreSQL readiness signals
must be confirmed before the long idle polling cadence is used. Reconnect
preserves both registrations. Membership events are hints to fetch and verify,
never direct instructions to change local trust.

## Bounds and verification

The existing maximum is 16 remote peers (17 group members including self).
There are at most 4096 removal identities and 4096 cloud writers per account.
A signed snapshot is capped at 768 KiB, payload pages at eight rows, catalog
pages at 64 entries and a complete drain at 4096 writers. Admission beyond a
bound fails without eviction.
Unchanged group exports avoid cloning the peer store; unchanged imports avoid
key derivation, file writes and notifications. Unchanged cloud snapshots do not
produce upserts while the own writer digest still matches. A missing or changed
own cloud row is republished from local signed state even when the roster has
not changed. The database recomputes SHA-256 over the nonce and ciphertext on
every write; account clients cannot write the digest column. A ciphertext change
with the same nonce therefore invalidates the cache too. This repairs one-off
account-only tampering without a restart; account write access still permits
ongoing denial of service. A lazy zeroizing cache holds at most 16 membership
credentials and is cleared immediately on membership changes. No new polling
timer, worker, process or socket is introduced.

Required functional evidence covers migration of the three-device chain,
direct transfer after the intermediary stops, persistence, offline removal,
signature tampering, explicit readmission, encrypted cloud-only convergence,
unchanged-upload suppression, SQL account isolation and dual-table Realtime
readiness. Native UI and resource evidence must additionally use the same
candidate on macOS, physical Android and installed Windows.

RUN-062 tracks the resource impact. Native application/daemon/worker peak,
steady-state and growth evidence is not established. CPU, wakeup, I/O,
network and growth thresholds in the runtime budget file remain unresolved;
functional tests do not qualify those budgets.
