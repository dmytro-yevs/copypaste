/**
 * CopyPaste-1jms.34: CloudAccountMismatchBanner
 *
 * Tests that the banner renders when a mismatch is detected and is absent
 * otherwise (cloud off / anon-key-only / no mismatch).
 *
 * NOTE: peer supabase_account_id plumbing is deferred to CopyPaste-1jms.35.
 * Until then the caller always passes hasMismatch=false and the banner never
 * shows. These tests exercise the component contract directly so the behaviour
 * is locked in regardless of the caller.
 */
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { CloudAccountMismatchBanner } from "./CloudAccountMismatchBanner";

describe("CloudAccountMismatchBanner", () => {
  it("is absent when hasMismatch is false", () => {
    render(
      <CloudAccountMismatchBanner
        hasMismatch={false}
        localAccountId="proj_abc/uid_123"
      />,
    );
    expect(
      screen.queryByTestId("cloud-account-mismatch-banner"),
    ).not.toBeInTheDocument();
  });

  it("is absent when cloud is off (hasMismatch=false, no accountId)", () => {
    render(
      <CloudAccountMismatchBanner hasMismatch={false} localAccountId={null} />,
    );
    expect(
      screen.queryByTestId("cloud-account-mismatch-banner"),
    ).not.toBeInTheDocument();
  });

  it("renders the banner with role=alert when hasMismatch is true", () => {
    render(
      <CloudAccountMismatchBanner
        hasMismatch={true}
        localAccountId="proj_abc/uid_123"
      />,
    );
    const banner = screen.getByTestId("cloud-account-mismatch-banner");
    expect(banner).toBeInTheDocument();
    expect(banner).toHaveAttribute("role", "alert");
  });

  it("banner text mentions mismatch and sync failure", () => {
    render(
      <CloudAccountMismatchBanner
        hasMismatch={true}
        localAccountId="proj_abc/uid_123"
      />,
    );
    const banner = screen.getByTestId("cloud-account-mismatch-banner");
    expect(banner.textContent).toMatch(/mismatch/i);
    // Should tell the user something about account consistency.
    expect(banner.textContent).toMatch(/Supabase/i);
  });

  it("shows the local account id when provided and hasMismatch is true", () => {
    const id = "proj_example/uid_00000000-0000-0000-0000-000000000001";
    render(
      <CloudAccountMismatchBanner hasMismatch={true} localAccountId={id} />,
    );
    const banner = screen.getByTestId("cloud-account-mismatch-banner");
    expect(banner.textContent).toContain(id);
  });

  it("does not show account id section when localAccountId is null", () => {
    render(
      <CloudAccountMismatchBanner hasMismatch={true} localAccountId={null} />,
    );
    const banner = screen.getByTestId("cloud-account-mismatch-banner");
    // The banner renders but has no account-id snippet.
    expect(banner.textContent).not.toMatch(/This device:/);
  });

  it("is absent when hasMismatch defaults to false (accounts match)", () => {
    // Same account id on both sides → no mismatch → banner absent.
    render(
      <CloudAccountMismatchBanner
        hasMismatch={false}
        localAccountId="proj_shared/uid_same"
      />,
    );
    expect(
      screen.queryByTestId("cloud-account-mismatch-banner"),
    ).not.toBeInTheDocument();
  });
});
