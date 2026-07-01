// ---------------------------------------------------------------------------
// src/lib/fixtures/ — DEV-only typed fixture factories (design.md Decision 7/G3,
// task 6.5).
//
// IMPORT-BOUNDARY RULE (round-5 M2 / task 6.5): production code MUST NOT
// import from `src/lib/fixtures/**`. The ONLY allowed consumers are:
//
//   - src/lib/mockIpc.ts        (dynamic-import-gated: only reached when
//                                 import.meta.env.DEV && (VITE_MOCK=1 || ?mock=1),
//                                 see lib/ipc/transport.ts)
//   - src/views/GalleryView/**  (dynamic-import-gated: only reached when
//                                 import.meta.env.DEV && the gallery URL branch
//                                 is active, see App.tsx)
//
// This module carries no gate of its own — it is DEV-only *only* because both
// of its consumers are already DEV-gated dynamic imports excluded from the
// production module graph. Do not import it from anything else.
//
// Enforced two ways:
//   (a) the production build's chunk-graph check (task 6.12) — confirms no
//       production-reachable chunk contains this module's path; and
//   (b) importBoundary.test.ts in this directory — greps the source tree for
//       any import of "lib/fixtures" outside the two allowed consumers above
//       and fails if one is found.
// ---------------------------------------------------------------------------

export { makeHistoryEntry } from "./historyEntry";
export { makeDevice, makeDiscoveredDevice, makeOwnDeviceInfo } from "./device";
export { makePairStatus } from "./pairing";
export { FIXTURE_OWN_DEVICE_ID, FIXTURE_OWN_FINGERPRINT } from "./ids";
export { FIXTURE_NOW, mins, hours, days } from "./time";
