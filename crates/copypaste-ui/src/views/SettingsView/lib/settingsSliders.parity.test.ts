/**
 * #12 — TS size consts parity: TypeScript guard.
 *
 * Source of truth for the TS values:
 *   crates/copypaste-ui/src/views/SettingsView/lib/settingsSliders.ts
 *
 * Source of truth for the Rust values:
 *   crates/copypaste-core/src/config/defaults.rs
 *
 * The Rust defaults are (binary MiB/GiB):
 *   MAX_TEXT_SIZE_BYTES      = 10 * 1024 * 1024          (10 MiB)
 *   MAX_IMAGE_SIZE_BYTES     = 64 * 1024 * 1024          (64 MiB)
 *   MAX_FILE_SIZE_BYTES      = 100 * 1024 * 1024         (100 MiB)
 *   STORAGE_QUOTA_BYTES      = 10 * 1024 * 1024 * 1024   (10 GiB)
 *
 * Since copypaste-ui cannot import Rust, the expected values are encoded as
 * numeric literals here with a comment citing defaults.rs as the source of truth.
 * A matching Rust test in crates/copypaste-core/tests/ts_size_consts_parity.rs
 * asserts the same literals against the real Rust constants, so a Rust-side change
 * immediately breaks a Rust test that names THIS file as the TS side to update.
 *
 * If a Rust default changes:
 *   1. Update defaults.rs (the source of truth).
 *   2. Update DEFAULT_* in settingsSliders.ts.
 *   3. Update the expected_* literals in THIS file.
 *   4. Update the TS_DEFAULT_* literals in ts_size_consts_parity.rs.
 */
import {
  DEFAULT_MAX_TEXT_BYTES,
  DEFAULT_MAX_IMAGE_BYTES,
  DEFAULT_MAX_FILE_BYTES,
  DEFAULT_STORAGE_QUOTA_BYTES,
} from "./settingsSliders";

// Expected values — literals from crates/copypaste-core/src/config/defaults.rs
// (the single source of truth). Update here when defaults.rs changes.
const EXPECTED_MAX_TEXT_BYTES = 10 * 1024 * 1024;           // defaults.rs: MAX_TEXT_SIZE_BYTES
const EXPECTED_MAX_IMAGE_BYTES = 64 * 1024 * 1024;          // defaults.rs: MAX_IMAGE_SIZE_BYTES
const EXPECTED_MAX_FILE_BYTES = 100 * 1024 * 1024;          // defaults.rs: MAX_FILE_SIZE_BYTES
const EXPECTED_STORAGE_QUOTA_BYTES = 10 * 1024 * 1024 * 1024; // defaults.rs: STORAGE_QUOTA_BYTES

describe("settingsSliders size consts parity with copypaste-core defaults", () => {
  it("DEFAULT_MAX_TEXT_BYTES matches Rust MAX_TEXT_SIZE_BYTES (10 MiB)", () => {
    // Source of truth: crates/copypaste-core/src/config/defaults.rs:9 (MAX_TEXT_SIZE_BYTES)
    // If this fails, update DEFAULT_MAX_TEXT_BYTES in settingsSliders.ts
    // AND TS_DEFAULT_MAX_TEXT_BYTES in crates/copypaste-core/tests/ts_size_consts_parity.rs
    expect(DEFAULT_MAX_TEXT_BYTES).toBe(EXPECTED_MAX_TEXT_BYTES);
  });

  it("DEFAULT_MAX_IMAGE_BYTES matches Rust MAX_IMAGE_SIZE_BYTES (64 MiB)", () => {
    // Source of truth: crates/copypaste-core/src/config/defaults.rs:11 (MAX_IMAGE_SIZE_BYTES)
    // If this fails, update DEFAULT_MAX_IMAGE_BYTES in settingsSliders.ts
    // AND TS_DEFAULT_MAX_IMAGE_BYTES in crates/copypaste-core/tests/ts_size_consts_parity.rs
    expect(DEFAULT_MAX_IMAGE_BYTES).toBe(EXPECTED_MAX_IMAGE_BYTES);
  });

  it("DEFAULT_MAX_FILE_BYTES matches Rust MAX_FILE_SIZE_BYTES (100 MiB)", () => {
    // Source of truth: crates/copypaste-core/src/config/defaults.rs:29 (MAX_FILE_SIZE_BYTES)
    // If this fails, update DEFAULT_MAX_FILE_BYTES in settingsSliders.ts
    // AND TS_DEFAULT_MAX_FILE_BYTES in crates/copypaste-core/tests/ts_size_consts_parity.rs
    expect(DEFAULT_MAX_FILE_BYTES).toBe(EXPECTED_MAX_FILE_BYTES);
  });

  it("DEFAULT_STORAGE_QUOTA_BYTES matches Rust STORAGE_QUOTA_BYTES (10 GiB)", () => {
    // Source of truth: crates/copypaste-core/src/config/defaults.rs:31 (STORAGE_QUOTA_BYTES)
    // If this fails, update DEFAULT_STORAGE_QUOTA_BYTES in settingsSliders.ts
    // AND TS_DEFAULT_STORAGE_QUOTA_BYTES in crates/copypaste-core/tests/ts_size_consts_parity.rs
    expect(DEFAULT_STORAGE_QUOTA_BYTES).toBe(EXPECTED_STORAGE_QUOTA_BYTES);
  });
});
