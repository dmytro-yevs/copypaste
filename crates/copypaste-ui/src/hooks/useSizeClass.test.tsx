/**
 * DMY-154: the shell read the user agent and never the width. The assertion
 * that matters is the pair — one width, two user agents, one answer — because
 * that is the one an OS-identity implementation fails while every
 * single-platform test it has still passes.
 */
import { afterEach, describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";

import { EXPANDED_MIN_PX, useSizeClass } from "@/hooks/useSizeClass";
import { resetViewportWidth, setViewportWidth } from "@/test/viewport";

const ANDROID = "Mozilla/5.0 (Linux; Android 15; Pixel Tablet) AppleWebKit/537.36";
const MACOS = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15";
const WINDOWS = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36";
const realUserAgent = navigator.userAgent;

function SizeClassProbe() {
  return <p data-testid="size-class">{useSizeClass()}</p>;
}

function classAt(width: number, userAgent = realUserAgent): string {
  Object.defineProperty(navigator, "userAgent", { configurable: true, value: userAgent });
  setViewportWidth(width);
  const view = render(<SizeClassProbe />);
  const value = screen.getByTestId("size-class").textContent!;
  view.unmount();
  return value;
}

afterEach(() => {
  Object.defineProperty(navigator, "userAgent", {
    configurable: true,
    value: realUserAgent,
  });
  resetViewportWidth();
});

describe("the size class", () => {
  it.each([
    [360, "compact"],
    [411, "compact"],
    [EXPANDED_MIN_PX - 1, "compact"],
    [EXPANDED_MIN_PX, "expanded"],
    [891, "expanded"],
    [1024, "expanded"],
  ])("is %s px wide → %s", (width, expected) => {
    expect(classAt(width)).toBe(expected);
  });

  it.each([
    ["a phone in portrait", 411],
    ["a tablet in landscape", 891],
  ])("reads %s the same on every platform", (_name, width) => {
    const answers = [ANDROID, MACOS, WINDOWS].map((agent) => classAt(width, agent));
    expect(new Set(answers).size).toBe(1);
  });

  it("gives a wide Android window the expanded class", () => {
    expect(classAt(891, ANDROID)).toBe("expanded");
  });

  it("gives a narrow desktop window the compact class", () => {
    expect(classAt(360, MACOS)).toBe("compact");
  });

  it("follows a rotation without remounting", () => {
    setViewportWidth(411);
    render(<SizeClassProbe />);
    expect(screen.getByTestId("size-class").textContent).toBe("compact");

    setViewportWidth(891);
    expect(screen.getByTestId("size-class").textContent).toBe("expanded");

    setViewportWidth(411);
    expect(screen.getByTestId("size-class").textContent).toBe("compact");
  });
});
