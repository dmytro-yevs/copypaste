import { useState } from "react";
import { screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { withUser } from "@/test/harness";
import { TooltipProvider } from "@/components/ui/tooltip";
import { SourceExclusions } from "./SourceExclusions";

const ipc = vi.hoisted(() => ({ listInstalledSourceApps: vi.fn() }));
vi.mock("@/lib/ipc", async (importOriginal) => ({
    ...(await importOriginal<typeof import("@/lib/ipc")>()),
    listInstalledSourceApps: () => ipc.listInstalledSourceApps(),
}));
vi.mock("@/hooks/useHistory", () => ({
    useHistory: () => ({ data: { items: [] } }),
}));
vi.mock("@/features/source-apps", () => ({
    SourceAppIcon: () => <span aria-hidden="true" />,
}));

afterEach(() => {
    window.history.replaceState({}, "", "/");
    ipc.listInstalledSourceApps.mockReset();
});

describe("SourceExclusions", () => {
    it("normalizes Windows catalog IDs and keeps a manual selection outside the catalog", async () => {
        window.history.replaceState({}, "", "/?platform=windows");
        ipc.listInstalledSourceApps.mockResolvedValue([{ package_id: "Chrome.EXE", label: "Chrome" }]);
        function Probe() {
            const [ids, setIds] = useState(["manual.exe"]);
            return <SourceExclusions ids={ids} onChange={setIds} />;
        }
        const { user } = withUser(<TooltipProvider><Probe /></TooltipProvider>);

        expect(screen.getByRole("button", { name: "Remove manual.exe" })).toBeTruthy();
        await user.click((await screen.findByText("Chrome")).closest("button")!);
        await waitFor(() => expect(screen.getByRole("button", { name: "Remove chrome.exe" })).toBeTruthy());
        expect(screen.getByRole("button", { name: "Remove manual.exe" })).toBeTruthy();
        await user.click(screen.getByRole("button", { name: "Remove manual.exe" }));
        expect(screen.queryByRole("button", { name: "Remove manual.exe" })).toBeNull();
    });

    it("links manual validation and normalization notices to the input", async () => {
        window.history.replaceState({}, "", "/?platform=windows");
        ipc.listInstalledSourceApps.mockResolvedValue([]);
        const { user } = withUser(<TooltipProvider><SourceExclusions ids={[]} onChange={vi.fn()} /></TooltipProvider>);
        const input = screen.getByRole("textbox", { name: "Program name" });
        await user.type(input, ".exe");
        await user.click(screen.getByRole("button", { name: "Add app" }));
        const error = screen.getByRole("alert");
        expect(error.textContent).toContain("Enter a program name");
        expect(input.getAttribute("aria-describedby")).toBe(error.id);
        expect(input.getAttribute("aria-invalid")).toBe("true");

        await user.clear(input);
        await user.type(input, "Chrome.EXE");
        await user.click(screen.getByRole("button", { name: "Add app" }));
        const notice = screen.getByText("Added as chrome.exe.").closest("[role=status]");
        expect(notice).toBeTruthy();
        expect(input.getAttribute("aria-describedby")).toBe(notice?.id);
    });
});
