import { Search, Trash2 } from "lucide-react";
import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";

import { clipSourceMetadata } from "@/components/history/clipMetadata";
import { SourceAppIcon } from "@/components/history/SourceAppIcon";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useHistory } from "@/hooks/useHistory";
import { useTranslation } from "@/i18n";
import { isAndroidPlatform } from "@/lib/platform";
import { listInstalledSourceApps, type InstalledSourceApp } from "@/lib/ipc";

const APP_ID = /^[A-Za-z0-9_-]+(?:\.[A-Za-z0-9_-]+)+$/;

interface SourceExclusionsProps {
  ids: readonly string[];
  disabled?: boolean;
  onChange: (ids: string[]) => void;
}

/** The service is the source of truth. This component only proposes ids that
 * already appeared in history, and never invents an application name or icon. */
export function SourceExclusions({ ids, disabled = false, onChange }: SourceExclusionsProps) {
  const { t } = useTranslation();
  const history = useHistory("");
  const [query, setQuery] = useState("");
  const [validation, setValidation] = useState<string | null>(null);
  const android = isAndroidPlatform();
  const installedApps = useQuery({
    queryKey: ["installed-source-apps"],
    queryFn: listInstalledSourceApps,
    enabled: android,
    staleTime: 5 * 60 * 1000,
  });
  const knownIds = useMemo(
    () =>
      [...new Set(
        history.data?.items
          .map((item) => item.source_app_bundle_id)
          .filter((id): id is string => id !== null) ?? [],
      )].sort((a, b) => a.localeCompare(b)),
    [history.data?.items],
  );
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

  const add = (id: string) => {
    const next = id.trim();
    if (!APP_ID.test(next)) {
      setValidation(t("settings.service.exclusions.invalid"));
      return;
    }
    if (ids.includes(next)) {
      setValidation(t("settings.service.exclusions.exists"));
      return;
    }
    setValidation(null);
    setQuery("");
    onChange([...ids, next]);
  };

  return (
    <section
      data-settings-search-target={`section:${t("settings.service.exclusions.title")}`}
      className="flex flex-col gap-s-2 rounded-lg border border-divider bg-card p-s-3"
    >
      <div className="flex flex-col gap-s-1">
        <h2 className="text-sm font-semibold">{t("settings.service.exclusions.title")}</h2>
        <p className="text-xs text-muted-foreground">
          {t(
            android
              ? "settings.service.exclusions.androidLimitation"
              : "settings.service.exclusions.description",
          )}
        </p>
      </div>

      {android ? (
        <AndroidExclusionPicker
          apps={visibleInstalledApps}
          selectedIds={ids}
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
              aria-label={t("settings.service.exclusions.inputLabel")}
              value={query}
              disabled={disabled}
              placeholder={t("settings.service.exclusions.placeholder")}
              aria-invalid={validation !== null || undefined}
              onChange={(event) => {
                setQuery(event.target.value);
                setValidation(null);
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
          {validation && <p role="alert" className="text-xs text-destructive">{validation}</p>}

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
                    const item = history.data?.items.find((candidate) => candidate.source_app_bundle_id === id);
                    if (!item) return null;
                    const source = clipSourceMetadata(item);
                    return (
                      <button
                        key={id}
                        type="button"
                        disabled={disabled || ids.includes(id)}
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
    </section>
  );
}

interface AndroidExclusionPickerProps {
  apps: readonly InstalledSourceApp[];
  selectedIds: readonly string[];
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
  return (
    <div className="flex flex-col gap-s-2">
      <Input
        aria-label={t("settings.service.exclusions.searchInstalled")}
        value={query}
        disabled={disabled}
        placeholder={t("settings.service.exclusions.searchInstalled")}
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
              disabled={disabled || selectedIds.includes(app.package_id)}
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
