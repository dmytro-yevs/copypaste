/**
 * The frontend rules that cost something when broken.
 *
 * These exist because the app cannot be exercised end-to-end in CI here: the
 * WebKit webview does not execute under headless Xvfb without a GPU, so
 * launching the binary proves the shell starts and nothing about what it
 * renders. Everything below runs in jsdom and needs no display.
 *
 * Each case is a rule from port-manifest/06-ui-behaviour.md that v1 paid for.
 */
import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";

import { HistoryRow, SENSITIVE_A11Y_LABEL, rowLabel } from "./components/HistoryRow";
import { classifyError, friendlyError, toFriendly } from "./lib/errors";
import { ROW_HEIGHT, PREVIEW_LINES, TITLE_LINE_PX, SINGLE_LINE_FLOOR } from "./lib/layout";
import type { Item } from "./lib/ipc";

const SECRET = "AKIAIOSFODNN7EXAMPLE";

function item(over: Partial<Item> = {}): Item {
    return {
        id: "row-1",
        content: "an ordinary clipboard entry",
        content_type: "text",
        created_at: Date.now(),
        pinned: false,
        is_sensitive: false,
        ...over,
    };
}

const noop = () => {};
const rowProps = {
    onCopy: noop,
    onDelete: noop,
    onTogglePin: noop,
    isActive: false,
    onActivate: noop,
    style: {},
};

describe("a sensitive item never discloses its content", () => {
    it("keeps the plaintext out of the DOM entirely", () => {
        const { container } = render(
            // eslint-disable-next-line @typescript-eslint/no-explicit-any
            <HistoryRow {...(rowProps as any)} item={item({ content: SECRET, is_sensitive: true })} />,
        );
        // Not merely blurred or clipped — absent. A CSS-only treatment would
        // still ship the secret to the DOM, where a screenshot or the
        // accessibility tree would expose it.
        expect(container.innerHTML).not.toContain(SECRET);
        expect(container.textContent ?? "").not.toContain(SECRET);
    });

    it("labels the row without quoting the secret", () => {
        const secret = item({ content: SECRET, is_sensitive: true });
        expect(rowLabel(secret)).toBe(SENSITIVE_A11Y_LABEL);
        expect(rowLabel(secret)).not.toContain(SECRET);
    });

    it("still renders ordinary content", () => {
        render(
            // eslint-disable-next-line @typescript-eslint/no-explicit-any
            <HistoryRow {...(rowProps as any)} item={item({ content: "visible text" })} />,
        );
        expect(screen.getByText(/visible text/)).toBeTruthy();
    });
});

describe("no error shown to a user carries a filesystem path", () => {
    // The daemon socket lives under the home directory, so its path spells out
    // the local username. This is the frontend half of CLAUDE.md rule 4.
    const leaky = [
        "connection refused on /Users/dmytro/Library/Application Support/CopyPaste/daemon.sock",
        "No such file or directory (os error 2): /home/dmytro/.local/share/CopyPaste/daemon.sock",
        new Error("failed to open /Users/someone/secret/path"),
        { message: "/home/other/x" },
    ];

    it.each(leaky.map((raw, i) => [i, raw] as const))(
        "case %i is mapped to fixed copy, not echoed",
        (_i, raw) => {
            const text = toFriendly(raw);
            expect(text).not.toMatch(/\/Users\/|\/home\/|\.sock/);
            expect(text).not.toContain("dmytro");
            // It must still say something useful.
            expect(text.length).toBeGreaterThan(0);
        },
    );

    it("every error kind has non-empty copy that names no path", () => {
        const kinds = new Set(leaky.map(classifyError));
        for (const kind of kinds) {
            const msg = friendlyError(kind);
            expect(msg.trim().length).toBeGreaterThan(0);
            expect(msg).not.toMatch(/\/Users\/|\/home\//);
        }
    });
});

describe("row height reserves the full preview cap", () => {
    // Manifest 06: the "smarter" character-count estimate was itself the bug —
    // it under-measured, so rows overlapped and the scroll position drifted.
    // The reservation must be an upper bound, independent of content.
    it("is large enough for the capped number of preview lines", () => {
        expect(ROW_HEIGHT).toBeGreaterThanOrEqual(
            SINGLE_LINE_FLOOR + (PREVIEW_LINES - 1) * TITLE_LINE_PX,
        );
    });

    it("does not vary with content length", () => {
        // The exported height is a constant by construction; assert that it is
        // not a function of any item, so no future edit can make it adaptive
        // without failing here.
        expect(typeof ROW_HEIGHT).toBe("number");
        expect(Number.isFinite(ROW_HEIGHT)).toBe(true);
        expect(ROW_HEIGHT).toBeGreaterThan(0);
    });
});
