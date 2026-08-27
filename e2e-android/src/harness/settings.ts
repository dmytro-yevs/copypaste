import type { AndroidApp } from "./app.js";
import { tapButton, waitFor } from "./ui.js";

export const SETTINGS_NAVIGATION = '[aria-label="Preference sections"]';
export const SETTINGS_BACK = 'button[aria-label="Back to Preferences"]';
export const SETTINGS_PANEL = 'section[aria-label], [role="tabpanel"][aria-label]';

export interface SettingsViewSnapshot {
  navigation: boolean;
  back: boolean;
  visiblePanels: readonly string[];
}

export type SettingsViewLevel = "navigation" | "detail" | "neither";

export function settingsViewLevel(snapshot: SettingsViewSnapshot): SettingsViewLevel {
  if (snapshot.navigation) return "navigation";
  if (snapshot.back && snapshot.visiblePanels.length > 0) return "detail";
  return "neither";
}

async function settingsView(app: AndroidApp): Promise<SettingsViewSnapshot> {
  return app.withPage((page) =>
    page.evaluate(
      (navigationSelector, backSelector, panelSelector) => ({
        navigation: document.querySelector(navigationSelector) !== null,
        back: document.querySelector(backSelector) !== null,
        visiblePanels: Array.from(
          document.querySelectorAll<HTMLElement>(panelSelector),
        )
          .filter((panel) => panel.getClientRects().length > 0)
          .flatMap((panel) => panel.getAttribute("aria-label") ?? []),
      }),
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
      const level = settingsViewLevel(await settingsView(app));
      if (level === "navigation") return true;
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
  await ensureSettingsNavigation(app);
  await waitFor(
    () =>
      app.withPage(async (page) => {
        const point = await page.evaluate(
          (selector, name) => {
            const navigation = document.querySelector(selector);
            if (!navigation) return null;
            const trigger = Array.from(
              navigation.querySelectorAll<HTMLButtonElement>("button"),
            ).find((button) => {
              const heading = button.querySelector("strong")?.textContent;
              return (heading ?? button.textContent ?? "").trim() === name;
            });
            if (!trigger || trigger.disabled || trigger.getAttribute("aria-disabled") === "true") {
              return null;
            }
            const rect = trigger.getBoundingClientRect();
            if (rect.width === 0 || rect.height === 0) return null;
            const x = rect.x + rect.width / 2;
            const y = rect.y + rect.height / 2;
            if (!trigger.contains(document.elementFromPoint(x, y))) return null;
            return { x, y };
          },
          SETTINGS_NAVIGATION,
          label,
        );
        if (!point) return false;
        await page.mouse.click(point.x, point.y);
        return true;
      }),
    `no tappable Preferences section labelled ${JSON.stringify(label)}`,
    15_000,
  );

  await waitFor(
    () =>
      app.withPage((page) =>
        page.evaluate(
          (selector, name) =>
            Array.from(document.querySelectorAll<HTMLElement>(selector)).some(
              (panel) =>
                panel.getAttribute("aria-label") === name &&
                panel.getClientRects().length > 0 &&
                panel.querySelector("[aria-busy]") === null,
            ),
          SETTINGS_PANEL,
          label,
        ),
      ),
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
