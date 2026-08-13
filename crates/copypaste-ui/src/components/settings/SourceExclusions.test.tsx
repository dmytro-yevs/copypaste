/**
 * The exclusion list is a privacy control, so the failure that matters is the
 * silent one: an entry the user typed, saw accepted, and that matched nothing.
 * Windows names a process `chrome.exe` and the user may type `Chrome.exe`,
 * `chrome`, or a path pasted from Explorer (DMY-158).
 *
 * Default exclusions are DMY-170's; this file only covers what a typed entry
 * becomes and what the user is told about it.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { screen } from "@testing-library/react";

import { SourceExclusions } from "@/components/settings/SourceExclusions";
import * as platform from "@/lib/platform";
import { withUser } from "@/test/harness";

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    listItems: () =>
      Promise.resolve({ items: [], total: 0, skipped_undecryptable: 0, next_cursor: null }),
    getStatus: () => Promise.resolve(null),
    listInstalledSourceApps: () => Promise.resolve([]),
  };
});

function onWindows() {
  vi.spyOn(platform, "isAndroidPlatform").mockReturnValue(false);
  vi.spyOn(platform, "isWindowsPlatform").mockReturnValue(true);
}

function onMac() {
  vi.spyOn(platform, "isAndroidPlatform").mockReturnValue(false);
  vi.spyOn(platform, "isWindowsPlatform").mockReturnValue(false);
}

function exclusions(ids: readonly string[] = []) {
  const onChange = vi.fn();
  return { onChange, ...withUser(<SourceExclusions ids={ids} onChange={onChange} />) };
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("on Windows", () => {
  it("speaks about programs, not bundle identifiers", () => {
    onWindows();
    exclusions();
    expect(screen.getByLabelText("Program name").getAttribute("placeholder")).toBe("chrome.exe");
    expect(screen.getByText(/Task Manager/)).toBeTruthy();
  });

  it.each([
    ["chrome.exe", "chrome.exe"],
    ["Chrome.exe", "Chrome.exe"],
    ["chrome", "chrome"],
    [String.raw`C:\Program Files\Google\Chrome\Application\chrome.exe`, "a pasted path"],
  ])("adds %s as chrome.exe (%s)", async (typed) => {
    onWindows();
    const { user, onChange } = exclusions();
    await user.type(screen.getByLabelText("Program name"), typed);
    await user.click(screen.getByRole("button", { name: "Add app" }));
    expect(onChange).toHaveBeenCalledWith(["chrome.exe"]);
  });

  it("says so when what it stored is not what was typed", async () => {
    onWindows();
    const { user } = exclusions();
    await user.type(screen.getByLabelText("Program name"), "Chrome");
    await user.keyboard("{Enter}");
    expect((await screen.findByRole("status")).textContent).toBe("Added as chrome.exe.");
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("refuses an entry that names no program, and says which field", async () => {
    onWindows();
    const { user, onChange } = exclusions();
    const input = screen.getByLabelText("Program name");
    await user.type(input, ".exe");
    await user.keyboard("{Enter}");

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBe("Enter a program name, such as chrome.exe.");
    expect(input.getAttribute("aria-invalid")).toBe("true");
    expect(input.getAttribute("aria-describedby")).toBe(alert.id);
    expect(onChange).not.toHaveBeenCalled();
  });

  it("recognises a program already excluded under another spelling", async () => {
    onWindows();
    const { user, onChange } = exclusions(["chrome.exe"]);
    await user.type(screen.getByLabelText("Program name"), String.raw`C:\Apps\CHROME.EXE`);
    await user.keyboard("{Enter}");

    expect((await screen.findByRole("alert")).textContent).toBe(
      "That program is already excluded as chrome.exe.",
    );
    expect(onChange).not.toHaveBeenCalled();
  });

  it("clears the message as soon as the entry is edited again", async () => {
    onWindows();
    const { user } = exclusions();
    await user.type(screen.getByLabelText("Program name"), ".exe");
    await user.keyboard("{Enter}");
    expect(await screen.findByRole("alert")).toBeTruthy();

    await user.type(screen.getByLabelText("Program name"), "x");
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.getByLabelText("Program name").hasAttribute("aria-invalid")).toBe(false);
  });

  it("keeps every entry reachable and removable by name", async () => {
    onWindows();
    const { user, onChange } = exclusions(["chrome.exe", "1password.exe"]);
    const remove = screen.getByRole("button", { name: "Remove 1password.exe" });
    await user.click(remove);
    expect(onChange).toHaveBeenCalledWith(["chrome.exe"]);
  });
});

describe("everywhere else", () => {
  it("still asks for a bundle identifier and refuses a bare program name", async () => {
    onMac();
    const { user, onChange } = exclusions();
    const input = screen.getByLabelText("App bundle or package ID");
    expect(input.getAttribute("placeholder")).toBe("com.example.app");

    await user.type(input, "chrome");
    await user.keyboard("{Enter}");
    expect((await screen.findByRole("alert")).textContent).toBe(
      "Enter a bundle or package ID, such as com.example.app.",
    );
    expect(onChange).not.toHaveBeenCalled();
  });

  /** macOS compares the identifier exactly, so lowercasing one here would be a
   *  new exclusion that matches nothing rather than a normalised one. */
  it("stores a bundle identifier exactly as typed", async () => {
    onMac();
    const { user, onChange } = exclusions();
    await user.type(screen.getByLabelText("App bundle or package ID"), "com.apple.Passwords");
    await user.keyboard("{Enter}");
    expect(onChange).toHaveBeenCalledWith(["com.apple.Passwords"]);
    expect(screen.queryByRole("status")).toBeNull();
  });
});
