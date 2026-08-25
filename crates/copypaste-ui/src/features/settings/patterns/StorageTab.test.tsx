import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { TooltipProvider } from "@/components/ui";
import { StorageTab } from "./StorageTab";

vi.mock("@/hooks/useStatus", () => ({
  statusItemCount: (status: { item_count: number }) => status.item_count,
  useStatus: () => ({ data: 12, isError: false, isPending: false }),
}));

vi.mock("@/hooks/useDeferredDelete", () => ({
  useDeferredDelete: () => ({ pendingAll: false, removeAll: vi.fn() }),
}));

vi.mock("@/hooks/useServiceConfig", () => {
  const mutation = () => ({
    isError: false,
    isPending: false,
    mutate: vi.fn(),
  });
  return {
    useBackupDatabase: mutation,
    useExportHistory: mutation,
    useImportHistory: () => ({
      apply: mutation(),
      cancel: mutation(),
      isPending: false,
      prepare: mutation(),
    }),
    useRestoreDatabase: mutation,
  };
});

describe("StorageTab", () => {
  it("contains storage, transfer, recovery, and destructive actions once", () => {
    render(
      <TooltipProvider>
        <StorageTab />
      </TooltipProvider>,
    );

    for (const heading of [
      "Stored history",
      "Danger zone",
      "Import and export",
      "Backup and restore",
    ]) {
      expect(screen.getByRole("heading", { name: heading })).toBeTruthy();
    }

    for (const action of [
      "Clear history",
      "Export…",
      "Import…",
      "Back up…",
      "Restore…",
    ]) {
      expect(screen.getAllByRole("button", { name: action })).toHaveLength(1);
    }
  });
});
