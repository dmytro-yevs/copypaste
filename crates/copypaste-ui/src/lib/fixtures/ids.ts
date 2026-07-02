// ---------------------------------------------------------------------------
// src/lib/fixtures/ids.ts — shared fixture identity constants.
//
// DEV-only. See index.ts for the import-boundary rule.
// ---------------------------------------------------------------------------

/**
 * Canonical "own device" UUID shared by every fixture that needs to express
 * "this device" (HistoryEntry.origin_device_id, HistoryPage.own_device_id,
 * OwnDeviceInfo, get_own_fingerprint's paired fingerprint below). Keeping this
 * in one place means mockIpc.ts and the gallery can never disagree about which
 * id means "local device".
 */
export const FIXTURE_OWN_DEVICE_ID = "aabbccdd-1234-5678-abcd-ef0011223344";

/** Canonical own-device mTLS certificate fingerprint (pairs with the id above). */
export const FIXTURE_OWN_FINGERPRINT =
  "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2";
