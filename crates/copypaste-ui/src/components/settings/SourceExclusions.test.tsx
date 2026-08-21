/**
 * The exclusion list is a privacy control, so the failure that matters is the
 * silent one: an entry the user typed, saw accepted, and that matched nothing.
 * Windows names a process `chrome.exe` and the user may type `Chrome.exe`,
 * `chrome`, or a path pasted from Explorer (DMY-158).
 *
 * Default exclusions are DMY-170's; this file only covers what a typed entry
 * becomes and what the user is told about it.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor } from "@testing-library/react";

import { SourceExclusions } from "@/components/settings/SourceExclusions";
import { en } from "@/i18n";
import * as platform from "@/lib/platform";
import { items, page, withUser } from "@/test/harness";

const listItems = vi.fn();
const listInstalledSourceApps = vi.fn();
const getSourceAppIcon = vi.fn();

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    listItems: (...args: unknown[]) => listItems(...args),
    getStatus: () => Promise.resolve(null),
    listInstalledSourceApps: () => listInstalledSourceApps(),
    getSourceAppIcon: (...args: unknown[]) => getSourceAppIcon(...args),
  };
});

beforeEach(() => {
  listItems.mockReset().mockResolvedValue(
    page(items(3, { source_app_bundle_id: "com.example.editor" })),
  );
  listInstalledSourceApps.mockReset().mockResolvedValue([]);
  getSourceAppIcon.mockReset().mockResolvedValue(null);
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

function onWindows() {
  vi.spyOn(platform, "isAndroidPlatform").mockReturnValue(false);
  vi.spyOn(platform, "isWindowsPlatform").mockReturnValue(true);
}

function onMac() {
  vi.spyOn(platform, "isAndroidPlatform").mockReturnValue(false);
  vi.spyOn(platform, "isWindowsPlatform").mockReturnValue(false);
}

function onAndroid() {
  vi.spyOn(platform, "isAndroidPlatform").mockReturnValue(true);
  vi.spyOn(platform, "isWindowsPlatform").mockReturnValue(false);
}

function exclusions(ids: readonly string[] = []) {
  const onChange = vi.fn();
  return { onChange, ...withUser(<SourceExclusions ids={ids} onChange={onChange} />) };
}

const disclosure = () =>
  screen.getByRole("button", { name: en.settings.service.exclusions.title });

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

describe("on Android", () => {
  it("opens the picker before the installed-app query starts", async () => {
    onAndroid();
    listInstalledSourceApps.mockResolvedValue([
      { package_id: "com.example.first", label: "First" },
    ]);

    withUser(<SourceExclusions ids={[]} onChange={() => {}} />);

    expect(screen.getByLabelText("Search installed apps")).toBeTruthy();
    expect(screen.getByText("Reading the service's settings…")).toBeTruthy();
    expect(listInstalledSourceApps).toHaveBeenCalledTimes(0);

    await waitFor(() => expect(listInstalledSourceApps).toHaveBeenCalledTimes(1));
  });

  it("virtualizes the installed-app list instead of mounting every row", async () => {
    onAndroid();
    listInstalledSourceApps.mockResolvedValue(
      Array.from({ length: 200 }, (_, index) => ({
        package_id: `com.example.${index}`,
        label: `App ${index}`,
      })),
    );

    withUser(<SourceExclusions ids={[]} onChange={() => {}} />);

    expect(await screen.findByText("App 0")).toBeTruthy();
    expect(screen.queryByText("App 199")).toBeNull();
    await waitFor(() => expect(getSourceAppIcon.mock.calls.length).toBeLessThan(200));
  });
});

describe("the collapsed panel", () => {
  /** INV-7: an accessibility pointer may only name a node that is mounted. The
   *  region stays and is hidden; only the editor that owns the history query is
   *  unmounted. */
  it("controls a region that exists while it is collapsed", () => {
    withUser(<SourceExclusions ids={[]} collapsible onChange={() => {}} />);

    const button = disclosure();
    expect(button.getAttribute("aria-expanded")).toBe("false");
    const controls = button.getAttribute("aria-controls");
    expect(controls).toBeTruthy();

    const region = document.getElementById(controls!);
    expect(region).not.toBeNull();
    expect(region).toHaveProperty("hidden", true);
    expect(
      screen.queryByLabelText(en.settings.service.exclusions.inputLabel),
    ).toBeNull();
  });

  it("reads no clipboard history until it is opened", async () => {
    const { user } = withUser(<SourceExclusions ids={[]} collapsible onChange={() => {}} />);

    await new Promise((settle) => setTimeout(settle, 50));
    expect(listItems).toHaveBeenCalledTimes(0);

    await user.click(disclosure());
    await waitFor(() => expect(listItems).toHaveBeenCalled());

    const region = document.getElementById(disclosure().getAttribute("aria-controls")!);
    expect(region).toHaveProperty("hidden", false);
    expect(disclosure().getAttribute("aria-expanded")).toBe("true");
    expect(
      await screen.findByLabelText(en.settings.service.exclusions.inputLabel),
    ).not.toBeNull();
  });

  it("stops reading again once it is closed", async () => {
    const { user } = withUser(<SourceExclusions ids={[]} collapsible onChange={() => {}} />);
    await user.click(disclosure());
    await waitFor(() => expect(listItems).toHaveBeenCalled());

    await user.click(disclosure());
    listItems.mockClear();
    await new Promise((settle) => setTimeout(settle, 50));

    expect(listItems).toHaveBeenCalledTimes(0);
  });
});

describe("the panel that is not collapsible", () => {
  it("is open, and needs no disclosure button", async () => {
    withUser(<SourceExclusions ids={[]} onChange={() => {}} />);

    expect(
      screen.queryByRole("button", { name: en.settings.service.exclusions.title }),
    ).toBeNull();
    expect(
      await screen.findByLabelText(en.settings.service.exclusions.inputLabel),
    ).not.toBeNull();
  });
});
