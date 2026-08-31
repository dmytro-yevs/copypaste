import { createRef } from "react";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { TooltipProvider } from "@/components/ui";
import type { OriginDevice } from "@/lib/itemOrigin";
import { DEFAULT_VIEW, type ViewOptions } from "@/lib/view";
import {
    DEFAULT_PREFS,
    PREFERENCES_VERSION,
    STORAGE_KEY,
    usePrefs,
} from "@/store/prefs";
import { LibraryToolbar } from "./LibraryToolbar";

const toolbarSize = vi.hoisted(() => ({ width: 900 }));

vi.mock("@/hooks/useViewportMetrics", async (importOriginal) => ({
    ...(await importOriginal<typeof import("@/hooks/useViewportMetrics")>()),
    useObservedElementSize: () => ({
        ref: () => {},
        width: toolbarSize.width,
        height: 48,
    }),
}));

const ORIGINS: readonly OriginDevice[] = [
    { id: "desktop", name: "Desk", kind: "desktop" },
    { id: "phone", name: "Phone", kind: "phone" },
];

const BASE_PROPS = {
    value: "",
    onChange: vi.fn(),
    onEnterList: vi.fn(),
    inputRef: createRef<HTMLInputElement>(),
    filtered: false,
    visible: 4,
    total: 4,
    view: DEFAULT_VIEW,
    onViewChange: vi.fn(),
    origins: ORIGINS,
    displayLimit: null,
};

type ToolbarProps = Parameters<typeof LibraryToolbar>[0];

function toolbar(overrides: Partial<ToolbarProps> = {}) {
    return (
        <TooltipProvider>
            <LibraryToolbar {...BASE_PROPS} {...overrides} />
        </TooltipProvider>
    );
}

function controlIndicator(control: HTMLElement): HTMLElement | null {
    return control
        .closest<HTMLElement>('[data-slot="active-control"]')
        ?.querySelector<HTMLElement>(
            ':scope > [data-slot="active-control-indicator"]',
        ) ?? null;
}

function expectActive(control: HTMLElement) {
    const indicator = controlIndicator(control);
    expect(indicator).not.toBeNull();
    expect(indicator?.getAttribute("aria-hidden")).toBe("true");
    expect(indicator?.textContent).toBe("");
    expect(indicator?.hasAttribute("tabindex")).toBe(false);
}

function expectDefault(control: HTMLElement) {
    expect(controlIndicator(control)).toBeNull();
}

beforeEach(() => {
    toolbarSize.width = 900;
    window.localStorage.clear();
    usePrefs.setState(DEFAULT_PREFS);
});

afterEach(() => {
    vi.clearAllMocks();
});

describe("Library toolbar active-control badges", () => {
    it("keeps the kind menu state on its trigger", async () => {
        const user = userEvent.setup();
        render(toolbar());
        const trigger = screen.getByRole("button", {
            name: "Filter by kind, default: All kinds",
        });

        await user.click(trigger);
        expect(trigger.getAttribute("aria-expanded")).toBe("true");
        expect(trigger.getAttribute("data-state")).toBe("open");
        expect(
            screen.getByRole("menuitemcheckbox", { name: "Links" }),
        ).toBeTruthy();

        await user.keyboard("{Escape}");
        expect(trigger.getAttribute("aria-expanded")).toBe("false");
        expect(trigger.getAttribute("data-state")).toBe("closed");
    });

    it("keeps the sort menu state on its trigger", async () => {
        const user = userEvent.setup();
        render(toolbar());
        const trigger = screen.getByRole("combobox", {
            name: "Sort order, default: Newest first",
        });

        await user.click(trigger);
        expect(trigger.getAttribute("aria-expanded")).toBe("true");
        expect(trigger.getAttribute("data-state")).toBe("open");
        expect(
            screen.getByRole("option", { name: "Oldest first" }),
        ).toBeTruthy();

        await user.keyboard("{Escape}");
        expect(trigger.getAttribute("aria-expanded")).toBe("false");
        expect(trigger.getAttribute("data-state")).toBe("closed");
    });

    it("has no badges at defaults and says each control is default", () => {
        const { container } = render(toolbar());

        expect(
            screen.getByRole("searchbox", {
                name: "Search clipboard history, default",
            }),
        ).toBeTruthy();
        expect(
            screen.getByRole("button", {
                name: "Filter by kind, default: All kinds",
            }),
        ).toBeTruthy();
        expect(
            screen.getByRole("button", {
                name: "Filter by device, default: All devices",
            }),
        ).toBeTruthy();
        expect(
            screen.getByRole("combobox", {
                name: "Sort order, default: Newest first",
            }),
        ).toBeTruthy();
        expect(
            screen
                .getByRole("button", {
                    name: "Group clipboard items by device, default",
                })
                .getAttribute("aria-pressed"),
        ).toBe("false");
        expect(
            container.querySelectorAll(
                '[data-slot="active-control-indicator"]',
            ),
        ).toHaveLength(0);
    });

    it("adds and removes the search badge with the effective query", () => {
        const { rerender } = render(toolbar());

        rerender(toolbar({ value: "invoice" }));
        expectActive(
            screen.getByRole("searchbox", {
                name: "Search clipboard history, active",
            }),
        );

        rerender(toolbar({ value: "   " }));
        expectDefault(
            screen.getByRole("searchbox", {
                name: "Search clipboard history, default",
            }),
        );
    });

    it("adds and removes the kind-filter badge", () => {
        const { rerender } = render(toolbar());
        const activeView: ViewOptions = {
            ...DEFAULT_VIEW,
            kinds: ["image"],
        };

        rerender(toolbar({ view: activeView }));
        expectActive(
            screen.getByRole("button", {
                name: "Filter by kind, active: Images",
            }),
        );

        rerender(toolbar());
        expectDefault(
            screen.getByRole("button", {
                name: "Filter by kind, default: All kinds",
            }),
        );
    });

    it("adds and removes the device-filter badge", () => {
        const { rerender } = render(toolbar());

        rerender(
            toolbar({ view: { ...DEFAULT_VIEW, devices: ["phone"] } }),
        );
        expectActive(
            screen.getByRole("button", {
                name: "Filter by device, active: Phone",
            }),
        );

        rerender(toolbar());
        expectDefault(
            screen.getByRole("button", {
                name: "Filter by device, default: All devices",
            }),
        );
    });

    it("adds and removes the non-default sort badge", () => {
        const { rerender } = render(toolbar());

        rerender(toolbar({ view: { ...DEFAULT_VIEW, sort: "oldest" } }));
        expectActive(
            screen.getByRole("combobox", {
                name: "Sort order, active: Oldest first",
            }),
        );

        rerender(toolbar());
        expectDefault(
            screen.getByRole("combobox", {
                name: "Sort order, default: Newest first",
            }),
        );
    });

    it("restores and resets the persisted grouping badge", async () => {
        window.localStorage.setItem(
            STORAGE_KEY,
            JSON.stringify({
                state: { sortByDevice: true },
                version: PREFERENCES_VERSION,
            }),
        );
        await act(async () => usePrefs.persist.rehydrate());

        function PrefsToolbar() {
            const groupByDevice = usePrefs((state) => state.sortByDevice);
            return toolbar({
                view: { ...DEFAULT_VIEW, groupByDevice },
            });
        }

        render(<PrefsToolbar />);
        expectActive(
            screen.getByRole("button", {
                name: "Group clipboard items by device, active",
            }),
        );

        window.localStorage.setItem(
            STORAGE_KEY,
            JSON.stringify({
                state: { sortByDevice: false },
                version: PREFERENCES_VERSION,
            }),
        );
        await act(async () => usePrefs.persist.rehydrate());
        expectDefault(
            screen.getByRole("button", {
                name: "Group clipboard items by device, default",
            }),
        );
    });

    it("moves the search badge to the compact trigger without losing others", () => {
        toolbarSize.width = 390;
        const view: ViewOptions = {
            kinds: ["image"],
            devices: ["phone"],
            sort: "oldest",
            groupByDevice: true,
        };
        const { container, rerender } = render(
            toolbar({ value: "invoice", view }),
        );
        const controls = screen.getByRole("toolbar", {
            name: "Library controls",
        });
        expect(controls.getAttribute("data-compact-search")).toBe("true");
        const searchTrigger = controls.querySelector<HTMLElement>(
            'button[aria-controls][aria-label="Search clipboard history, active"]',
        );
        expect(searchTrigger).not.toBeNull();
        if (searchTrigger) expectActive(searchTrigger);
        expect(
            container.querySelectorAll(
                '[data-slot="active-control-indicator"]',
            ),
        ).toHaveLength(5);

        toolbarSize.width = 900;
        rerender(toolbar({ value: "invoice", view }));
        expect(controls.hasAttribute("data-compact-search")).toBe(false);
        expectActive(
            screen.getByRole("searchbox", {
                name: "Search clipboard history, active",
            }),
        );
        expect(
            container.querySelectorAll(
                '[data-slot="active-control-indicator"]',
            ),
        ).toHaveLength(5);
    });

    it("does not badge the plain bulk actions", () => {
        const { container } = render(
            toolbar({
                value: "invoice",
                selection: {
                    count: 1,
                    total: 4,
                    allSelected: false,
                    allPinned: false,
                    busy: false,
                    onToggleAll: vi.fn(),
                    onSelectAll: vi.fn(),
                    onTogglePin: vi.fn(),
                    onDelete: vi.fn(),
                    onClose: vi.fn(),
                },
            }),
        );

        expect(
            screen.getByRole("toolbar", { name: "Selection actions" }),
        ).toBeTruthy();
        expect(
            container.querySelectorAll(
                '[data-slot="active-control-indicator"]',
            ),
        ).toHaveLength(0);
    });
});
