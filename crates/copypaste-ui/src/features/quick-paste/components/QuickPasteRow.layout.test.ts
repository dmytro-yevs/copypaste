import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const tokens = readFileSync(resolve(process.cwd(), "../../design/dist/css/tokens.base.css"), "utf8");

function token(name: string): number {
  const match = tokens.match(new RegExp(`--${name}:\\s*(\\d+)px;`));
  if (!match) throw new Error(`Missing ${name} token`);
  return Number(match[1]);
}

describe("Quick Paste image row geometry", () => {
  it("keeps an image clear of the shortcut and pin at the 403px native width", () => {
    const width = 403;
    const rowPadding = token("s-3");
    const sourceIcon = token("icon-md");
    const rowGap = token("s-2");
    const actionColumn = token("ctl-h-sm") + token("s-7");
    const imageCap = token("s-9") * 6;
    const shortcutReserve = token("ctl-h-sm") + token("s-3");
    const shortcutWidth = token("fs-xs") + token("s-2");

    const imageStart = rowPadding + sourceIcon + rowGap;
    const imageWidth = Math.min(
      imageCap,
      width - rowPadding * 2 - sourceIcon - rowGap - actionColumn,
    );
    const shortcutStart = width - shortcutReserve - shortcutWidth;

    expect(imageStart + imageWidth).toBeLessThanOrEqual(shortcutStart);
  });
});
