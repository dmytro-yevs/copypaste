/**
 * The Windows half of this table is the same table as
 * `windows_attribution::tests::every_way_a_user_writes_one_process_names_it`.
 * Two implementations of one rule, so they are asserted against the same cases:
 * a spelling this file accepts and the daemon does not is an exclusion that
 * reads as saved and never fires (DMY-158).
 */
import { describe, expect, it } from "vitest";

import { canonicalExclusion, exclusionKey, findExclusion } from "@/lib/exclusions";

describe("a Windows exclusion entry", () => {
  it.each([
    "chrome.exe",
    "Chrome.exe",
    "CHROME.EXE",
    "chrome",
    "Chrome",
    "  chrome.exe  ",
    String.raw`C:\Program Files\Google\Chrome\Application\chrome.exe`,
    String.raw`"C:\Program Files\Google\Chrome\Application\chrome.exe"`,
    "C:/Program Files/Google/Chrome/Application/chrome.exe",
  ])("names one program however it is written: %s", (entry) => {
    expect(exclusionKey(entry, true)).toBe("chrome");
    expect(canonicalExclusion(entry, true)).toBe("chrome.exe");
  });

  // A trailing separator names a directory, not a program. Written with an
  // escape because a raw template literal cannot end in a backslash.
  it.each(["", "   ", ".exe", '"  "', "C:\\Program Files\\"])(
    "names no program at all: %s",
    (entry) => {
      expect(exclusionKey(entry, true)).toBeNull();
      expect(canonicalExclusion(entry, true)).toBeNull();
    },
  );

  it("does not confuse neighbours", () => {
    expect(exclusionKey("chromium.exe", true)).toBe("chromium");
    expect(exclusionKey("chrome.exe.bak", true)).toBe("chrome.exe.bak");
  });

  /** The stored spelling is the one the item carries, so the list and the row
   *  it is about read the same. */
  it("stores the spelling Task Manager shows", () => {
    expect(canonicalExclusion("chrome", true)).toBe("chrome.exe");
    expect(canonicalExclusion(String.raw`C:\Windows\notepad.exe`, true)).toBe("notepad.exe");
  });

  it("finds an existing entry in a different spelling", () => {
    const ids = ["chrome.exe", "1password.exe"];
    expect(findExclusion(ids, "Chrome", true)).toBe("chrome.exe");
    expect(findExclusion(ids, String.raw`C:\Apps\1Password.exe`, true)).toBe("1password.exe");
    expect(findExclusion(ids, "firefox.exe", true)).toBeUndefined();
    expect(findExclusion(ids, ".exe", true)).toBeUndefined();
  });
});

describe("a bundle identifier", () => {
  it("is unchanged and case-sensitive, because that is what macOS compares", () => {
    expect(canonicalExclusion("com.apple.Passwords", false)).toBe("com.apple.Passwords");
    expect(exclusionKey("  com.apple.Passwords  ", false)).toBe("com.apple.Passwords");
    expect(findExclusion(["com.apple.Passwords"], "com.apple.passwords", false)).toBeUndefined();
  });

  it("still has to look like one", () => {
    for (const entry of ["chrome", "", "   ", "com.example.app!", ".exe"]) {
      expect(exclusionKey(entry, false), entry).toBeNull();
    }
    // A dotted name passes the shape test on every platform; only Windows
    // reduces it to a program, which is why the platform is a parameter here
    // and not a guess made from the string.
    expect(exclusionKey("chrome.exe", false)).toBe("chrome.exe");
  });
});
