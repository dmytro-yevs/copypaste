import { describe, expect, it } from "vitest";

import { allocateDriverPorts } from "./app.js";

describe("app startup ports", () => {
  it("allocates driver and native ports sequentially", async () => {
    const calls: string[] = [];

    const ports = await allocateDriverPorts(async () => {
      calls.push("allocate");
      return 4100 + calls.length;
    });

    expect(ports).toEqual([4101, 4102]);
    expect(calls).toEqual(["allocate", "allocate"]);
  });

  it("does not attempt another allocation after either allocation fails", async () => {
    const calls: string[] = [];
    const failure = new Error("native port allocation failed");

    await expect(
      allocateDriverPorts(async () => {
        calls.push("allocate");
        if (calls.length === 2) throw failure;
        return 4101;
      }),
    ).rejects.toBe(failure);

    expect(calls).toEqual(["allocate", "allocate"]);
  });

  it("does not attempt native allocation after driver allocation fails", async () => {
    const calls: string[] = [];
    const failure = new Error("driver port allocation failed");

    await expect(
      allocateDriverPorts(async () => {
        calls.push("allocate");
        throw failure;
      }),
    ).rejects.toBe(failure);

    expect(calls).toEqual(["allocate"]);
  });
});
