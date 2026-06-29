/**
 * SyncTab crh3.15 — single signed-in banner
 * SyncTab crh3.17 — consistent credential field labels
 */
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { SyncTab } from "./SyncTab";
import type { SyncTabProps } from "./SyncTab";

// ---------------------------------------------------------------------------
// Minimal prop factory (all non-data props are no-ops / defaults)
// ---------------------------------------------------------------------------

const noop = () => {};

function makeProps(overrides: Partial<SyncTabProps> = {}): SyncTabProps {
  return {
    offline: false,
    syncEnabled: true,
    syncOnWifiOnly: false,
    autoApplySyncedClip: true,
    config: { p2p_enabled: true },
    syncRestarting: false,
    lanVisibility: true,
    supabaseUrl: "",
    setSupabaseUrl: noop,
    supabaseKey: "",
    setSupabaseKey: noop,
    supabaseEmail: "",
    setSupabaseEmail: noop,
    supabasePassword: "",
    setSupabasePassword: noop,
    relayUrl: "",
    setRelayUrl: noop,
    passphrase: "",
    setPassphrase: noop,
    passphraseSavedMsg: null,
    testMsg: null,
    testing: false,
    savedMsg: false,
    saveError: null,
    syncStatus: null,
    limitsMsg: {},
    inputCls: "",
    btnCls: "",
    btnStyle: {},
    handleWifiOnlyToggle: noop,
    handleAutoApplySyncedClipToggle: noop,
    handleP2pToggle: noop,
    handleLanVisibilityToggle: noop,
    handleSetPassphrase: noop,
    handleTestConnection: noop,
    handleSaveConfig: noop,
    cloudAccountMismatch: false,
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// crh3.15: single signed-in banner
// ---------------------------------------------------------------------------

describe("SyncTab crh3.15 — single signed-in banner", () => {
  it("renders exactly one status element when supabase_configured && signed_in && email", () => {
    const { container } = render(
      <SyncTab
        {...makeProps({
          syncStatus: {
            passphrase_set: false,
            supabase_configured: true,
            signed_in: true,
            email: "user@example.com",
            last_sync_ms: null,
          },
        })}
      />
    );

    // Only one surface-card signed-in element
    const signedInCards = container.querySelectorAll(".surface-card");
    // Filter to the signed-in banner specifically (contains "Signed in as")
    const signedInBanners = Array.from(signedInCards).filter((el) =>
      el.textContent?.includes("Signed in as")
    );
    expect(signedInBanners).toHaveLength(1);

    // No raw bg-ide-success/5 element (the old duplicate)
    const allDivs = Array.from(container.querySelectorAll("div"));
    const rawBanners = allDivs.filter((el) =>
      el.className.includes("bg-ide-success/5")
    );
    expect(rawBanners).toHaveLength(0);
  });

  it("shows the email in the single banner", () => {
    render(
      <SyncTab
        {...makeProps({
          syncStatus: {
            passphrase_set: false,
            supabase_configured: true,
            signed_in: true,
            email: "alice@example.com",
            last_sync_ms: null,
          },
        })}
      />
    );
    expect(screen.getByText(/Signed in as alice@example\.com/)).toBeInTheDocument();
    // The old raw div's "Connected ✓" text must NOT appear
    expect(screen.queryByText(/Connected ✓/)).not.toBeInTheDocument();
  });

  it("does not render a signed-in banner when syncStatus is null", () => {
    const { container } = render(
      <SyncTab {...makeProps({ syncStatus: null })} />
    );
    const signedInBanners = Array.from(
      container.querySelectorAll("div")
    ).filter((el) => el.textContent?.includes("Signed in as"));
    expect(signedInBanners).toHaveLength(0);
  });
});

// ---------------------------------------------------------------------------
// crh3.17: consistent credential field labels (Email / Password / Anon key)
// ---------------------------------------------------------------------------

describe("SyncTab crh3.17 — consistent Cloud Sync credential labels", () => {
  it("renders the email field as 'Email' (no Supabase prefix)", () => {
    render(<SyncTab {...makeProps()} />);
    // The SettingsRow title "Email" should be present
    expect(screen.getByText("Email")).toBeInTheDocument();
    // Must NOT contain "Supabase email"
    expect(screen.queryByText(/Supabase email/i)).not.toBeInTheDocument();
  });

  it("renders the password field as 'Password' (no Supabase prefix)", () => {
    render(<SyncTab {...makeProps()} />);
    expect(screen.getByText("Password")).toBeInTheDocument();
    expect(screen.queryByText(/Supabase password/i)).not.toBeInTheDocument();
  });

  it("still renders the Anon key field label unchanged", () => {
    render(<SyncTab {...makeProps()} />);
    expect(screen.getByText("Anon key")).toBeInTheDocument();
  });
});
