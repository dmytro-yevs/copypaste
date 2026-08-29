/**
 * The Devices screen's WebView half on Android.
 *
 * Pairing credentials and the SAS belong to the protected native presenter.
 * This suite therefore proves the WebView offers both ceremonies and reports
 * progress without acquiring a credential surface of its own.
 */
import { afterAll, describe, expect, test } from "vitest";

import { attachToApp, type AndroidApp } from "../src/harness/app.js";
import {
  accessibleSurface,
  expectNoFilesystemPath,
  expectNoRawError,
  outerHtml,
} from "../src/harness/leaks.js";
import {
  allDeviceSectionsSatisfyContracts,
  DEVICE_SECTION_CONTRACTS,
  deviceSectionSatisfiesContract,
  type DeviceSectionSnapshot,
} from "../src/harness/devices.js";
import { beforeAllWithEvidence } from "../src/harness/suite.js";
import {
  count,
  gotoView,
  tapButton,
  tapElement,
  visibleText,
  waitFor,
} from "../src/harness/ui.js";

const HEADER = "header.chrome";
const PAIRING_CODE = /\b[A-Z0-9]{4}(?:-[A-Z0-9]{4}){3}\b/;
const SECURITY_CODE = /\b[0-9A-F]{6}\b/;
const PAIRING_ACTIONS = ["Show pairing code", "Scan pairing code"];

let app: AndroidApp;

async function readDeviceSections(): Promise<DeviceSectionSnapshot[]> {
  return app.withPage((page) =>
    page.evaluate((contracts) => {
      const headingTags = "h1,h2,h3,h4,h5,h6";
      const isRendered = (element: Element): boolean => {
        const style = window.getComputedStyle(element);
        return (
          style.display !== "none" &&
          style.visibility !== "hidden" &&
          element.getAttribute("aria-hidden") !== "true" &&
          element.getClientRects().length > 0
        );
      };
      return contracts.map(({ id }) => {
        const sections = Array.from(document.querySelectorAll("section")).filter(
          (section) => section.getAttribute("aria-labelledby") === id,
        );
        const section = sections.length === 1 ? sections[0] : undefined;
        const headings = section
          ? Array.from(section.querySelectorAll(headingTags)).filter(
              (heading) => heading.id === id,
            )
          : [];
        return {
          sectionCount: sections.length,
          headingCount: headings.length,
          headingText: headings.length === 1 ? headings[0]?.textContent ?? null : null,
          rendered:
            section !== undefined &&
            headings.length === 1 &&
            isRendered(section) &&
            isRendered(headings[0]!),
        } satisfies DeviceSectionSnapshot;
      });
    }, DEVICE_SECTION_CONTRACTS),
  );
}

beforeAllWithEvidence("devices", async () => {
  app = await attachToApp();
  await gotoView(app, "Devices");
  await waitFor(
    async () => {
      const snapshots = await readDeviceSections();
      return allDeviceSectionsSatisfyContracts(snapshots);
    },
    "the Devices screen never settled",
  );
}, 180_000);

afterAll(async () => {
  await app?.detach();
});

async function pairingActionBoxes() {
  return app.withPage((page) =>
    page.evaluate(
      (scope: string, labels: string[]) => {
        const root = document.querySelector(scope);
        return labels.map((label) => {
          const button = Array.from(root?.querySelectorAll("button") ?? []).find(
            (node) => node.getAttribute("aria-label") === label,
          );
          const rect = button?.getBoundingClientRect();
          return {
            label,
            present: Boolean(button),
            width: rect?.width ?? 0,
            height: rect?.height ?? 0,
            right: rect?.right ?? 0,
          };
        });
      },
      '[role="dialog"]',
      PAIRING_ACTIONS,
    ),
  );
}

async function openPairingChoices(): Promise<void> {
  if ((await count(app, '[role="dialog"]')) > 0) return;
  await tapButton(app, "Connect a device", { within: HEADER });
  await waitFor(
    async () => (await count(app, '[role="dialog"]')) === 1,
    "the pairing choices never opened",
  );
}

describe("the screen", () => {
  test("describes each device section through its labelled heading", async () => {
    const snapshots = await readDeviceSections();
    expect(snapshots).toHaveLength(DEVICE_SECTION_CONTRACTS.length);
    for (const [index, contract] of DEVICE_SECTION_CONTRACTS.entries()) {
      const snapshot = snapshots[index]!;
      expect(snapshot.sectionCount, contract.id).toBe(1);
      expect(snapshot.headingCount, contract.id).toBe(1);
      expect(snapshot.headingText, contract.id).toBe(contract.heading);
      expect(deviceSectionSatisfiesContract(snapshot, contract), contract.id).toBe(true);
    }
    expectNoRawError(await outerHtml(app));
    expectNoFilesystemPath(await accessibleSurface(app));
  });

  test("offers both native pairing ceremonies", async () => {
    await openPairingChoices();
    const text = await visibleText(app);
    expect(text).toContain("Show pairing code");
    expect(text).toContain("Scan pairing code");
  });

  test("lays both pairing controls out as reachable touch targets", async () => {
    await openPairingChoices();
    const width = await app.withPage((page) =>
      page.evaluate(() => document.documentElement.clientWidth),
    );
    for (const action of await pairingActionBoxes()) {
      expect(action.present, action.label).toBe(true);
      expect(action.width, action.label).toBeGreaterThan(0);
      expect(action.height, action.label).toBeGreaterThanOrEqual(44);
      expect(action.right, action.label).toBeLessThanOrEqual(width + 1);
    }
  });
});

describe("the native security boundary", () => {
  test("keeps pairing credentials and comparison codes out of the WebView", async () => {
    await openPairingChoices();
    const surface = await app.withPage((page) =>
      page.evaluate((selector: string) => {
        const root = document.querySelector(selector) as HTMLElement | null;
        return {
          text: root?.innerText ?? "",
          credentialNodes:
            root?.querySelectorAll("input, output, canvas, [data-pairing-secret]").length ?? 0,
        };
      }, '[role="dialog"]'),
    );

    expect(surface.text).not.toMatch(PAIRING_CODE);
    expect(surface.text).not.toMatch(SECURITY_CODE);
    expect(surface.credentialNodes).toBe(0);
    await tapElement(app, '[data-slot="dialog-close"]');
  });
});
