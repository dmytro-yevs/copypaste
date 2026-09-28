import { useState } from "react";
import { screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { withUser } from "@/test/harness";
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
        const { user } = withUser(<Probe />);

        expect(screen.getByRole("button", { name: "Remove manual.exe" })).toBeTruthy();
        await user.click(await screen.findByRole("button", { name: /Chrome Chrome\.EXE/ }));
        await waitFor(() => expect(screen.getByRole("button", { name: "Remove chrome.exe" })).toBeTruthy());
        expect(screen.getByRole("button", { name: "Remove manual.exe" })).toBeTruthy();
        await user.click(screen.getByRole("button", { name: "Remove manual.exe" }));
        expect(screen.queryByRole("button", { name: "Remove manual.exe" })).toBeNull();
    });
});
