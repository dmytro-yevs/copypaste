import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, screen, waitFor } from "@testing-library/react";

import { QuickPasteApp } from "@/components/quick-paste/QuickPasteApp";
import { en } from "@/i18n";
import { IpcFailure } from "@/lib/errors";
import { rowHeight } from "@/lib/layout";
import { absoluteTime } from "@/lib/format";
import { DEFAULT_PREFS, STORAGE_KEY } from "@/store/prefs";
import { item, page, withUser } from "@/test/harness";

const copyItem = vi.fn();
const copyItemAsPlainText = vi.fn();
const hideWindow = vi.fn();
const listItems = vi.fn();
const getImagePreview = vi.fn();
const setAllowScreenshots = vi.fn();
const setPinned = vi.fn();
const openSettingsFromQuickPaste = vi.fn();
const restartService = vi.fn();
const toastError = vi.fn();

vi.mock("sonner", () => ({
  toast: { error: (...args: unknown[]) => toastError(...args) },
}));

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    copyItem: (...args: unknown[]) => copyItem(...args),
    copyItemAsPlainText: (...args: unknown[]) => copyItemAsPlainText(...args),
    hideWindow: () => hideWindow(),
    listItems: (...args: unknown[]) => listItems(...args),
    getImagePreview: (...args: unknown[]) => getImagePreview(...args),
    setAllowScreenshots: (...args: unknown[]) => setAllowScreenshots(...args),
    setPinned: (...args: unknown[]) => setPinned(...args),
    openSettingsFromQuickPaste: () => openSettingsFromQuickPaste(),
    restartService: () => restartService(),
  };
});

beforeEach(() => {
  window.localStorage.clear();
  copyItem.mockReset().mockResolvedValue(item());
  copyItemAsPlainText.mockReset().mockResolvedValue(item());
  hideWindow.mockReset().mockResolvedValue(undefined);
  listItems.mockReset().mockResolvedValue(page([item()]));
  getImagePreview.mockReset().mockRejectedValue(new Error("not used in this test"));
  setAllowScreenshots.mockReset().mockResolvedValue(undefined);
  setPinned.mockReset().mockResolvedValue(item());
  openSettingsFromQuickPaste.mockReset().mockResolvedValue(undefined);
  restartService.mockReset().mockResolvedValue({ state: "running", version: "2.0.0", matches_app: true, ours: true });
  toastError.mockReset();
});

describe("Quick Paste", () => {
  it("shows an exact local timestamp inline without copying", async () => {
    const clip = item({ created_at: new Date("2026-08-01T20:35:24Z").valueOf() });
    listItems.mockResolvedValue(page([clip]));
    const { user } = withUser(<QuickPasteApp />);

    const time = await screen.findByRole("button", { name: /Activate to show exact time/ });
    await user.click(time);

    expect(time.textContent).toBe(absoluteTime(clip.created_at));
    expect(copyItem).not.toHaveBeenCalled();
  });

  it("clamps result previews to the persisted popup line setting", async () => {
    window.localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({
        state: { ...DEFAULT_PREFS, previewLinesPopup: 5 },
        version: 0,
      }),
    );
    listItems.mockResolvedValue(page([item({ content: "five-line preview" })]));
    withUser(<QuickPasteApp />);

    const preview = await screen.findByText("five-line preview");
    expect(preview.getAttribute("style")).toContain("-webkit-line-clamp: 5");
  });

  it("uses History's card density even when the popup shows fewer text lines", async () => {
    window.localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({
        state: { ...DEFAULT_PREFS, previewLines: 4, previewLinesPopup: 1 },
        version: 0,
      }),
    );
    listItems.mockResolvedValue(page([item({ content: "one visible line" })]));
    withUser(<QuickPasteApp />);

    const row = await screen.findByRole("listitem");
    expect(row.dataset.clipRowDensity).toBe("history");
    expect(row.style.height).toBe(`${rowHeight(4)}px`);
    expect(screen.getByText("one visible line").getAttribute("style")).toContain(
      "-webkit-line-clamp: 1",
    );
  });

  it("copies the selected item and dismisses only after a successful copy", async () => {
    withUser(<QuickPasteApp />);

    await screen.findByRole("listitem", undefined, { timeout: 10_000 });
    fireEvent.keyDown(screen.getByRole("main"), { key: "Enter" });

    await waitFor(() => expect(copyItem).toHaveBeenCalledWith("row-1"));
    await waitFor(() => expect(hideWindow).toHaveBeenCalledTimes(1));
  });

  it("keeps the popup open and offers retry in a floating notification when copying fails", async () => {
    copyItem.mockRejectedValueOnce(new Error("clipboard unavailable"));
    withUser(<QuickPasteApp />);

    await screen.findByRole("listitem");
    fireEvent.keyDown(screen.getByRole("main"), { key: "Enter" });

    await waitFor(() => expect(toastError).toHaveBeenCalledTimes(1));
    expect(toastError.mock.calls[0]?.[0]).toContain("Couldn’t copy");
    expect(hideWindow).not.toHaveBeenCalled();
    await (toastError.mock.calls[0]?.[1] as { action: { onClick: () => void } }).action.onClick();
    await waitFor(() => expect(copyItem).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(hideWindow).toHaveBeenCalledTimes(1));
  });

  it("uses the explicit plain-text backend path for Option+Enter", async () => {
    withUser(<QuickPasteApp />);

    await screen.findByRole("listitem");
    fireEvent.keyDown(screen.getByRole("main"), { key: "Enter", altKey: true });

    await waitFor(() => expect(copyItemAsPlainText).toHaveBeenCalledWith("row-1"));
    expect(copyItem).not.toHaveBeenCalled();
    await waitFor(() => expect(hideWindow).toHaveBeenCalledTimes(1));
  });

  it("retries a failed Option+Enter copy as plain text", async () => {
    copyItemAsPlainText.mockRejectedValueOnce(new Error("clipboard unavailable"));
    withUser(<QuickPasteApp />);

    await screen.findByRole("listitem");
    fireEvent.keyDown(screen.getByRole("main"), { key: "Enter", altKey: true });
    await waitFor(() => expect(toastError).toHaveBeenCalledTimes(1));
    await (toastError.mock.calls[0]?.[1] as { action: { onClick: () => void } }).action.onClick();

    await waitFor(() => expect(copyItemAsPlainText).toHaveBeenCalledTimes(2));
    expect(copyItem).not.toHaveBeenCalled();
  });

  it("exposes the history cap instead of silently showing the first page", async () => {
    listItems.mockResolvedValue(page([item()], 0, "next", 214));
    withUser(<QuickPasteApp />);

    expect((await screen.findByText("1 of 214")).getAttribute("aria-live")).toBe("polite");
  });

  it("opens Settings through the backend without asking the popup to restore focus", async () => {
    const { user } = withUser(<QuickPasteApp />);

    await user.click(screen.getByRole("button", { name: "Open Settings" }));

    await waitFor(() => expect(openSettingsFromQuickPaste).toHaveBeenCalledTimes(1));
    expect(hideWindow).not.toHaveBeenCalled();
  });

  it("keeps the compact footer touch-ready without desktop hotkeys on Android", async () => {
    const userAgent = vi
      .spyOn(window.navigator, "userAgent", "get")
      .mockReturnValue("CopyPaste Android");
    withUser(<QuickPasteApp />);

    const settings = screen.getByRole("button", { name: "Open Settings" });
    expect(settings.className).toContain("size-[var(--sz-iconbtn)]");
    expect(screen.queryByText(/⌘1–9 quick paste/)).toBeNull();
    userAgent.mockRestore();
  });

  it("releases the warm popup cache when native hide invokes its memory hook", async () => {
    withUser(<QuickPasteApp />);

    await screen.findByRole("listitem");
    expect(window.__copypasteFreeMemory).toEqual(expect.any(Function));
    window.__copypasteFreeMemory?.();

    await waitFor(() => expect(screen.queryByRole("listitem")).toBeNull());
  });

  it("ignores a delayed response after a newer focus refresh", async () => {
    let resolveMount: ((value: ReturnType<typeof page>) => void) | undefined;
    let resolveFocus: ((value: ReturnType<typeof page>) => void) | undefined;
    const mount = new Promise<ReturnType<typeof page>>((resolve) => {
      resolveMount = resolve;
    });
    const focus = new Promise<ReturnType<typeof page>>((resolve) => {
      resolveFocus = resolve;
    });
    listItems.mockReset().mockReturnValueOnce(mount).mockReturnValueOnce(focus);

    withUser(<QuickPasteApp />);
    window.dispatchEvent(new Event("focus"));
    resolveFocus?.(page([item({ content: "newer response" })]));

    expect(await screen.findByText("newer response")).not.toBeNull();
    resolveMount?.(page([item({ content: "stale response" })]));

    await waitFor(() => expect(screen.queryByText("stale response")).toBeNull());
    expect(screen.getByText("newer response")).not.toBeNull();
  });

  it("uses the compact surface’s wrapping keyboard selection", async () => {
    listItems.mockResolvedValue(page([item({ id: "first" }), item({ id: "last" })]));
    withUser(<QuickPasteApp />);

    await screen.findAllByRole("listitem");
    fireEvent.keyDown(screen.getByRole("main"), { key: "ArrowUp" });
    fireEvent.keyDown(screen.getByRole("main"), { key: "Enter" });

    await waitFor(() => expect(copyItem).toHaveBeenCalledWith("last"));
  });

  it("scrolls a keyboard-selected row into view", async () => {
    listItems.mockResolvedValue(page([
      item({ id: "first", content: "first entry" }),
      item({ id: "second", content: "second entry" }),
    ]));
    withUser(<QuickPasteApp />);

    const rows = await screen.findAllByRole("listitem");
    await waitFor(() => expect(rows[0]?.getAttribute("aria-current")).toBe("true"));
    const scrollIntoView = vi.fn();
    Object.defineProperty(rows[1], "scrollIntoView", { configurable: true, value: scrollIntoView });

    fireEvent.keyDown(screen.getByRole("main"), { key: "ArrowDown" });

    await waitFor(() => expect(rows[1]?.getAttribute("aria-current")).toBe("true"));
    expect(scrollIntoView).toHaveBeenCalledWith({ block: "nearest" });
  });

  it("fuzzy-matches subsequences and ranks stronger matches first", async () => {
    listItems.mockResolvedValue(page([
      item({ id: "later", content: "notes about copy" }),
      item({ id: "prefix", content: "copy reference" }),
      item({ id: "subsequence", content: "Copy Paste entry" }),
    ]));
    const { user } = withUser(<QuickPasteApp />);
    const search = screen.getByRole("textbox", { name: "Search clipboard history" });

    await screen.findAllByRole("listitem");
    await user.type(search, "copy");
    expect(screen.getAllByRole("listitem").map((row) => row.textContent)).toEqual([
      expect.stringContaining("copy reference"),
      expect.stringContaining("Copy Paste entry"),
      expect.stringContaining("notes about copy"),
    ]);

    await user.clear(search);
    await user.type(search, "cpt");
    expect(screen.getAllByRole("listitem").map((row) => row.textContent)).toEqual([
      expect.stringContaining("Copy Paste entry"),
    ]);
  });

  it("redacts a potential-sensitive row while keeping its original item actionable", async () => {
    const plaintext = "Send the report to alice@example.com today";
    const redacted = "Send the report to ***REDACTED*** today";
    listItems.mockResolvedValue(page([item({
      id: "potential-sensitive",
      content: plaintext,
      sensitive_finding: {
        label: "email",
        spans: [{ start: 19, end: 36 }],
        spans_truncated: false,
        redacted_preview: redacted,
      },
    })]));
    const { user } = withUser(<QuickPasteApp />);

    expect(await screen.findByText(redacted)).not.toBeNull();
    expect(screen.getByRole("img", { name: en.quickPaste.row.potentialSensitive })).not.toBeNull();
    expect(screen.getByRole("img", { name: "Text" })).not.toBeNull();
    expect(screen.queryByText(en.quickPaste.row.sensitive)).toBeNull();
    expect(document.body.textContent).not.toContain(plaintext);
    expect(document.body.innerHTML).not.toContain("alice@example.com");
    expect(screen.queryByRole("button", { name: /alice@example\.com/ })).toBeNull();

    await user.click(screen.getByRole("button", { name: `Copy ${redacted}` }));
    await waitFor(() => expect(copyItem).toHaveBeenCalledWith("potential-sensitive"));
  });

  it("fuzzy-matches potential-sensitive items only against their redacted previews", async () => {
    listItems.mockResolvedValue(page([
      item({
        id: "later",
        content: "alice@example.com",
        sensitive_finding: {
          label: "email",
          spans: [{ start: 0, end: 17 }],
          spans_truncated: false,
          redacted_preview: "notes about contact ***REDACTED***",
        },
      }),
      item({
        id: "prefix",
        content: "bob@example.com",
        sensitive_finding: {
          label: "email",
          spans: [{ start: 0, end: 15 }],
          spans_truncated: false,
          redacted_preview: "contact ***REDACTED***",
        },
      }),
    ]));
    const { user } = withUser(<QuickPasteApp />);
    const search = screen.getByRole("textbox", { name: en.quickPaste.search.label });

    await screen.findAllByRole("listitem");
    await user.type(search, "contact");
    expect(screen.getAllByRole("listitem").map((row) => row.textContent)).toEqual([
      expect.stringContaining("contact ***REDACTED***"),
      expect.stringContaining("notes about contact ***REDACTED***"),
    ]);

    await user.clear(search);
    await user.type(search, "alice");
    expect(await screen.findByText("No matches for “alice”")).not.toBeNull();
    expect(screen.queryByRole("listitem")).toBeNull();
  });

  it("renders safe labels, type icons, and the copied time in popup metadata", async () => {
    const secret = "sk_live_SUPER_PRIVATE";
    listItems.mockResolvedValue(page([
      item({ id: "secret", is_sensitive: true, content: secret }),
      item({
        id: "image",
        content: "opaque image bytes",
        content_type: "image/png",
        source_app_bundle_id: "com.google.Chrome",
        created_at: Date.now() - 120_000,
      }),
      item({ id: "file", content: "[file]", content_type: "file" }),
    ]));
    const { user } = withUser(<QuickPasteApp />);

    await screen.findByText("Sensitive content");
    expect(screen.getByRole("img", { name: "Image" })).not.toBeNull();
    expect(screen.getByRole("img", { name: "File" })).not.toBeNull();
    expect(screen.getByText("Google Chrome")).toBeTruthy();
    expect(screen.getAllByText("2m").length).toBeGreaterThan(0);
    expect(screen.getByTitle("Image")).not.toBeNull();
    expect(document.body.textContent).not.toContain(secret);
    expect(document.body.textContent).not.toContain("opaque image bytes");

    await user.type(screen.getByRole("textbox", { name: "Search clipboard history" }), "live");
    expect(await screen.findByText("No matches for “live”")).not.toBeNull();
    expect(screen.queryByRole("listitem")).toBeNull();
  });

  it("offers Pin and Unpin row controls", async () => {
    listItems.mockResolvedValue(page([
      item({ id: "loose", content: "loose", pinned: false }),
      item({ id: "fixed", content: "fixed", pinned: true }),
    ]));
    const { user } = withUser(<QuickPasteApp />);

    await screen.findAllByRole("listitem");
    await user.click(screen.getByRole("button", { name: "Pin" }));
    await waitFor(() => expect(setPinned).toHaveBeenCalledWith("loose", true));
    await user.click(screen.getByRole("button", { name: "Unpin" }));
    await waitFor(() => expect(setPinned).toHaveBeenCalledWith("fixed", false));
  });

  it("keeps a failed pin actionable in a floating notification", async () => {
    setPinned.mockRejectedValueOnce(new Error("socket /Users/alice/private.sock"));
    const { user } = withUser(<QuickPasteApp />);

    await screen.findByRole("listitem");
    await user.click(screen.getByRole("button", { name: "Pin" }));

    await waitFor(() => expect(toastError).toHaveBeenCalledTimes(1));
    expect(toastError.mock.calls[0]?.[0]).toContain("Couldn’t pin");
    expect(document.body.textContent).not.toContain("/Users/alice/private.sock");
    expect(hideWindow).not.toHaveBeenCalled();
    await (toastError.mock.calls[0]?.[1] as { action: { onClick: () => void } }).action.onClick();
    await waitFor(() => expect(setPinned).toHaveBeenCalledTimes(2));
    expect(setPinned).toHaveBeenNthCalledWith(2, "row-1", true);
  });

  it("keeps each Cmd quick-paste hint beside its matching pin action", async () => {
    const { user } = withUser(<QuickPasteApp />);

    await screen.findByRole("listitem");
    expect(screen.getByText("⌘1")).not.toBeNull();
    expect(screen.queryByText(/↑↓ navigate/)).toBeNull();
    await user.type(screen.getByRole("textbox", { name: "Search clipboard history" }), "entry");
    expect(screen.queryByText("⌘1")).toBeNull();
    fireEvent.keyDown(screen.getByRole("main"), { key: "1", metaKey: true });
    expect(copyItem).not.toHaveBeenCalled();

    await user.clear(screen.getByRole("textbox", { name: "Search clipboard history" }));
    fireEvent.keyDown(screen.getByRole("main"), { key: "1", metaKey: true });
    await waitFor(() => expect(copyItem).toHaveBeenCalledWith("row-1"));
  });

  it("names the query in the no-match state", async () => {
    const { user } = withUser(<QuickPasteApp />);

    await screen.findByRole("listitem");
    await user.type(screen.getByRole("textbox", { name: "Search clipboard history" }), "missing");

    expect(await screen.findByText("No matches for “missing”")).not.toBeNull();
  });

  it("offers Restart only when the clipboard service is offline", async () => {
    listItems.mockRejectedValue(new IpcFailure("offline", true));
    const { user } = withUser(<QuickPasteApp />);

    expect(await screen.findByText("The clipboard service isn't running")).not.toBeNull();
    await user.click(screen.getByRole("button", { name: "Restart service" }));
    await waitFor(() => expect(restartService).toHaveBeenCalledTimes(1));
  });

  it("shows startup without a misleading recovery action", async () => {
    listItems.mockRejectedValue(new IpcFailure("not_ready", true));
    withUser(<QuickPasteApp />);

    expect(await screen.findByText("Starting clipboard service")).not.toBeNull();
    expect(screen.queryByRole("button", { name: /restart|try again/i })).toBeNull();
  });

  it("offers retry for an unexpected history failure", async () => {
    listItems.mockRejectedValueOnce(new Error("unexpected")).mockResolvedValueOnce(page([item()]));
    const { user } = withUser(<QuickPasteApp />);

    const title = await screen.findByText("Couldn't load clipboard history");
    expect(title.closest("[data-slot='empty-state']")).not.toBeNull();
    await user.click(screen.getByRole("button", { name: "Try again" }));
    await screen.findByRole("listitem");
    expect(listItems).toHaveBeenCalledTimes(2);
  });

  it("closes only when focus leaves the popup root and debounces the blur", async () => {
    withUser(<QuickPasteApp />);
    const root = screen.getByRole("main");
    const settings = screen.getByRole("button", { name: "Open Settings" });

    fireEvent.blur(root, { relatedTarget: settings });
    expect(hideWindow).not.toHaveBeenCalled();

    fireEvent.blur(root, { relatedTarget: document.body });
    fireEvent.blur(root, { relatedTarget: document.body });
    await waitFor(() => expect(hideWindow).toHaveBeenCalledTimes(1));
  });

  it("keeps the popup open while a transient retry action receives focus", async () => {
    withUser(<QuickPasteApp />);
    const root = screen.getByRole("main");
    const toaster = document.createElement("div");
    toaster.dataset.sonnerToaster = "";
    const retry = document.createElement("button");
    toaster.append(retry);
    document.body.append(toaster);

    fireEvent.blur(root, { relatedTarget: retry });
    expect(hideWindow).not.toHaveBeenCalled();

    toaster.remove();
  });
});

/**
 * The popup used to author its own English. Every sentence it can show is now
 * the catalogue's, which is what subjects it to the INV-12 path rules and the
 * plural pairing that `catalogue.test.ts` enforces.
 */
describe("the popup reads its text from the catalogue", () => {
  it("names the surface, the field and the settings affordance from it", async () => {
    withUser(<QuickPasteApp />);

    expect(screen.getByRole("main", { name: en.quickPaste.title })).not.toBeNull();
    expect(
      screen.getByRole("textbox", { name: en.quickPaste.search.label }),
    ).not.toBeNull();
    expect(screen.getByRole("button", { name: en.quickPaste.settings })).not.toBeNull();
    await screen.findByRole("listitem");
  });

  /** One item read "1 items" while the count was assembled by hand. */
  it("counts a single item in the singular", async () => {
    listItems.mockResolvedValue(page([item()]));
    withUser(<QuickPasteApp />);

    expect(await screen.findByText("1 item")).not.toBeNull();
  });

  it("puts the user's own search text in the no-results title, and nothing else", async () => {
    const { user } = withUser(<QuickPasteApp />);
    await screen.findByRole("listitem");

    await user.type(
      screen.getByRole("textbox", { name: en.quickPaste.search.label }),
      "nothingmatchesthis",
    );

    expect(
      await screen.findByText(
        en.quickPaste.noResults.title.replace("{{query}}", "nothingmatchesthis"),
      ),
    ).not.toBeNull();
    expect(screen.getByText(en.quickPaste.noResults.body)).not.toBeNull();
  });

  it("takes the offline and pin failures from it rather than authoring them", async () => {
    listItems.mockRejectedValue(new IpcFailure("offline", true));
    withUser(<QuickPasteApp />);

    expect(await screen.findByText(en.quickPaste.offline.title)).not.toBeNull();
    expect(
      screen.getByRole("button", { name: en.quickPaste.offline.action }),
    ).not.toBeNull();
  });
});
