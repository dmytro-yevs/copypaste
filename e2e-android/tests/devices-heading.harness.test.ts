import { describe, expect, test } from "vitest";

import {
  allDeviceSectionsSatisfyContracts,
  DEVICE_SECTION_CONTRACTS,
  deviceSectionSatisfiesContract,
  type DeviceSectionSnapshot,
} from "../src/harness/devices.js";

const ready = (headingText: string): DeviceSectionSnapshot => ({
  sectionCount: 1,
  headingCount: 1,
  headingText,
  rendered: true,
});

describe("Devices section readiness", () => {
  test("accepts canonical textContent when CSS uppercases the rendered heading", () => {
    expect(
      deviceSectionSatisfiesContract(ready("Your devices"), DEVICE_SECTION_CONTRACTS[0]),
    ).toBe(true);
  });

  test.each([
    ["wrong heading", { ...ready("YOUR DEVICES") }],
    ["missing heading", { ...ready("Your devices"), headingCount: 0, headingText: null }],
    ["duplicate heading", { ...ready("Your devices"), headingCount: 2 }],
    ["duplicate section", { ...ready("Your devices"), sectionCount: 2 }],
    ["hidden placeholder", { ...ready("Your devices"), rendered: false }],
  ])("rejects %s", (_name, snapshot) => {
    expect(deviceSectionSatisfiesContract(snapshot, DEVICE_SECTION_CONTRACTS[0])).toBe(false);
  });

  test("requires all three canonical sections", () => {
    expect(
      allDeviceSectionsSatisfyContracts(DEVICE_SECTION_CONTRACTS.map(({ heading }) => ready(heading))),
    ).toBe(true);
    expect(
      allDeviceSectionsSatisfyContracts([
        ready("Your devices"),
        ready("Cloud connection"),
        { ...ready("Discovered on your network"), sectionCount: 2 },
      ]),
    ).toBe(false);
  });
});
