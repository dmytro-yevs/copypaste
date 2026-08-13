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
import type { ExportReport, ImportPreview } from "@/lib/ipc";
import { status, withUser } from "@/test/harness";

const getStatus = vi.fn();
const exportHistory = vi.fn();
const prepareImportHistory = vi.fn();
const applyImportHistory = vi.fn();
const cancelImportHistory = vi.fn();
const backupDatabase = vi.fn();
const restoreDatabase = vi.fn();

const toasts: string[] = [];
const toastKinds: string[] = [];

vi.mock("sonner", () => {
  const record = (kind: string) => (message: string) => {
    toasts.push(message);
    toastKinds.push(kind);
  };
  return {
    toast: Object.assign(record("default"), {
      success: record("success"),
      warning: record("warning"),
      error: record("error"),
    }),
  };
});

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    getStatus: () => getStatus(),
    exportHistory: (includeSensitive: boolean) => exportHistory(includeSensitive),
    prepareImportHistory: () => prepareImportHistory(),
    applyImportHistory: (token: string) => applyImportHistory(token),
    cancelImportHistory: (token: string) => cancelImportHistory(token),
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

function preview(over: Partial<ImportPreview> = {}): ImportPreview {
  return { token: "preview-1", item_count: 4, ...over };
}

beforeEach(() => {
  toasts.length = 0;
  toastKinds.length = 0;
  getStatus.mockReset().mockResolvedValue(status());
  exportHistory.mockReset().mockResolvedValue(report());
  prepareImportHistory.mockReset().mockResolvedValue(preview());
  applyImportHistory.mockReset().mockResolvedValue({
    inserted: 3,
    skipped: 1,
    skipped_duplicate: 1,
    skipped_empty: 0,
    skipped_too_large: 0,
    pins_failed: 0,
  });
  cancelImportHistory.mockReset().mockResolvedValue(undefined);
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
  it("opens the file panel before it asks for confirmation", async () => {
    prepareImportHistory.mockReturnValue(new Promise(() => {}));
    const { user } = withUser(<StorageTab />);
    await user.click(screen.getByRole("button", { name: "Import…" }));

    expect(prepareImportHistory).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(applyImportHistory).not.toHaveBeenCalled();
  });

  it("previews the validated item count without writing", async () => {
    prepareImportHistory.mockResolvedValue(preview({ item_count: 17 }));
    const { user } = withUser(<StorageTab />);
    await user.click(screen.getByRole("button", { name: "Import…" }));

    expect(
      await screen.findByRole("heading", { name: "Import 17 items?" }),
    ).toBeTruthy();
    expect(screen.getByText(/This will import 17 items from the file/)).toBeTruthy();
    expect(applyImportHistory).not.toHaveBeenCalled();
  });

  it("does not confirm or write when the chosen file is invalid", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    prepareImportHistory.mockRejectedValue(
      new Error("invalid export at /Users/alice/private/broken.json"),
    );
    const { user } = withUser(<StorageTab />);
    await user.click(screen.getByRole("button", { name: "Import…" }));

    await waitFor(() => expect(toasts).toHaveLength(1));
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(applyImportHistory).not.toHaveBeenCalled();
    expect(toasts[0]).not.toContain("/Users/");
    expect(document.body.textContent).not.toContain("alice");
  });

  it("does nothing when the file panel is dismissed", async () => {
    prepareImportHistory.mockResolvedValue(null);
    const { user } = withUser(<StorageTab />);
    await user.click(screen.getByRole("button", { name: "Import…" }));

    await waitFor(() => expect(prepareImportHistory).toHaveBeenCalledTimes(1));
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(applyImportHistory).not.toHaveBeenCalled();
    expect(toasts).toHaveLength(0);
  });

  it("cancels the prepared import without writing", async () => {
    const { user } = withUser(<StorageTab />);
    await user.click(screen.getByRole("button", { name: "Import…" }));
    await user.click(await screen.findByRole("button", { name: "Cancel" }));

    await waitFor(() =>
      expect(cancelImportHistory).toHaveBeenCalledWith("preview-1"),
    );
    expect(applyImportHistory).not.toHaveBeenCalled();
  });

  it("applies one prepared import exactly once after confirmation", async () => {
    const { user } = withUser(<StorageTab />);
    await user.click(screen.getByRole("button", { name: "Import…" }));
    await user.click(await screen.findByRole("button", { name: "Import" }));

    await waitFor(() =>
      expect(applyImportHistory).toHaveBeenCalledWith("preview-1"),
    );
    expect(applyImportHistory).toHaveBeenCalledTimes(1);
    expect(cancelImportHistory).not.toHaveBeenCalled();
  });

  it("reports what went in and what was already there", async () => {
    const { user } = withUser(<StorageTab />);
    await user.click(screen.getByRole("button", { name: "Import…" }));
    await user.click(await screen.findByRole("button", { name: "Import" }));
    await waitFor(() => expect(toasts).toHaveLength(1));
    expect(toasts[0]).toContain("Imported 3 items");
    expect(toasts[0]).toContain("1 was already here");
    expect(toastKinds[0]).toBe("success");
  });

  /** DMY-156: the items landed, so this is not an error — but a pin the file
   *  named is missing, and a plain success toast would let the user believe
   *  their pinned items came back. */
  it("warns rather than celebrating when a pin could not be restored", async () => {
    applyImportHistory.mockResolvedValue({
      inserted: 3,
      skipped: 0,
      skipped_duplicate: 0,
      skipped_empty: 0,
      skipped_too_large: 0,
      pins_failed: 2,
    });
    const { user } = withUser(<StorageTab />);
    await user.click(screen.getByRole("button", { name: "Import…" }));
    await user.click(await screen.findByRole("button", { name: "Import" }));

    await waitFor(() => expect(toasts).toHaveLength(1));
    expect(toasts[0]).toContain("Imported 3 items");
    expect(toasts[0]).toContain("2 items could not be pinned");
    expect(toasts[0]).toContain("import the file again to retry");
    expect(toastKinds[0]).toBe("warning");
  });
});

describe("backup", () => {
  /** No confirmation: it writes a new file and touches nothing that exists. */
  it("goes straight to the panel", async () => {
    const { user } = withUser(<StorageTab />);
    await user.click(screen.getByRole("button", { name: "Back up…" }));
    await waitFor(() => expect(backupDatabase).toHaveBeenCalledTimes(1));
  });

  it("says a backup does not overwrite an existing one", () => {
    withUser(<StorageTab />);
    expect(screen.getByText(/without overwriting backups/)).toBeTruthy();
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
