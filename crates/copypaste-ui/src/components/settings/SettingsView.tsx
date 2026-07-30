/**
 * The Settings screen.
 *
 * Sub-navigation is Radix Tabs, which is where A11Y-6 comes from: `role
 * ="tablist"` / `role="tab"` / `aria-selected`, panes wired with
 * `id`/`aria-labelledby`, arrow keys moving selection, Home/End jumping to the
 * bounds, and wrap-around at both ends. v1 hand-wrote a `tabListKeyDown`
 * factory for that (manifest §9.1 says replace it).
 *
 * A11Y-15: the tab row wraps at the 720px minimum rather than scrolling
 * invisibly — the defect behind CopyPaste-g27b.31 was "Logs" being entirely
 * off-screen with no indication it existed.
 *
 * Settings works while the service is down: everything on it is client-owned
 * except the About pane, which says so itself.
 *
 * **There is deliberately no Shortcuts tab yet**, because the global hotkey has
 * a constraint a recorder must respect and the bridge does not expose one to
 * bind against. When it does, whoever builds the recorder needs three things
 * from manifest §7.3 and one from ADR-0002:
 *
 *   - capture from `KeyboardEvent.code`, never `.key`, so a Cyrillic or AZERTY
 *     layout records the same physical binding (INV-23);
 *   - require at least one modifier, ignore bare modifier keydowns, and let
 *     Escape cancel without changing the binding;
 *   - announce the bound accelerator in the control's accessible name using the
 *     raw string (`CmdOrCtrl+Shift+V`), not the glyphs (A11Y-13);
 *   - **refuse the five media keys** — `MediaPlayPause`, `MediaTrackNext`,
 *     `MediaTrackPrevious`, `MediaFastForward`, `MediaRewind`. `global-hotkey`
 *     binds ordinary keys through Carbon `RegisterEventHotKey`, which needs no
 *     Accessibility permission, but falls back to an active `CGEventTap` for
 *     exactly those five — and per ADR-0001 an ad-hoc-signed app loses that
 *     grant on every update, so the shortcut would silently stop working after
 *     an upgrade. The bridge already refuses them; the recorder must say *why*
 *     rather than appearing to ignore the keypress. The default is
 *     `CmdOrCtrl+Shift+V`, and it comes from the backend so the two cannot
 *     drift (CopyPaste-sqw0).
 */
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { AboutTab } from "@/components/settings/AboutTab";
import { AppearanceTab } from "@/components/settings/AppearanceTab";
import { ListTab } from "@/components/settings/ListTab";

export function SettingsView() {
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <header className="flex shrink-0 items-center border-b border-divider bg-panel px-s-3 py-s-2">
        <h1 className="text-sm font-semibold">Settings</h1>
      </header>

      <div className="min-h-0 flex-1 overflow-y-auto p-s-3">
        <div className="mx-auto flex max-w-[var(--content-max-width)] flex-col gap-s-3">
          <Tabs defaultValue="appearance">
            <TabsList aria-label="Settings sections">
              <TabsTrigger value="appearance">Appearance</TabsTrigger>
              <TabsTrigger value="list">List</TabsTrigger>
              <TabsTrigger value="about">About</TabsTrigger>
            </TabsList>

            <TabsContent value="appearance">
              <AppearanceTab />
            </TabsContent>
            <TabsContent value="list">
              <ListTab />
            </TabsContent>
            <TabsContent value="about">
              <AboutTab />
            </TabsContent>
          </Tabs>
        </div>
      </div>
    </div>
  );
}
