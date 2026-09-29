import type { ConfigApplied, ConfigData, PrivateModeData } from "@/generated/ipc";

/** DEV-only read values for preview scenarios, mirroring ConfigData::default. */
const CONFIG = {
  private_mode: false,
  poll_interval_ms: 500,
  history_limit: 10_000,
  storage_quota_bytes: 10 * 1024 * 1024 * 1024,
  retention_days: 0,
  dedup_window_secs: 60,
  max_text_size_bytes: 4 * 1024 * 1024,
  max_image_size_bytes: 4 * 1024 * 1024,
  max_file_size_bytes: 4 * 1024 * 1024,
  max_decoded_image_mb: 50,
  excluded_app_bundle_ids: [],
  lan_visibility: true,
  sync_enabled: true,
  notify_on_copy: false,
  sound_on_copy: false,
} satisfies ConfigData;

export function previewConfigApplied(): ConfigApplied {
  return {
    config: { ...CONFIG, excluded_app_bundle_ids: [] },
    restart_required: [],
  };
}

export function previewPrivateMode(): PrivateModeData {
  return { private_mode: CONFIG.private_mode, private_mode_epoch: 0 };
}
