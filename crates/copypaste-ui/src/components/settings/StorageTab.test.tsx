/**
 * Export, import, backup and restore — and the confirmations in front of them.
 *
 * Two rules are what these assert:
 *
 * **An export that withheld something has to say so.** The count comes back
 * from the service precisely so a shorter file and a smaller history are not
 * the same thing to the user (`P2-tj9s`, `CopyPaste-93yr`).
 *
 * **A restore replaces everything.** It is the most destructive thing the
 * product does, so the dialog names what goes rather than asking "are you
 * sure?", and cancelling has to reach nothing.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor } from "@testing-library/react";

import { StorageTab } from "@/components/settings/StorageTab";
import type { ExportReport } from "@/lib/ipc";
import { status, withUser } from "@/test/harness";

const getStatus = vi.fn();
const exportHistory = vi.fn();
const importHistory = vi.fn();
const backupDatabase = vi.fn();
const restoreDatabase = vi.fn();

const toasts: string[] = [];

vi.mock("sonner", () => {
  const record = (message: string) => toasts.push(message);
  return {
    toast: Object.assign(record, {
      success: record,
      warning: record,
      error: record,
    }),
  };
});

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    getStatus: () => getStatus(),
    exportHistory: (includeSensitive: boolean) => exportHistory(includeSensitive),
    importHistory: () => importHistory(),
    backupDatabase: () => backupDatabase(),
    restoreDatabase: () => restoreDatabase(),
  };
});

function report(over: Partial<ExportReport> = {}): ExportReport {
  return {
    exported: 12,
    skipped_sensitive: 0,
    skipped_non_text: 0,
    skipped_undecryptable: 0,
    ...over,
  };
}

beforeEach(() => {
  toasts.length = 0;
  getStatus.mockReset().mockResolvedValue(status());
  exportHistory.mockReset().mockResolvedValue(report());
  importHistory.mockReset().mockResolvedValue({ inserted: 3, skipped: 1 });
  backupDatabase.mockReset().mockResolvedValue(2_500_000);
  restoreDatabase.mockReset().mockResolvedValue(true);
});

afterEach(() => vi.restoreAllMocks());

describe("export", () => {
  it("warns that the file is readable before it writes one", async () => {
    const { user } = withUser(<StorageTab />);
    await user.click(screen.getByRole("button", { name: "Export…" }));
    expect(
      await screen.findByText(/plain readable text, not encrypted/),
    ).toBeTruthy();
    expect(exportHistory).not.toHaveBeenCalled();
  });

  /** The wire default is false and the client default is false; the checkbox is
   *  the second ask, and it must not be pre-answered. */
  it("withholds flagged items unless the box in the dialog is ticked", async () => {
    const { user } = withUser(<StorageTab />);
    await user.click(screen.getByRole("button", { name: "Export…" }));
    await user.click(await screen.findByRole("button", { name: "Choose where to save" }));
    await waitFor(() => expect(exportHistory).toHaveBeenCalledWith(false));
  });

  it("passes the opt-in through when it is ticked", async () => {
    const { user } = withUser(<StorageTab />);
    await user.click(screen.getByRole("button", { name: "Export…" }));
    await user.click(
      await screen.findByRole("checkbox", {
        name: /Include items that look like passwords/,
      }),
    );
    await user.click(screen.getByRole("button", { name: "Choose where to save" }));
    await waitFor(() => expect(exportHistory).toHaveBeenCalledWith(true));
  });

  it("forgets the opt-in as soon as the dialog closes", async () => {
    const { user } = withUser(<StorageTab />);
    await user.click(screen.getByRole("button", { name: "Export…" }));
    await user.click(
      await screen.findByRole("checkbox", {
        name: /Include items that look like passwords/,
      }),
    );
    await user.click(screen.getByRole("button", { name: "Cancel" }));

    await user.click(screen.getByRole("button", { name: "Export…" }));
    expect(
      (
        await screen.findByRole("checkbox", {
          name: /Include items that look like passwords/,
        })
      ).getAttribute("aria-checked"),
    ).toBe("false");
  });

  /** The whole reason the count is on the wire: a user who is not told believes
   *  they exported everything. */
  it("reports what it left out, and does not read as a plain success", async () => {
    exportHistory.mockResolvedValue(report({ exported: 9, skipped_sensitive: 3 }));
    const { user } = withUser(<StorageTab />);
    await user.click(screen.getByRole("button", { name: "Export…" }));
    await user.click(await screen.findByRole("button", { name: "Choose where to save" }));

    await waitFor(() => expect(toasts).toHaveLength(1));
    expect(toasts[0]).toContain("Exported 9 items");
    expect(toasts[0]).toContain("3 that look like secrets were left out");
  });

  /** A closed save panel is not a failure and must not toast at all. */
  it("says nothing when the panel is dismissed", async () => {
    exportHistory.mockResolvedValue(null);
    const { user } = withUser(<StorageTab />);
    await user.click(screen.getByRole("button", { name: "Export…" }));
    await user.click(await screen.findByRole("button", { name: "Choose where to save" }));
    await waitFor(() => expect(exportHistory).toHaveBeenCalled());
    expect(toasts).toHaveLength(0);
  });
});

describe("import", () => {
  /** "Import" reads as "overwrite". It is not one, and the dialog has to say
   *  the true thing rather than a scarier one. */
  it("says nothing already here is deleted", async () => {
    const { user } = withUser(<StorageTab />);
    await user.click(screen.getByRole("button", { name: "Import…" }));
    expect(await screen.findByText(/Nothing here is deleted/)).toBeTruthy();
  });

  it("reports what went in and what was already there", async () => {
    const { user } = withUser(<StorageTab />);
    await user.click(screen.getByRole("button", { name: "Import…" }));
    await user.click(await screen.findByRole("button", { name: "Choose what to import" }));
    await waitFor(() => expect(toasts).toHaveLength(1));
    expect(toasts[0]).toContain("Imported 3 items");
    expect(toasts[0]).toContain("1 was already here");
  });
});

describe("backup", () => {
  /** No confirmation: it writes a new file and touches nothing that exists. */
  it("goes straight to the panel", async () => {
    const { user } = withUser(<StorageTab />);
    await user.click(screen.getByRole("button", { name: "Back up…" }));
    await waitFor(() => expect(backupDatabase).toHaveBeenCalledTimes(1));
  });

  it("says a backup is never written over an existing one", () => {
    withUser(<StorageTab />);
    expect(screen.getByText(/never written over something that already exists/)).toBeTruthy();
  });
});

describe("restore", () => {
  it("names what is lost rather than asking whether the user is sure", async () => {
    const { user } = withUser(<StorageTab />);
    await user.click(screen.getByRole("button", { name: "Restore…" }));
    const body = await screen.findByText(/Every clip on this device is deleted/);
    expect(body.textContent).toContain("pinned items included");
    expect(body.textContent).toContain("cannot be undone");
  });

  it("reaches nothing when it is cancelled", async () => {
    const { user } = withUser(<StorageTab />);
    await user.click(screen.getByRole("button", { name: "Restore…" }));
    await user.click(await screen.findByRole("button", { name: "Cancel" }));
    expect(restoreDatabase).not.toHaveBeenCalled();
  });

  it("runs only after the replace is confirmed", async () => {
    const { user } = withUser(<StorageTab />);
    await user.click(screen.getByRole("button", { name: "Restore…" }));
    await user.click(await screen.findByRole("button", { name: "Choose a backup" }));
    await waitFor(() => expect(restoreDatabase).toHaveBeenCalledTimes(1));
  });
});

it("names no path anywhere (INV-12)", async () => {
  const { user, container } = withUser(<StorageTab />);
  for (const label of ["Export…", "Import…", "Restore…"]) {
    await user.click(screen.getByRole("button", { name: label }));
    expect(document.body.innerHTML).not.toMatch(/\/Users\/|\/home\/|~\/|\.sock\b/);
    await user.click(await screen.findByRole("button", { name: "Cancel" }));
  }
  expect(container.innerHTML).not.toMatch(/\/Users\/|\/home\/|~\//);
});
