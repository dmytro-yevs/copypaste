import type { AndroidApp } from "./app.js";
import { tapButton, waitFor } from "./ui.js";

export const SETTINGS_NAVIGATION = '[aria-label="Preference sections"]';
export const SETTINGS_BACK = 'button[aria-label="Back to Preferences"]';
export const SETTINGS_PANEL = 'section[aria-label], [role="tabpanel"][aria-label]';

export interface SettingsViewSnapshot {
  navigation: boolean;
  back: boolean;
  visiblePanels: readonly string[];
  busyPanels: readonly string[];
  scrollTop: number | null;
}

export type SettingsViewLevel = "navigation" | "detail" | "neither";

export function settingsViewLevel(snapshot: SettingsViewSnapshot): SettingsViewLevel {
  if (snapshot.navigation) return "navigation";
  if (snapshot.back && snapshot.visiblePanels.length > 0) return "detail";
  return "neither";
}

export function settingsNavigationReady(
  snapshot: SettingsViewSnapshot,
  afterBack: boolean,
): boolean {
  return settingsViewLevel(snapshot) === "navigation" &&
    (!afterBack || (snapshot.scrollTop !== null && snapshot.scrollTop <= 1));
}

export function settingsPanelReady(
  snapshot: SettingsViewSnapshot,
  label: string,
): boolean {
  return snapshot.visiblePanels.includes(label) &&
    !snapshot.busyPanels.includes(label);
}

export interface SettingsTriggerSnapshot {
  exists: boolean;
  disabled: boolean;
  ariaDisabled: string | null;
  width: number;
  height: number;
  top: number;
  viewportTop: number;
  viewportHeight: number;
  clipped: number;
  documentOverflow: number;
  centerInsideViewport: boolean;
  centerHit: boolean;
}

export type SettingsTriggerDecision = "missing" | "blocked" | "scroll" | "tap";

export function settingsTriggerDecision(
  trigger: SettingsTriggerSnapshot,
): SettingsTriggerDecision {
  if (!trigger.exists) return "missing";
  if (trigger.disabled || trigger.ariaDisabled === "true") return "blocked";
  if (
    trigger.width <= 0 ||
    trigger.height <= 0 ||
    !trigger.centerInsideViewport ||
    !trigger.centerHit
  ) {
    return "scroll";
  }
  return "tap";
}

export function settingsScrollDelta(trigger: SettingsTriggerSnapshot): number {
  return trigger.top + trigger.height / 2 -
    (trigger.viewportTop + trigger.viewportHeight / 2);
}

async function settingsView(app: AndroidApp): Promise<SettingsViewSnapshot> {
  return app.withPage((page) =>
    page.evaluate(
      (navigationSelector, backSelector, panelSelector) => {
        const navigation = document.querySelector(navigationSelector);
        const back = document.querySelector(backSelector);
        const anchor = navigation ?? back;
        let viewport = anchor?.parentElement ?? null;
        while (viewport && !/(auto|scroll)/.test(getComputedStyle(viewport).overflowY)) {
          viewport = viewport.parentElement;
        }
        const visiblePanels = Array.from(
          document.querySelectorAll<HTMLElement>(panelSelector),
        ).filter((panel) => panel.getClientRects().length > 0);
        return {
          navigation: navigation !== null,
          back: back !== null,
          visiblePanels: visiblePanels.flatMap(
            (panel) => panel.getAttribute("aria-label") ?? [],
          ),
          busyPanels: visiblePanels
            .filter((panel) => panel.querySelector("[aria-busy]") !== null)
            .flatMap((panel) => panel.getAttribute("aria-label") ?? []),
          scrollTop: viewport?.scrollTop ?? null,
        };
      },
      SETTINGS_NAVIGATION,
      SETTINGS_BACK,
      SETTINGS_PANEL,
    ),
  );
}

export async function ensureSettingsNavigation(app: AndroidApp): Promise<void> {
  let backRequested = false;
  await waitFor(
    async () => {
      const snapshot = await settingsView(app);
      const level = settingsViewLevel(snapshot);
      if (settingsNavigationReady(snapshot, backRequested)) return true;
      if (level === "detail" && !backRequested) {
        backRequested = true;
        await tapButton(app, "Back to Preferences");
      }
      return false;
    },
    "the Preferences navigation never became available",
    60_000,
  );
}

interface TriggerPoint extends SettingsTriggerSnapshot {
  x: number;
  y: number;
}

async function settingsTrigger(
  app: AndroidApp,
  label: string,
): Promise<TriggerPoint> {
  return app.withPage((page) =>
    page.evaluate((selector, name) => {
      const navigation = document.querySelector(selector);
      const missing = {
        exists: false,
        disabled: false,
        ariaDisabled: null,
        width: 0,
        height: 0,
        top: 0,
        viewportTop: 0,
        viewportHeight: 0,
        clipped: 0,
        documentOverflow: 0,
        centerInsideViewport: false,
        centerHit: false,
        x: 0,
        y: 0,
      };
      if (!navigation) return missing;
      const trigger = Array.from(
        navigation.querySelectorAll<HTMLButtonElement>("button"),
      ).find((button) => {
        const heading = button.querySelector("strong")?.textContent;
        return (heading ?? button.textContent ?? "").trim() === name;
      });
      if (!trigger) return missing;

      let viewport = navigation.parentElement;
      while (viewport && !/(auto|scroll)/.test(getComputedStyle(viewport).overflowY)) {
        viewport = viewport.parentElement;
      }
      const rect = trigger.getBoundingClientRect();
      const view = viewport?.getBoundingClientRect() ?? {
        left: 0,
        right: innerWidth,
        top: 0,
        bottom: innerHeight,
      };
      const x = rect.x + rect.width / 2;
      const y = rect.y + rect.height / 2;
      const centerInsideViewport =
        x >= view.left && x <= view.right && y >= view.top && y <= view.bottom;
      return {
        exists: true,
        disabled: trigger.disabled,
        ariaDisabled: trigger.getAttribute("aria-disabled"),
        width: rect.width,
        height: rect.height,
        top: rect.top,
        viewportTop: view.top,
        viewportHeight: view.bottom - view.top,
        clipped: trigger.scrollWidth - trigger.clientWidth,
        documentOverflow:
          document.documentElement.scrollWidth - document.documentElement.clientWidth,
        centerInsideViewport,
        centerHit:
          centerInsideViewport && trigger.contains(document.elementFromPoint(x, y)),
        x,
        y,
      };
    }, SETTINGS_NAVIGATION, label),
  );
}

async function scrollSettingsTrigger(app: AndroidApp, delta: number): Promise<void> {
  await app.withPage((page) =>
    page.evaluate((selector, scrollDelta) => {
      const navigation = document.querySelector(selector);
      if (!navigation) return;
      let viewport = navigation.parentElement;
      while (viewport && !/(auto|scroll)/.test(getComputedStyle(viewport).overflowY)) {
        viewport = viewport.parentElement;
      }
      if (!viewport) return;
      viewport.scrollTop += scrollDelta;
      viewport.dispatchEvent(new Event("scroll", { bubbles: true }));
    }, SETTINGS_NAVIGATION, delta),
  );
}

export interface SettingsSectionGeometry {
  width: number;
  height: number;
  clipped: number;
  documentOverflow: number;
  centerHit: boolean;
}

export async function settingsSectionGeometry(
  app: AndroidApp,
  label: string,
): Promise<SettingsSectionGeometry> {
  await ensureSettingsNavigation(app);
  let observed: TriggerPoint | null = null;
  await waitFor(
    async () => {
      observed = await settingsTrigger(app, label);
      const decision = settingsTriggerDecision(observed);
      if (decision === "tap") return true;
      if (decision === "scroll") {
        await scrollSettingsTrigger(app, settingsScrollDelta(observed));
      }
      return false;
    },
    () => `the ${label} Preferences category never became actionable: ${JSON.stringify(observed)}`,
    15_000,
  );
  return {
    width: observed!.width,
    height: observed!.height,
    clipped: observed!.clipped,
    documentOverflow: observed!.documentOverflow,
    centerHit: observed!.centerHit,
  };
}

export async function settingsSectionLabels(app: AndroidApp): Promise<string[]> {
  await ensureSettingsNavigation(app);
  return app.withPage((page) =>
    page.evaluate((selector) => {
      const navigation = document.querySelector(selector);
      if (!navigation) return [];
      return Array.from(navigation.querySelectorAll("button"), (button) => {
        const heading = button.querySelector("strong")?.textContent;
        return (heading ?? button.textContent ?? "").trim();
      });
    }, SETTINGS_NAVIGATION),
  );
}

export async function openSettingsSection(
  app: AndroidApp,
  label: string,
): Promise<void> {
  await settingsSectionGeometry(app, label);
  const point = await settingsTrigger(app, label);
  if (settingsTriggerDecision(point) !== "tap") {
    throw new Error(`the ${label} Preferences category stopped being actionable`);
  }
  await app.withPage((page) => page.mouse.click(point.x, point.y));

  await waitFor(
    async () => settingsPanelReady(await settingsView(app), label),
    `the ${label} Preferences section never finished opening`,
    30_000,
  );
}

export async function settingsPanel(
  app: AndroidApp,
  label: string,
): Promise<{ width: number; height: number; text: string } | null> {
  return app.withPage((page) =>
    page.evaluate(
      (selector, name) => {
        const panel = Array.from(
          document.querySelectorAll<HTMLElement>(selector),
        ).find(
          (candidate) =>
            candidate.getAttribute("aria-label") === name &&
            candidate.getClientRects().length > 0,
        );
        if (!panel) return null;
        const rect = panel.getBoundingClientRect();
        return { width: rect.width, height: rect.height, text: panel.innerText.trim() };
      },
      SETTINGS_PANEL,
      label,
    ),
  );
}
