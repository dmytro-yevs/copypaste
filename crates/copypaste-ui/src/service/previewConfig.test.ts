import { describe, expect, it } from "vitest";

import { previewConfigApplied, previewPrivateMode } from "./previewConfig";

describe("preview config read models", () => {
  it("returns a complete, consistent settings and privacy snapshot", () => {
    const applied = previewConfigApplied();
    const privateMode = previewPrivateMode();

    expect(applied).toMatchObject({
      config: {
        private_mode: false,
        poll_interval_ms: 500,
        history_limit: 10_000,
        dedup_window_secs: 60,
        sync_enabled: true,
        excluded_app_bundle_ids: [],
      },
      restart_required: [],
    });
    expect(Object.keys(applied.config)).toHaveLength(16);
    expect(privateMode).toEqual({
      private_mode: applied.config.private_mode,
      private_mode_epoch: 0,
    });
  });

  it("does not share mutable settings arrays across reads", () => {
    const first = previewConfigApplied();
    first.config.excluded_app_bundle_ids.push("com.example.preview");
    first.restart_required.push("poll_interval_ms");

    expect(previewConfigApplied()).toMatchObject({
      config: { excluded_app_bundle_ids: [] },
      restart_required: [],
    });
  });
});
