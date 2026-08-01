/**
 * Liveness is shown at the field it belongs to, never as a note at the bottom
 * of the pane: that is read after the user has already concluded the switch did
 * nothing.
 *
 * `sensitive_ttl_secs` ships at `0` because a silent irreversible delete is
 * CLAUDE.md rule 4's worst outcome — v1 shipped 30 seconds beside a tab that
 * showed the sweep had run, v2 carried the number without the interface. This
 * is that interface, and the note names where the deletions are reported.
 */
import { useState } from "react";
import { LoaderCircle, RotateCw, TriangleAlert } from "lucide-react";

import { StateNotice } from "@/components/StateNotice";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { ChoiceRow } from "@/components/settings/ChoiceRow";
import { Row } from "@/components/settings/Row";
import { Section } from "@/components/settings/Section";
import { SourceExclusions } from "@/components/settings/SourceExclusions";
import {
  DEDUP_WINDOW_SECS,
  HISTORY_LIMIT,
  MAX_DECODED_IMAGE_MB,
  MAX_FILE_SIZE_BYTES,
  MAX_FILE_SIZE_BYTES_LIMIT,
  MAX_IMAGE_SIZE_BYTES,
  MAX_TEXT_SIZE_BYTES,
  MIN_DECODED_IMAGE_MB,
  MIN_FILE_SIZE_BYTES,
  MIN_IMAGE_SIZE_BYTES,
  MIN_TEXT_SIZE_BYTES,
  POLL_INTERVAL_MS,
  POLL_INTERVAL_MAX_MS,
  POLL_INTERVAL_MIN_MS,
  RETENTION_DAYS,
  SENSITIVE_TTL_SECS,
  STORAGE_QUOTA_BYTES,
} from "@/components/settings/serviceChoices";
import {
  useRestartService,
  useServiceConfig,
  useSetServiceConfig,
} from "@/hooks/useServiceConfig";
import { useCaptureMutation, useCaptureState } from "@/hooks/useCapture";
import { useStatus } from "@/hooks/useHistory";
import { useTranslation } from "@/i18n";
import { primaryOf } from "@/lib/capture";
import { isUnavailable } from "@/lib/errors";
import { captureArm, captureRefresh, captureSetEnabled, type ConfigPatch } from "@/lib/ipc";

/** The field `ConfigData::field_liveness` marks `NeedsRestart`. Named rather
 *  than inferred from an empty list: the badge has to be on the row before
 *  anyone touches it. */
const NEEDS_RESTART = "lan_visibility";

export function ServiceTab() {
  const { t } = useTranslation();
  const config = useServiceConfig();
  const save = useSetServiceConfig();
  const restart = useRestartService();
  const status = useStatus();
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
      <StateNotice
        icon={TriangleAlert}
        tone="attention"
        title={t("settings.service.unavailable")}
      />
    );
  }

  const data = config.data?.config;
  if (data === undefined) {
    return (
      <StateNotice
        icon={config.error !== null ? TriangleAlert : LoaderCircle}
        busy={config.error === null}
        tone={config.error !== null ? "danger" : "info"}
        title={
          config.error !== null
            ? t("errors.offline")
            : t("settings.service.loading")
        }
        action={
          config.error !== null
            ? {
                label: t("common.tryAgain"),
                icon: RotateCw,
                onClick: () => void config.refetch(),
              }
            : undefined
        }
      />
    );
  }

  const busy = save.isPending;
  const sweeping = data.sensitive_ttl_secs > 0;
  const supportsPrivateMode =
    status.data !== undefined && status.data.clipboard_backend !== "android-inprocess";

  return (
    <div className="flex flex-col gap-s-4">
      <CaptureControls />
      <Section
        title={t("settings.service.groups.capture.title")}
        description={t("settings.service.groups.capture.description")}
      >
        {supportsPrivateMode && (
          <SwitchRow
            title={t("settings.service.privateMode.title")}
            description={t("settings.service.privateMode.description")}
            id="private-mode"
            checked={data.private_mode}
            disabled={busy}
            onChange={(private_mode) => apply({ private_mode })}
          />
        )}

        <ChoiceRow
          title={t("settings.service.poll.title")}
          description={t("settings.service.poll.description")}
          choices={POLL_INTERVAL_MS}
          value={data.poll_interval_ms}
          disabled={busy}
          validation={{
            min: POLL_INTERVAL_MIN_MS,
            max: POLL_INTERVAL_MAX_MS,
            message: t("settings.service.validation.poll"),
          }}
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
          title={t("settings.service.maxText.title")}
          description={t("settings.service.maxText.description")}
          choices={MAX_TEXT_SIZE_BYTES}
          value={data.max_text_size_bytes}
          disabled={busy}
          validation={{
            min: MIN_TEXT_SIZE_BYTES,
            message: t("settings.service.validation.text"),
          }}
          onChange={(max_text_size_bytes) => apply({ max_text_size_bytes })}
        />

        <ChoiceRow
          title={t("settings.service.maxImage.title")}
          description={t("settings.service.maxImage.description")}
          choices={MAX_IMAGE_SIZE_BYTES}
          value={data.max_image_size_bytes}
          disabled={busy}
          validation={{
            min: MIN_IMAGE_SIZE_BYTES,
            message: t("settings.service.validation.image"),
          }}
          onChange={(max_image_size_bytes) => apply({ max_image_size_bytes })}
        />

        <ChoiceRow
          title={t("settings.service.maxFile.title")}
          description={t("settings.service.maxFile.description")}
          choices={MAX_FILE_SIZE_BYTES}
          value={data.max_file_size_bytes}
          disabled={busy}
          validation={{
            min: MIN_FILE_SIZE_BYTES,
            max: MAX_FILE_SIZE_BYTES_LIMIT,
            message: t("settings.service.validation.file"),
          }}
          onChange={(max_file_size_bytes) => apply({ max_file_size_bytes })}
        />

        <ChoiceRow
          title={t("settings.service.maxDecodedImage.title")}
          description={t("settings.service.maxDecodedImage.description")}
          choices={MAX_DECODED_IMAGE_MB}
          value={data.max_decoded_image_mb}
          disabled={busy}
          validation={{
            min: MIN_DECODED_IMAGE_MB,
            message: t("settings.service.validation.decodedImage"),
          }}
          onChange={(max_decoded_image_mb) => apply({ max_decoded_image_mb })}
        />

        <SourceExclusions
          ids={data.excluded_app_bundle_ids}
          disabled={busy}
          onChange={(excluded_app_bundle_ids) => apply({ excluded_app_bundle_ids })}
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
          title={t("settings.service.storageQuota.title")}
          description={t("settings.service.storageQuota.description")}
          choices={STORAGE_QUOTA_BYTES}
          value={data.storage_quota_bytes}
          disabled={busy}
          onChange={(storage_quota_bytes) => apply({ storage_quota_bytes })}
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

/**
 * Background capture belongs in Service on both platforms.  The Android
 * setup route remains available for its permission ladder, but this row is
 * where a user can see and change the live capture state without having to
 * hunt for a platform-specific screen.
 */
function CaptureControls() {
  const { t } = useTranslation();
  const capture = useCaptureState();
  const run = useCaptureMutation();
  const snapshot = capture.data;

  if (snapshot === undefined) {
    return (
      <Section title={t("capture.title")}>
        <Row title={t("capture.loading.title")}>
          <LoaderCircle aria-hidden="true" className="animate-spin motion-reduce:animate-none" />
        </Row>
      </Section>
    );
  }

  const primary = primaryOf(snapshot.nextStep);
  const canToggle = snapshot.rung !== "desktop" && snapshot.shizuku.supported;
  const enabled = snapshot.health.state !== "disabled";
  const action =
    primary === "none"
      ? undefined
      : () =>
          run.mutate(
            primary === "recheck" ? () => captureRefresh() : () => captureArm(),
          );
  const actionLabel =
    primary === "arm"
      ? t("capture.setup.action.arm")
      : primary === "permission"
        ? t("capture.setup.action.permission")
        : t("capture.setup.action.checkAgain");

  return (
    <Section title={t("capture.title")}>
      <Row title={snapshot.headline} description={snapshot.detail ?? undefined}>
        {action ? (
          <Button disabled={run.isPending} onClick={action}>
            <RotateCw
              aria-hidden="true"
              className={run.isPending ? "animate-spin motion-reduce:animate-none" : undefined}
            />
            {run.isPending ? t("capture.setup.action.busy") : actionLabel}
          </Button>
        ) : (
          <Badge variant={snapshot.health.state === "working" ? "ok" : "secondary"}>
            {snapshot.health.state === "working" ? "On" : "Off"}
          </Badge>
        )}
      </Row>
      {canToggle && (
        <Row
          title={t("capture.setup.enable.title")}
          description={t("capture.setup.enable.body")}
        >
          <Switch
            aria-label={t("capture.setup.enable.title")}
            checked={enabled}
            disabled={run.isPending}
            onCheckedChange={(next) => run.mutate(() => captureSetEnabled(next))}
          />
        </Row>
      )}
    </Section>
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
  return (
    <Row title={title} description={description} badge={badge} note={note}>
      <Switch
        id={id}
        aria-label={title}
        checked={checked}
        disabled={disabled}
        onCheckedChange={onChange}
      />
    </Row>
  );
}
