import { ChevronDown, Search, Trash2 } from "lucide-react";
import { useId, useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";

import { clipSourceMetadata } from "@/components/history/clipMetadata";
import { SourceAppIcon } from "@/components/history/SourceAppIcon";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useHistory } from "@/hooks/useHistory";
import { useTranslation } from "@/i18n";
import { canonicalExclusion, findExclusion } from "@/lib/exclusions";
import { isAndroidPlatform, isWindowsPlatform } from "@/lib/platform";
import { listInstalledSourceApps, type InstalledSourceApp, type Item } from "@/lib/ipc";

interface SourceExclusionsProps {
  ids: readonly string[];
  disabled?: boolean;
  collapsible?: boolean;
  onChange: (ids: string[]) => void;
}

/** The service is the source of truth. This component only proposes ids that
 * already appeared in history, and never invents an application name or icon. */
export function SourceExclusions({
  ids,
  disabled = false,
  collapsible = false,
  onChange,
}: SourceExclusionsProps) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(!collapsible);
  const controlsId = useId();
  const android = isAndroidPlatform();
  const windows = isWindowsPlatform();

  const heading = (
    <div className="flex flex-col gap-s-1">
      <h2 className="text-sm font-semibold">
        {collapsible ? (
          <Button
            type="button"
            variant="ghost"
            aria-expanded={expanded}
            aria-controls={controlsId}
            className="w-full justify-between px-0 text-left whitespace-normal"
            onClick={() => setExpanded((current) => !current)}
          >
            {t("settings.service.exclusions.title")}
            <ChevronDown aria-hidden="true" className={expanded ? "rotate-180" : undefined} />
          </Button>
        ) : (
          t("settings.service.exclusions.title")
        )}
      </h2>
      <p className="text-xs text-muted-foreground">
        {t(
          android
            ? "settings.service.exclusions.androidLimitation"
            : windows
              ? "settings.service.exclusions.windowsDescription"
              : "settings.service.exclusions.description",
        )}
      </p>
    </div>
  );

  // The region the button controls stays mounted and is hidden, because INV-7
  // forbids an accessibility pointer to a node that is not there. The *editor*
  // is unmounted: it owns the history query, and mounting it starts a 3s poll
  // that decrypts a page of clipboard history for a panel nobody has opened.
  return (
    <section
      data-settings-search-target={`section:${t("settings.service.exclusions.title")}`}
      className="flex flex-col gap-s-2 rounded-lg border border-divider bg-card p-s-3"
    >
      {heading}
      <div id={controlsId} hidden={!expanded} className="flex flex-col gap-s-2">
        {expanded && (
          <ExclusionsEditor
            ids={ids}
            disabled={disabled}
            android={android}
            windows={windows}
            onChange={onChange}
          />
        )}
      </div>
    </section>
  );
}

interface ExclusionsEditorProps {
  ids: readonly string[];
  disabled: boolean;
  android: boolean;
  windows: boolean;
  onChange: (ids: string[]) => void;
}

function ExclusionsEditor({
  ids,
  disabled,
  android,
  windows,
  onChange,
}: ExclusionsEditorProps) {
  const { t } = useTranslation();
  const history = useHistory("");
  const [query, setQuery] = useState("");
  const [validation, setValidation] = useState<string | null>(null);
  const [normalizedNotice, setNormalizedNotice] = useState<string | null>(null);
  const validationId = useId();
  const noticeId = useId();
  const installedApps = useQuery({
    queryKey: ["installed-source-apps"],
    queryFn: listInstalledSourceApps,
    enabled: android,
    staleTime: 5 * 60 * 1000,
  });

  /** One pass over the history, not one per proposed id: the `find` this
   *  replaces ran inside the render loop, so a 10,000-row history times the
   *  distinct apps in it was scanned on every keystroke. */
  const firstByApp = useMemo(() => {
    const first = new Map<string, Item>();
    for (const item of history.data?.items ?? []) {
      const app = item.source_app_bundle_id;
      if (app !== null && !first.has(app)) first.set(app, item);
    }
    return first;
  }, [history.data?.items]);

  const knownIds = useMemo(
    () => [...firstByApp.keys()].sort((a, b) => a.localeCompare(b)),
    [firstByApp],
  );
  const selectedIds = useMemo(() => new Set(ids), [ids]);
  const normalized = query.trim();
  const visibleKnownIds = knownIds.filter((id) =>
    id.toLocaleLowerCase().includes(normalized.toLocaleLowerCase()),
  );
  const visibleInstalledApps = useMemo(() => {
    const needle = normalized.toLocaleLowerCase();
    return (installedApps.data ?? []).filter((app) =>
      !needle || app.label.toLocaleLowerCase().includes(needle) || app.package_id.toLocaleLowerCase().includes(needle),
    );
  }, [installedApps.data, normalized]);
  const installedById = useMemo(
    () => new Map((installedApps.data ?? []).map((app) => [app.package_id, app])),
    [installedApps.data],
  );

  /** Windows: Chrome.exe, chrome, and a pasted path are one program. */
  const add = (id: string) => {
    setNormalizedNotice(null);
    const next = canonicalExclusion(id, windows);
    if (next === null) {
      setValidation(t(windows
        ? "settings.service.exclusions.windowsInvalid"
        : "settings.service.exclusions.invalid"));
      return;
    }
    const existing = findExclusion(ids, next, windows);
    if (existing !== undefined) {
      setValidation(windows
        ? t("settings.service.exclusions.windowsExists", { id: existing })
        : t("settings.service.exclusions.exists"));
      return;
    }
    setValidation(null);
    setQuery("");
    if (next !== id.trim()) {
      setNormalizedNotice(t("settings.service.exclusions.normalized", { id: next }));
    }
    onChange([...ids, next]);
  };

  return (
    <>
      {android ? (
        <AndroidExclusionPicker
          apps={visibleInstalledApps}
          selectedIds={selectedIds}
          query={query}
          disabled={disabled}
          loading={installedApps.isLoading}
          failed={installedApps.isError}
          onQueryChange={setQuery}
          onAdd={add}
        />
      ) : (
      <>
          <div className="flex flex-col gap-s-2 sm:flex-row">
            <Input
              aria-label={t(windows
                ? "settings.service.exclusions.windowsInputLabel"
                : "settings.service.exclusions.inputLabel")}
              value={query}
              disabled={disabled}
              placeholder={t(windows
                ? "settings.service.exclusions.windowsPlaceholder"
                : "settings.service.exclusions.placeholder")}
              aria-invalid={validation !== null || undefined}
              aria-describedby={validation ? validationId : normalizedNotice ? noticeId : undefined}
              onChange={(event) => {
                setQuery(event.target.value);
                setValidation(null);
                setNormalizedNotice(null);
              }}
              onKeyDown={(event) => {
                if (event.key !== "Enter") return;
                event.preventDefault();
                add(normalized);
              }}
            />
            <Button
              type="button"
              variant="outline"
              disabled={disabled || normalized.length === 0}
              onClick={() => add(normalized)}
            >
              {t("settings.service.exclusions.add")}
            </Button>
          </div>
          {validation && (
            <p id={validationId} role="alert" className="text-xs text-destructive">
              {validation}
            </p>
          )}
          {!validation && normalizedNotice && (
            <p id={noticeId} role="status" className="text-xs text-muted-foreground">
              {normalizedNotice}
            </p>
          )}

          {knownIds.length > 0 && (
            <div className="flex flex-col gap-s-1">
              <p className="flex items-center gap-s-1 text-xs font-medium text-muted-foreground">
                <Search size={14} aria-hidden="true" />
                {t("settings.service.exclusions.seen")}
              </p>
              <div className="flex max-h-32 flex-col overflow-y-auto rounded-md border border-divider">
                {visibleKnownIds.length === 0 ? (
                  <p className="px-s-2 py-s-2 text-xs text-muted-foreground">
                    {t("settings.service.exclusions.noMatches")}
                  </p>
                ) : (
                  visibleKnownIds.map((id) => {
                    const item = firstByApp.get(id);
                    if (!item) return null;
                    const source = clipSourceMetadata(item);
                    return (
                      <button
                        key={id}
                        type="button"
                        disabled={disabled || selectedIds.has(id)}
                        className="flex min-h-9 items-center gap-s-2 px-s-2 text-left text-sm hover:bg-accent disabled:cursor-not-allowed disabled:opacity-50"
                        onClick={() => add(id)}
                      >
                        <SourceAppIcon
                          bundleId={id}
                          Fallback={source.Icon}
                          className="size-4"
                        />
                        <span className="min-w-0 flex-1 truncate">{source.label}</span>
                        <span className="truncate text-xs text-muted-foreground">{id}</span>
                      </button>
                    );
                  })
                )}
              </div>
            </div>
          )}
      </>
      )}

      {ids.length > 0 && (
        <ul className="flex flex-col divide-y divide-divider rounded-md border border-divider">
          {ids.map((id) => {
            const app = installedById.get(id);
            return (
            <li key={id} className="flex min-h-10 items-center gap-s-2 px-s-2">
              {android && (
                <SourceAppIcon bundleId={id} Fallback={Search} className="size-5 text-muted-foreground" />
              )}
              <span className="min-w-0 flex-1">
                {app && <span className="block truncate text-sm">{app.label}</span>}
                <span className="block truncate font-mono text-xs text-muted-foreground">{id}</span>
              </span>
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                aria-label={t("settings.service.exclusions.remove", { id })}
                disabled={disabled}
                onClick={() => onChange(ids.filter((candidate) => candidate !== id))}
              >
                <Trash2 aria-hidden="true" />
              </Button>
            </li>
            );
          })}
        </ul>
      )}
    </>
  );
}

interface AndroidExclusionPickerProps {
  apps: readonly InstalledSourceApp[];
  selectedIds: ReadonlySet<string>;
  query: string;
  disabled: boolean;
  loading: boolean;
  failed: boolean;
  onQueryChange: (value: string) => void;
  onAdd: (id: string) => void;
}

function AndroidExclusionPicker({
  apps, selectedIds, query, disabled, loading, failed, onQueryChange, onAdd,
}: AndroidExclusionPickerProps) {
  const { t } = useTranslation();
  const searchLabel = t("settings.service.exclusions.searchInstalled");
  return (
    <div className="flex flex-col gap-s-2">
      <label htmlFor="android-exclusion-search" className="sr-only">
        {searchLabel}
      </label>
      <Input
        id="android-exclusion-search"
        aria-label={searchLabel}
        value={query}
        disabled={disabled}
        placeholder={searchLabel}
        onChange={(event) => onQueryChange(event.target.value)}
      />
      <div className="flex max-h-56 flex-col overflow-y-auto rounded-md border border-divider">
        {loading ? (
          <p className="px-s-2 py-s-2 text-xs text-muted-foreground">{t("settings.service.loading")}</p>
        ) : failed ? (
          <p role="alert" className="px-s-2 py-s-2 text-xs text-destructive">
            {t("settings.service.exclusions.appsUnavailable")}
          </p>
        ) : apps.length === 0 ? (
          <p className="px-s-2 py-s-2 text-xs text-muted-foreground">
            {t("settings.service.exclusions.noInstalledMatches")}
          </p>
        ) : (
          apps.map((app) => (
            <button
              key={app.package_id}
              type="button"
              disabled={disabled || selectedIds.has(app.package_id)}
              className="flex min-h-11 items-center gap-s-2 px-s-2 text-left hover:bg-accent disabled:cursor-not-allowed disabled:opacity-50"
              onClick={() => onAdd(app.package_id)}
            >
              <SourceAppIcon bundleId={app.package_id} Fallback={Search} className="size-5 text-muted-foreground" />
              <span className="min-w-0 flex-1">
                <span className="block truncate text-sm">{app.label}</span>
                <span className="block truncate font-mono text-xs text-muted-foreground">{app.package_id}</span>
              </span>
            </button>
          ))
        )}
      </div>
    </div>
  );
}
