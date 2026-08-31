import { describe, expect, it } from "vitest";

import { item } from "@/test/harness";
import { originName, originOf, originsOf } from "./itemOrigin";

describe("item origins", () => {
  it("keeps an origin generic when the item DTO lacks device metadata", () => {
    const origin = originOf(item({ origin_device_name: "Laptop phone" }));

    expect(origin).toMatchObject({
      id: "device-1",
      name: "Laptop phone",
      kind: "unknown",
    });
  });

  it("uses the opaque id for an unnamed generic origin", () => {
    const origin = originOf(item({ origin_device_id: "12345678-0000", origin_device_name: null }));

    expect(origin?.kind).toBe("unknown");
    expect(origin && originName(origin)).toBe("12345678");
    expect(originsOf([item({ origin_device_name: "Android laptop" })])[0]?.kind).toBe("unknown");
  });
});
