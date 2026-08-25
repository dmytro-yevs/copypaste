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
import { VisuallyHidden } from "@/components/ui";
import {
  resolveSettingsSearch,
  SettingsSearchField,
  type ResolvedSettingsSearchItem,
} from "@/features/settings/components/SettingsSearch";
import { SettingsCompactNavigation } from "@/features/settings/patterns/SettingsCompactNavigation";
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
  const [searchQuery, setSearchQuery] = useState("");
  const [searchExpanded, setSearchExpanded] = useState(true);
  const [searchAnnouncement, setSearchAnnouncement] = useState("");
  const deferredSearchQuery = useDeferredValue(searchQuery);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const highlightTimer = useRef<number | undefined>(undefined);
  const contentViewportRef = useRef<HTMLDivElement>(null);
  const requestedTab = useUi((state) => state.settingsTab);
  const setSettingsTab = useUi((state) => state.setSettingsTab);

  const resetContentScroll = useCallback(() => {
    if (contentViewportRef.current) contentViewportRef.current.scrollTop = 0;
  }, []);
  const openSection = useCallback((section: PreferenceSection) => {
    setDesktopSection(section);
    setMobileSection(section);
    resetContentScroll();
  }, [resetContentScroll]);
  const closeMobileSection = useCallback(() => {
    setMobileSection(null);
    resetContentScroll();
  }, [resetContentScroll]);
  const compactBack = useSettingsLevel(
    compact && mobileSection !== null,
    closeMobileSection,
  );

  usePrefs((state) => state.theme);
  const prefsReady = import.meta.env.MODE === "test" || usePrefs.persist.hasHydrated();

  useEffect(() => {
    if (requestedTab === null) return;
    const section = preferenceSectionForTab(requestedTab);
    setDesktopSection(section);
    setMobileSection(section);
    resetContentScroll();
    setSettingsTab(null);
  }, [requestedTab, resetContentScroll, setSettingsTab]);

  const controller = useMemo(
    () => ({ prefsReady, capabilities }),
    [capabilities, prefsReady],
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
        return [item.tab, sectionLabels.get(item.tab) ?? item.tab];
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
    },
    [],
  );

  const selectSearchResult = (result: ResolvedSettingsSearchItem) => {
    openSection(result.item.tab);
    setSearchQuery("");
    setSearchAnnouncement(t("settings.search.opened", { title: result.title }));
    window.requestAnimationFrame(() => {
      window.requestAnimationFrame(() => {
        const targetIds = [
          `row:${result.title}`,
          `section:${result.title}`,
          result.sectionLabel && `section:${result.sectionLabel}`,
        ];
        const target = [...document.querySelectorAll<HTMLElement>("[data-settings-search-target]")]
          .find((element) => targetIds.includes(element.dataset.settingsSearchTarget));
        if (!target) return;

        target.scrollIntoView({ behavior: "smooth", block: "center" });
        target.tabIndex = -1;
        target.focus({ preventScroll: true });
        target.dataset.settingsSearchHighlight = "true";
        if (highlightTimer.current !== undefined) {
          window.clearTimeout(highlightTimer.current);
        }
        highlightTimer.current = window.setTimeout(() => {
          delete target.dataset.settingsSearchHighlight;
          highlightTimer.current = undefined;
        }, 1800);
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
            title="Preferences"
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
              renderSection={(section) => renderPreferenceSection(section, controller)}
            />
          </Container>
        </ScrollViewport>
      ) : (
        <ScrollViewport ref={contentViewportRef} className={styles.contentViewport}>
          <Container width="fluid" gutter="screen" className={styles.desktopContent}>
            <ScreenHeader
              eyebrow="Personalize CopyPaste"
              title="Preferences"
              description="Choose a focused page for each part of CopyPaste."
              actions={search}
            />
            <div className={styles.desktopBody}>
              <SettingsNavigation
                sections={sections}
                active={desktopSection}
                onSelect={(section) => {
                  openSection(section);
                }}
              />
              <section
                role="tabpanel"
                className={styles.content}
                aria-label={activeDefinition?.label}
              >
                <h2 className={styles.panelTitle}>{activeDefinition?.label}</h2>
                <div className={styles.sectionStack}>
                  {renderPreferenceSection(desktopSection, controller)}
                </div>
              </section>
            </div>
          </Container>
        </ScrollViewport>
      )}
      <VisuallyHidden role="status" aria-live="polite">
        {searchAnnouncement}
      </VisuallyHidden>
    </Screen>
  );
}
