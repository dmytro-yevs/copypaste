import { useState } from "react";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { TooltipProvider } from "./tooltip";
import { Select, type SelectItem } from "./select";

const catalog = {
    searchLabel: "Search applications",
    listLabel: "Installed applications",
    emptyLabel: "No matching applications",
    loadingLabel: "Loading applications",
    errorLabel: "Applications unavailable",
    errorDescription: "Check permissions and try again",
    retryLabel: "Try again",
    refreshLabel: "Refresh applications",
    removeLabel: (id: string) => `Remove ${id}`,
    onRetry: vi.fn(),
    disableSelectedOptions: true,
};

function CatalogProbe({ items, initial = [] }: { items: readonly SelectItem[]; initial?: string[] }) {
    const [values, setValues] = useState(initial);
    return <Select mode="multiple" display="catalog" items={items} values={values} onValuesChange={setValues} catalog={{ ...catalog, selectedItems: values.map((value) => ({ value, label: value })) }} />;
}

describe("Select", () => {
    it("retains unknown selected IDs independently of filtering and a large virtualized catalog", async () => {
        const user = userEvent.setup();
        const items = Array.from({ length: 1_000 }, (_, index) => ({ value: `app.${index}`, label: `Application ${index}` }));
        render(<CatalogProbe items={items} initial={["manual.unknown"]} />);

        expect(screen.getByRole("button", { name: "Remove manual.unknown" })).toBeTruthy();
        const list = screen.getByRole("list", { name: "Installed applications" });
        await waitFor(() => expect(within(list).getAllByRole("listitem").length).toBeGreaterThan(0));
        expect(within(list).getAllByRole("listitem").length).toBeLessThan(100);

        await user.type(screen.getByRole("textbox", { name: "Search applications" }), "Application 999");
        await waitFor(() => expect(screen.getByText("Application 999")).toBeTruthy());
        expect(screen.getByRole("button", { name: "Remove manual.unknown" })).toBeTruthy();
        await user.click(screen.getByRole("button", { name: "Remove manual.unknown" }));
        expect(screen.queryByRole("button", { name: "Remove manual.unknown" })).toBeNull();
    });

    it("keeps retry available after catalog failure and uses shared states", async () => {
        const user = userEvent.setup();
        const retry = vi.fn();
        render(<Select mode="multiple" display="catalog" items={[]} values={["manual.unknown"]} onValuesChange={vi.fn()} catalog={{ ...catalog, failed: true, onRetry: retry, selectedItems: [{ value: "manual.unknown", label: "Manual entry" }] }} />);
        expect(screen.getByRole("alert").textContent).toContain("Applications unavailable");
        expect(screen.getByText("Manual entry")).toBeTruthy();
        await user.click(screen.getByRole("button", { name: "Try again" }));
        expect(retry).toHaveBeenCalledOnce();
    });

    it("adds catalog entries and supports arrow-key navigation", async () => {
        const user = userEvent.setup();
        render(<CatalogProbe items={[{ value: "app.one", label: "One" }, { value: "app.two", label: "Two" }]} />);
        const one = await screen.findByRole("button", { name: "One" });
        one.focus();
        await user.keyboard("{ArrowDown}");
        await waitFor(() => expect(document.activeElement).toBe(screen.getByRole("button", { name: "Two" })));
        await user.keyboard("{Enter}");
        expect(screen.getByRole("button", { name: "Remove app.two" })).toBeTruthy();
        expect(screen.getByRole("list", { name: "Installed applications" })).toBeTruthy();
    });

    it("closes after single selection and stays open after multiple selection", async () => {
        const user = userEvent.setup();
        function Probe() {
            const [one, setOne] = useState("one");
            const [many, setMany] = useState<string[]>([]);
            const items = [{ value: "one", label: "One" }, { value: "two", label: "Two" }];
            return <TooltipProvider><Select aria-label="Single" items={items} value={one} onValueChange={setOne} /><Select mode="multiple" aria-label="Multiple" items={items} values={many} onValuesChange={setMany} allLabel="All" /></TooltipProvider>;
        }
        render(<Probe />);
        await user.click(screen.getByRole("combobox", { name: "Single: One" }));
        await user.click(screen.getByRole("option", { name: "Two" }));
        expect(screen.queryByRole("option", { name: "One" })).toBeNull();
        await user.click(screen.getByRole("button", { name: "Multiple: All" }));
        await user.click(screen.getByRole("menuitemcheckbox", { name: "One" }));
        expect(screen.getByRole("menuitemcheckbox", { name: "Two" })).toBeTruthy();
    });
});
