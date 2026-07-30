import { describe, expect, it } from "vitest";

import { decodePairing, encodePairing } from "@/lib/pairing";

const PAYLOAD = {
  code: "K5PYA3ZC7QW2NBMR6TX4HJVD8LGE9SFU5CPO2AIK7YNQ3ZBW",
  addr: "192.168.1.24:47654",
};

describe("the pairing payload", () => {
  it("round-trips the code and the address together", () => {
    expect(decodePairing(encodePairing(PAYLOAD))).toEqual(PAYLOAD);
  });

  /** One scan is the whole pairing. A QR carrying only the code would leave
   *  the address to be typed and solve the smaller half of the problem. */
  it("carries the address, not only the code", () => {
    expect(encodePairing(PAYLOAD)).toContain(encodeURIComponent(PAYLOAD.addr));
  });

  it("survives a code with characters a URL would otherwise eat", () => {
    const awkward = { code: "a+b/c=d&e?f", addr: "host.local:1" };
    expect(decodePairing(encodePairing(awkward))).toEqual(awkward);
  });

  /**
   * A scanner sees whatever is in frame. Every one of these must decode to
   * `null` rather than to a half-parsed result that reaches `pair_accept`.
   */
  it.each([
    ["a plain URL", "https://example.com/?c=abc&a=1.2.3.4:1"],
    ["someone else's scheme", "otherapp://pair?v=2&c=abc&a=1.2.3.4:1"],
    ["the right scheme, wrong purpose", "copypaste://open?v=2&c=abc&a=1.2.3.4:1"],
    ["a future version", "copypaste://pair?v=3&c=abc&a=1.2.3.4:1"],
    ["no version", "copypaste://pair?c=abc&a=1.2.3.4:1"],
    ["no code", "copypaste://pair?v=2&a=1.2.3.4:1"],
    ["an empty code", "copypaste://pair?v=2&c=&a=1.2.3.4:1"],
    ["no address", "copypaste://pair?v=2&c=abc"],
    ["an address with no port", "copypaste://pair?v=2&c=abc&a=1.2.3.4"],
    ["an address with a path", "copypaste://pair?v=2&c=abc&a=1.2.3.4:1/x"],
    ["not a URL at all", "just some text on a poster"],
    ["empty", ""],
  ])("refuses %s", (_name, scanned) => {
    expect(decodePairing(scanned)).toBeNull();
  });

  it("tolerates the whitespace a decoder can leave on the ends", () => {
    expect(decodePairing(`  ${encodePairing(PAYLOAD)}\n`)).toEqual(PAYLOAD);
  });
});
