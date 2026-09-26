import {
  useCallback,
  useDeferredValue,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import {
  Container,
  Screen,
  ScrollViewport,
} from "@/components/layout";
import { ScreenHeader } from "@/components/shared";
import { Tabs, TabsContent, VisuallyHidden } from "@/components/ui";
import {
  resolveSettingsSearch,
  SettingsSearchField,
  type ResolvedSettingsSearchItem,
} from "@/features/settings/components/SettingsSearch";
import { SettingsCompactNavigation } from "@/features/settings/patterns/SettingsCompactNavigation";
import type { DiagnosticsView } from "@/features/settings/patterns/DiagnosticsTab";
import { useSettingsLevel } from "@/features/settings/hooks/useSettingsLevel";
import {
  preferenceSectionForTab,
  visiblePreferenceSections,
  type PreferenceSection,
} from "@/features/settings/model/preferenceSections";
import {
  SETTINGS_SEARCH_ITEMS,
  type SettingsSearchTab,
} from "@/features/settings/model/settingsSearchIndex";
import { settingsCapabilities } from "@/features/settings/model/settingsNavigation";
import { SettingsNavigation } from "@/features/settings/patterns/SettingsNavigation";
import { renderPreferenceSection } from "@/features/settings/patterns/settingsTabs";
import {
  useObservedElementSize,
  useViewportMetrics,
} from "@/hooks/useViewportMetrics";
import { useTranslation } from "@/i18n";
import { EXPANDED_MIN_PX } from "@/lib/layoutBreakpoints";
import { currentPlatform } from "@/lib/platform";
import { usePrefs } from "@/store/prefs";
import { useUi } from "@/store/ui";
import styles from "./SettingsScreen.module.css";

const SEARCH_HIGHLIGHT_DURATION_MS = 3_200;
const SEARCH_TARGET_WAIT_MS = 1_500;
const SEARCH_HIGHLIGHT_DURATION_PROPERTY = "--settings-search-highlight-duration";

function clearSearchHighlight(target: HTMLElement | null) {
  if (target === null) return;
  delete target.dataset.settingsSearchHighlight;
  target.style.removeProperty(SEARCH_HIGHLIGHT_DURATION_PROPERTY);
}

export function SettingsScreen() {
  const { t } = useTranslation();
  const viewportCompact = useViewportMetrics().sizeClass === "compact";
  const { ref: screenRef, width: screenWidth } =
    useObservedElementSize<HTMLElement>();
  const compact = screenWidth > 0
    ? screenWidth < EXPANDED_MIN_PX
    : viewportCompact;
  const platform = currentPlatform();
  const capabilities = useMemo(() => settingsCapabilities(platform), [platform]);
  const sections = useMemo(
    () => visiblePreferenceSections(capabilities),
    [capabilities],
  );
  const [desktopSection, setDesktopSection] =
    useState<PreferenceSection>("appearance");
  const [mobileSection, setMobileSection] =
    useState<PreferenceSection | null>(null);
  const [diagnosticsView, setDiagnosticsView] = useState<DiagnosticsView>("overview");
  const [revealAdvancedKey, setRevealAdvancedKey] = useState<string>();
  const advancedRevealSequence = useRef(0);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchExpanded, setSearchExpanded] = useState(true);
  const [searchAnnouncement, setSearchAnnouncement] = useState("");
  const deferredSearchQuery = useDeferredValue(searchQuery);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const highlightTimer = useRef<number | undefined>(undefined);
  const highlightedTarget = useRef<HTMLElement | null>(null);
  const cancelPendingSearchTarget = useRef<(() => void) | null>(null);
  const searchFocusSequence = useRef(0);
  const contentViewportRef = useRef<HTMLDivElement>(null);
  const requestedTab = useUi((state) => state.settingsTab);
  const setSettingsTab = useUi((state) => state.setSettingsTab);

  const resetContentScroll = useCallback(() => {
    if (contentViewportRef.current) contentViewportRef.current.scrollTop = 0;
  }, []);
  const openSection = useCallback((section: PreferenceSection) => {
    setDesktopSection(section);
    setMobileSection(section);
    setDiagnosticsView("overview");
    setRevealAdvancedKey(undefined);
    resetContentScroll();
  }, [resetContentScroll]);
  const openEvents = useCallback(() => {
    setDiagnosticsView("runtime-events");
    resetContentScroll();
  }, [resetContentScroll]);
  const closeEvents = useCallback(() => {
    setDiagnosticsView("overview");
    resetContentScroll();
  }, [resetContentScroll]);
  const compactPath = useMemo(
    () => !compact || mobileSection === null
      ? []
      : diagnosticsView === "runtime-events" && mobileSection === "diagnostics"
        ? ["diagnostics", "runtime-events"]
        : [mobileSection],
    [compact, diagnosticsView, mobileSection],
  );
  const restoreCompactPath = useCallback((path: readonly string[]) => {
    const section = sections.find((item) => item.value === path[0])?.value ?? null;
    if (section !== null) setDesktopSection(section);
    setMobileSection(section);
    setDiagnosticsView(
      section === "diagnostics" && path[1] === "runtime-events"
        ? "runtime-events"
        : "overview",
    );
    resetContentScroll();
  }, [resetContentScroll, sections]);
  const compactBack = useSettingsLevel(compactPath, restoreCompactPath);

  usePrefs((state) => state.theme);
  const prefsReady = import.meta.env.MODE === "test" || usePrefs.persist.hasHydrated();

  useEffect(() => {
    if (requestedTab === null) return;
    const section = preferenceSectionForTab(requestedTab);
    setDesktopSection(section);
    setMobileSection(section);
    setDiagnosticsView(requestedTab === "runtime-events" ? "runtime-events" : "overview");
    resetContentScroll();
    setSettingsTab(null);
  }, [requestedTab, resetContentScroll, setSettingsTab]);

  const controller = useMemo(
    () => ({
      prefsReady,
      capabilities,
      diagnosticsView,
      revealAdvancedKey,
      onOpenEvents: openEvents,
      onBackFromEvents: compact ? undefined : closeEvents,
    }),
    [capabilities, closeEvents, compact, diagnosticsView, openEvents, prefsReady, revealAdvancedKey],
  );
  const activeDefinition = sections.find(
    (section) => section.value === desktopSection,
  );
  const searchPlatform = platform === "android"
    ? "android"
    : platform === "windows"
      ? "windows"
      : "desktop";
  const searchTabLabels = useMemo(() => {
    const sectionLabels = new Map(
      sections.map((section) => [section.value, section.label]),
    );
    return new Map<SettingsSearchTab, string>(
      SETTINGS_SEARCH_ITEMS.map((item) => {
        return [item.tab, sectionLabels.get(preferenceSectionForTab(item.tab)) ?? item.tab];
      }),
    );
  }, [sections]);

  useEffect(() => {
    if (sections.some((section) => section.value === desktopSection)) return;
    const fallback = sections[0]?.value ?? "appearance";
    setDesktopSection(fallback);
    setMobileSection((current) =>
      current !== null && !sections.some((section) => section.value === current)
        ? null
        : current,
    );
  }, [desktopSection, sections]);
  const searchResults = useMemo(
    () => resolveSettingsSearch(
      SETTINGS_SEARCH_ITEMS.filter((item) =>
        (!item.capability || capabilities[item.capability]) &&
        (!item.platforms || item.platforms.includes(searchPlatform)),
      ),
      searchTabLabels,
      (key) => t(key as never),
      deferredSearchQuery,
    ),
    [capabilities, deferredSearchQuery, searchPlatform, searchTabLabels, t],
  );

  useEffect(
    () => () => {
      if (highlightTimer.current !== undefined) {
        window.clearTimeout(highlightTimer.current);
      }
      cancelPendingSearchTarget.current?.();
      searchFocusSequence.current += 1;
      clearSearchHighlight(highlightedTarget.current);
    },
    [],
  );

  const selectSearchResult = (result: ResolvedSettingsSearchItem) => {
    cancelPendingSearchTarget.current?.();
    searchFocusSequence.current += 1;
    const focusSequence = searchFocusSequence.current;
    openSection(preferenceSectionForTab(result.item.tab));
    if (result.item.tab === "runtime-events") openEvents();
    if (result.item.disclosure !== undefined) {
      advancedRevealSequence.current += 1;
      setRevealAdvancedKey(
        `${result.item.title}:${advancedRevealSequence.current}`,
      );
    }
    setSearchQuery("");
    setSearchAnnouncement(t("settings.search.opened", { title: result.title }));
    window.requestAnimationFrame(() => {
      window.requestAnimationFrame(() => {
        if (focusSequence !== searchFocusSequence.current) return;
        let observer: MutationObserver | undefined;
        let waitTimer: number | undefined;
        const cancel = () => {
          observer?.disconnect();
          if (waitTimer !== undefined) window.clearTimeout(waitTimer);
          cancelPendingSearchTarget.current = null;
        };
        cancelPendingSearchTarget.current = cancel;
        const focusTarget = (allowSection: boolean) => {
          const candidates = [...document.querySelectorAll<HTMLElement>("[data-settings-search-target]")];
          const target = candidates.find((element) =>
            element.dataset.settingsSearchTarget === `row:${result.title}`,
          ) ?? (allowSection ? candidates.find((element) =>
            element.dataset.settingsSearchTarget === `section:${result.title}` ||
            element.dataset.settingsSearchTarget === `section:${result.sectionLabel}`,
          ) : undefined);
          if (!target) return false;
          cancel();
          target.scrollIntoView({ behavior: "smooth", block: "center" });
          const field = target instanceof HTMLDetailsElement
            ? target.querySelector<HTMLElement>("summary")
            : result.item.tab === "runtime-events"
            ? target.querySelector<HTMLElement>('[role="searchbox"], input[type="search"]')
            : target.querySelector<HTMLElement>(
              '[role="combobox"], input, select, textarea, button, [role="switch"], [role="slider"]',
            );
          if (!field) target.tabIndex = -1;
          (field ?? target).focus({ preventScroll: true });
          if (highlightTimer.current !== undefined) {
            window.clearTimeout(highlightTimer.current);
          }
          clearSearchHighlight(highlightedTarget.current);
          highlightedTarget.current = target;
          target.style.setProperty(
            SEARCH_HIGHLIGHT_DURATION_PROPERTY,
            `${SEARCH_HIGHLIGHT_DURATION_MS}ms`,
          );
          target.dataset.settingsSearchHighlight = "true";
          highlightTimer.current = window.setTimeout(() => {
            clearSearchHighlight(target);
            highlightedTarget.current = null;
            highlightTimer.current = undefined;
          }, SEARCH_HIGHLIGHT_DURATION_MS);
          return true;
        };
        observer = new MutationObserver(() => focusTarget(false));
        observer.observe(document.body, { childList: true, subtree: true });
        waitTimer = window.setTimeout(() => {
          if (!focusTarget(true)) cancel();
        }, SEARCH_TARGET_WAIT_MS);
        focusTarget(false);
      });
    });
  };

  const search = (
    <div className={styles.search} data-expanded={searchExpanded || undefined}>
      <SettingsSearchField
        query={searchQuery}
        onQueryChange={setSearchQuery}
        results={searchResults}
        onSelect={selectSearchResult}
        inputRef={searchInputRef}
        expanded={searchExpanded}
        onExpandedChange={setSearchExpanded}
      />
    </div>
  );

  return (
    <Screen ref={screenRef} className={styles.root}>
      {compact && mobileSection === null ? (
        <div className={styles.compactHeader}>
          <ScreenHeader
            eyebrow="Personalize CopyPaste"
            title="Settings"
            description="Choose a focused page for each part of CopyPaste."
          />
          {search}
        </div>
      ) : null}

      {compact ? (
        <ScrollViewport
          ref={contentViewportRef}
          className={styles.compactViewport}
          padding="compact"
        >
          <Container width="reading" gutter="none">
            <SettingsCompactNavigation
              sections={sections}
              active={mobileSection}
              onSelect={openSection}
              onBack={compactBack}
              backLabel={diagnosticsView === "runtime-events" ? "Back to Diagnostics" : "Back to Settings"}
              renderSection={(section) => renderPreferenceSection(section, controller)}
            />
          </Container>
        </ScrollViewport>
      ) : (
        <ScrollViewport ref={contentViewportRef} className={styles.contentViewport}>
          <Container width="fluid" gutter="screen" className={styles.desktopContent}>
            <ScreenHeader
              eyebrow="Personalize CopyPaste"
              title="Settings"
              description="Choose a focused page for each part of CopyPaste."
              actions={search}
            />
            <Tabs value={desktopSection} onValueChange={(value) => openSection(value as PreferenceSection)}>
              <div className={styles.desktopBody}>
                <SettingsNavigation sections={sections} />
                <TabsContent value={desktopSection} className={styles.content}>
                  <h2 className={styles.panelTitle}>{activeDefinition?.label}</h2>
                  <div className={styles.sectionStack}>
                    {renderPreferenceSection(desktopSection, controller)}
                  </div>
                </TabsContent>
              </div>
            </Tabs>
          </Container>
        </ScrollViewport>
      )}
      <VisuallyHidden role="status" aria-live="polite">
        {searchAnnouncement}
      </VisuallyHidden>
    </Screen>
  );
}
