/**
 * The background service's own settings — the ones that decide what it
 * captures, keeps and announces.
 *
 * Until this existed, Settings showed appearance and list preferences, which
 * are the WebView's own, while every value that governs the service was
 * reachable only from the CLI (CLAUDE.md rule 6).
 *
 * # Liveness is shown at the field
 *
 * `ConfigData::field_liveness` marks exactly one field `NeedsRestart`, and the
 * service reports it back from the write. It is rendered as a badge beside that
 * field's own title and, once changed, as a sentence with the restart in it —
 * not as a note at the bottom of the pane, which is read after the user has
 * already concluded the switch did nothing.
 *
 * # The auto-delete control is the reason the default is off
 *
 * `sensitive_ttl_secs` ships at `0` because v2 had nowhere to say the sweep had
 * run: v1 shipped 30 seconds beside a tab that showed the value, v2 carried the
 * number without the interface, and a silent irreversible delete is CLAUDE.md
 * rule 4's worst outcome. This is that interface. It states what turning it on
 * costs, at the control; `EventData::swept` and the Diagnostics tab are the
 * other half, so the note names where the deletions are reported rather than
 * admitting they are not.
 */
import { useState } from "react";
import { RotateCw } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { ChoiceRow } from "@/components/settings/ChoiceRow";
import { Row } from "@/components/settings/Row";
import { Section } from "@/components/settings/Section";
import {
  DEDUP_WINDOW_SECS,
  HISTORY_LIMIT,
  MAX_ITEM_BYTES,
  POLL_INTERVAL_MS,
  RETENTION_DAYS,
  SENSITIVE_TTL_SECS,
} from "@/components/settings/serviceChoices";
import {
  useRestartService,
  useServiceConfig,
  useSetServiceConfig,
} from "@/hooks/useServiceConfig";
import { useTranslation } from "@/i18n";
import { isUnavailable } from "@/lib/errors";
import type { ConfigPatch } from "@/lib/ipc";

/** The field `ConfigData::field_liveness` marks `NeedsRestart`. Named rather
 *  than inferred from an empty list: the badge has to be on the row before
 *  anyone touches it. */
const NEEDS_RESTART = "lan_visibility";

export function ServiceTab() {
  const { t } = useTranslation();
  const config = useServiceConfig();
  const save = useSetServiceConfig();
  const restart = useRestartService();
  const [pending, setPending] = useState(false);

  const apply = (patch: ConfigPatch) => {
    save.mutate(patch, {
      onSuccess: (applied) => {
        if (applied.restart_required.length > 0) setPending(true);
      },
    });
  };

  if (config.error !== null && isUnavailable(config.error)) {
    return (
      <p className="py-s-3 text-sm text-muted-foreground">
        {t("settings.service.unavailable")}
      </p>
    );
  }

  const data = config.data?.config;
  if (data === undefined) {
    return (
      <p className="py-s-3 text-sm text-muted-foreground">
        {config.error !== null
          ? t("errors.offline")
          : t("settings.service.loading")}
      </p>
    );
  }

  const busy = save.isPending;
  const sweeping = data.sensitive_ttl_secs > 0;

  return (
    <div className="flex flex-col gap-s-4">
      <Section
        title={t("settings.service.groups.capture.title")}
        description={t("settings.service.groups.capture.description")}
      >
        <ChoiceRow
          title={t("settings.service.poll.title")}
          description={t("settings.service.poll.description")}
          choices={POLL_INTERVAL_MS}
          value={data.poll_interval_ms}
          disabled={busy}
          onChange={(poll_interval_ms) => apply({ poll_interval_ms })}
        />

        <ChoiceRow
          title={t("settings.service.dedup.title")}
          description={t("settings.service.dedup.description")}
          choices={DEDUP_WINDOW_SECS}
          value={data.dedup_window_secs}
          disabled={busy}
          onChange={(dedup_window_secs) => apply({ dedup_window_secs })}
        />

        <ChoiceRow
          title={t("settings.service.maxItem.title")}
          description={t("settings.service.maxItem.description")}
          choices={MAX_ITEM_BYTES}
          value={data.max_item_bytes}
          disabled={busy}
          onChange={(max_item_bytes) => apply({ max_item_bytes })}
        />
      </Section>

      <Section title={t("settings.service.groups.keeping.title")}>
        <ChoiceRow
          title={t("settings.service.historyLimit.title")}
          description={t("settings.service.historyLimit.description")}
          choices={HISTORY_LIMIT}
          value={data.history_limit}
          disabled={busy}
          onChange={(history_limit) => apply({ history_limit })}
        />

        <ChoiceRow
          title={t("settings.service.retention.title")}
          description={t("settings.service.retention.description")}
          choices={RETENTION_DAYS}
          value={data.retention_days}
          disabled={busy}
          onChange={(retention_days) => apply({ retention_days })}
        />

        <ChoiceRow
          title={t("settings.service.sensitive.title")}
          description={t("settings.service.sensitive.description")}
          note={
            <span
              className={
                sweeping
                  ? "text-xs text-warn-strong"
                  : "text-xs text-muted-foreground"
              }
            >
              {sweeping
                ? `${t("settings.service.sensitive.warning")} ${t("settings.service.sensitive.announced")}`
                : t("settings.service.sensitive.off")}
            </span>
          }
          choices={SENSITIVE_TTL_SECS}
          value={data.sensitive_ttl_secs}
          disabled={busy}
          onChange={(sensitive_ttl_secs) => apply({ sensitive_ttl_secs })}
        />
      </Section>

      <Section title={t("settings.service.groups.telling.title")}>
        <SwitchRow
          title={t("settings.service.notify.title")}
          description={t("settings.service.notify.description")}
          id="notify-on-copy"
          checked={data.notify_on_copy}
          disabled={busy}
          onChange={(notify_on_copy) => apply({ notify_on_copy })}
        />

        <SwitchRow
          title={t("settings.service.sound.title")}
          description={t("settings.service.sound.description")}
          id="sound-on-copy"
          checked={data.sound_on_copy}
          disabled={busy}
          onChange={(sound_on_copy) => apply({ sound_on_copy })}
        />
      </Section>

      <Section title={t("settings.service.groups.network.title")}>
        <SwitchRow
          title={t("settings.service.syncEnabled.title")}
          description={t("settings.service.syncEnabled.description")}
          id="sync-enabled"
          checked={data.sync_enabled}
          disabled={busy}
          onChange={(sync_enabled) => apply({ sync_enabled })}
        />

        <SwitchRow
          title={t("settings.service.lan.title")}
          description={t("settings.service.lan.description")}
          id={NEEDS_RESTART}
          checked={data.lan_visibility}
          disabled={busy}
          onChange={(lan_visibility) => apply({ lan_visibility })}
          badge={
            <Badge variant="info">
              {t("settings.service.liveness.needsRestart")}
            </Badge>
          }
          note={
            pending ? (
              <span className="flex flex-wrap items-center gap-s-2 text-xs text-warn-strong">
                {t("settings.service.liveness.pending")}
                <Button
                  variant="outline"
                  size="sm"
                  disabled={restart.isPending}
                  onClick={() => restart.mutate()}
                >
                  <RotateCw aria-hidden="true" />
                  {t(
                    restart.isPending
                      ? "settings.service.liveness.restarting"
                      : "settings.service.liveness.restart",
                  )}
                </Button>
              </span>
            ) : (
              <span className="text-xs text-muted-foreground">
                {t("settings.service.liveness.needsRestartWhy")}
              </span>
            )
          }
        />
      </Section>
    </div>
  );
}

interface SwitchRowProps {
  title: string;
  description: string;
  id: string;
  checked: boolean;
  disabled?: boolean;
  badge?: React.ReactNode;
  note?: React.ReactNode;
  onChange: (checked: boolean) => void;
}

function SwitchRow({
  title,
  description,
  id,
  checked,
  disabled,
  badge,
  note,
  onChange,
}: SwitchRowProps) {
  const { t } = useTranslation();
  return (
    <Row title={title} description={description} badge={badge} note={note}>
      <div className="flex items-center gap-s-2">
        <Switch
          id={id}
          aria-label={title}
          checked={checked}
          disabled={disabled}
          onCheckedChange={onChange}
        />
        <Label htmlFor={id}>{t(checked ? "common.on" : "common.off")}</Label>
      </div>
    </Row>
  );
}
