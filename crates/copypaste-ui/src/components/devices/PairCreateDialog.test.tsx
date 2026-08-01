/**
 * The copy button, which for a while was a button that could not work.
 *
 * It called `writeText` from `@tauri-apps/plugin-clipboard-manager` while
 * `capabilities/default.json` deliberately withholds
 * `clipboard-manager:allow-write-text`, so the call always rejected — into an
 * empty `catch`. The user pressed Copy, saw nothing change, walked to the other
 * device and had nothing to paste.
 *
 * Two properties hold it shut: the write goes through the backend command, and
 * a failure is *reported*. The second is the one that would have made the first
 * bug visible on the day it shipped.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor } from "@testing-library/react";

import { PairCreateDialog } from "@/components/devices/PairCreateDialog";
import { withUser } from "@/test/harness";

const pairCreate = vi.fn();
const copyText = vi.fn();

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    pairCreate: (name: string) => pairCreate(name),
    copyText: (text: string) => copyText(text),
  };
});

vi.mock("@/components/devices/QrCode", () => ({
  QrCode: ({ label }: { label: string }) => <canvas role="img" aria-label={label} />,
}));

const CODE = "aaaabbbbccccddddeeeeffffgggghhhh";

beforeEach(() => {
  pairCreate.mockReset().mockResolvedValue({
    code: CODE,
    pairing_id: "pair-1",
    listen_addr: "192.168.1.24:7420",
  });
  copyText.mockReset().mockResolvedValue(undefined);
});

afterEach(() => vi.restoreAllMocks());

/** Generate a code, which is what puts the copy button on screen. */
async function withCode() {
  const { user } = withUser(<PairCreateDialog open onOpenChange={() => {}} />);
  await user.click(await screen.findByRole("button", { name: "Generate code" }));
  await screen.findByRole("button", { name: "Copy code" });
  return user;
}

describe("copying the pairing code", () => {
  it("goes through the backend, never the WebView's clipboard", async () => {
    const user = await withCode();
    await user.click(screen.getByRole("button", { name: "Copy code" }));

    await waitFor(() => expect(copyText).toHaveBeenCalledWith(CODE));
    expect(await screen.findByRole("button", { name: "Copied" })).toBeTruthy();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("says so when the copy fails, rather than looking like it worked", async () => {
    copyText.mockRejectedValue(new Error("no clipboard here"));
    const user = await withCode();
    await user.click(screen.getByRole("button", { name: "Copy code" }));

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toMatch(/couldn't be copied/i);
    expect(alert.textContent).toMatch(/read it off the screen/i);
    // The failure must not be reported by claiming success.
    expect(screen.queryByRole("button", { name: "Copied" })).toBeNull();
    // And it must not put the credential anywhere else on the way out.
    expect(alert.textContent).not.toContain(CODE);
  });
});

describe("revealing the pairing QR", () => {
  it("keeps the QR code off screen until the user explicitly reveals it", async () => {
    const user = await withCode();

    expect(screen.queryByLabelText(/pairing code as a QR code/i)).toBeNull();
    await user.click(screen.getByRole("button", { name: "Reveal QR code" }));
    expect(screen.getByLabelText(/pairing code as a QR code/i)).toBeTruthy();
  });
});
