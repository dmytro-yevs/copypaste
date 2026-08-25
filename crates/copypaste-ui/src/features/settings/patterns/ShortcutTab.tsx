import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useId, useRef, useState } from "react";

import { Button } from "@/components/ui";
import { ActionButton, FieldFeedback, SettingsRow } from "@/components/shared";
import { Section } from "@/features/settings/components/Section";
import { SettingsGroupSurface } from "@/features/settings/components/SettingsGroupSurface";
import { SwitchRow } from "@/features/settings/components/SwitchRow";
import {
  DEFAULT_SHORTCUT,
  acceleratorGlyphs,
  captureAccelerator,
} from "@/lib/accelerator";
import { useOpenAtLogin, useSetOpenAtLogin } from "@/hooks/useOpenAtLogin";
import { useTranslation } from "@/i18n";
import { cn } from "@/lib/cn";
import { isUnavailable, toFriendly } from "@/lib/errors";
import { getDefaultShortcut, getShortcut, setShortcut } from "@/lib/ipc";
import styles from "./ShortcutTab.module.css";

const SHORTCUT_KEY = ["shortcut"] as const;
const DEFAULT_SHORTCUT_KEY = ["default-shortcut"] as const;

function Keycaps({ accelerator }: { accelerator: string }) {
  return (
    <span aria-hidden="true" className={styles.keycaps}>
      {acceleratorGlyphs(accelerator).map((glyph, index) => (
        <kbd
          key={`${glyph}-${index}`}
          className={styles.keycap}
        >
          {glyph}
        </kbd>
      ))}
    </span>
  );
}

export function ShortcutTab({ supportsStartup }: { supportsStartup: boolean }) {
  const { t } = useTranslation();
  const qc = useQueryClient();
  const [capturing, setCapturing] = useState(false);
  const [refusal, setRefusal] = useState<string | null>(null);
  const [saved, setSaved] = useState<"saved" | "reset" | null>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const descriptionId = useId();
  const feedbackId = useId();

  const current = useQuery({
    queryKey: SHORTCUT_KEY,
    queryFn: getShortcut,
    retry: false,
    staleTime: Infinity,
  });
  const defaultShortcut = useQuery({
    queryKey: DEFAULT_SHORTCUT_KEY,
    queryFn: getDefaultShortcut,
    retry: false,
    staleTime: Infinity,
  });

  const fallback = defaultShortcut.data ?? DEFAULT_SHORTCUT;
  const bound = current.data ?? fallback;
  const save = useMutation({
    mutationFn: (accelerator: string) => setShortcut(accelerator),
    onSuccess: (_data, accelerator) => {
      qc.setQueryData(SHORTCUT_KEY, accelerator);
      setSaved(accelerator === fallback ? "reset" : "saved");
    },
  });
  const shortcutLoading = current.isPending || defaultShortcut.isPending;
  const unavailable =
    (current.error !== null && isUnavailable(current.error)) ||
    (save.error !== null && isUnavailable(save.error));
  const shortcutUnknown = current.error !== null;
  const resetDisabled =
    shortcutLoading ||
    shortcutUnknown ||
    defaultShortcut.isError ||
    unavailable ||
    bound === fallback ||
    save.isPending;

  useEffect(() => {
    if (!capturing) return;

    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Tab") {
        setCapturing(false);
        setRefusal(null);
        return;
      }
      event.preventDefault();
      event.stopPropagation();
      const result = captureAccelerator(event);
      switch (result.kind) {
        case "incomplete":
          return;
        case "cancelled":
          setCapturing(false);
          setRefusal(null);
          buttonRef.current?.blur();
          return;
        case "refused":
          setRefusal(result.reason);
          return;
        case "accelerator":
          setCapturing(false);
          setRefusal(null);
          if (result.value !== bound) {
            setSaved(null);
            save.mutate(result.value);
          }
          return;
      }
    }

    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [bound, capturing, save]);

  useEffect(() => {
    if (saved === null) return;
    const timeout = window.setTimeout(() => setSaved(null), 2500);
    return () => window.clearTimeout(timeout);
  }, [saved]);

  // A11Y-13: the raw accelerator, not the glyphs — screen readers handle
  // "CmdOrCtrl+Shift+V" and mangle "⌘⇧V" (CopyPaste-8ebg.53).
  const accessibleName = capturing
    ? t("settings.shortcut.capturingName")
    : t("settings.shortcut.current", { accelerator: bound });
  const feedback = shortcutLoading ? (
    <FieldFeedback state="pending">Checking shortcut…</FieldFeedback>
  ) : current.error !== null && !isUnavailable(current.error) ? (
    <FieldFeedback state="error">
      {toFriendly(current.error)}
    </FieldFeedback>
  ) : save.isPending ? (
    <FieldFeedback state="pending">Saving shortcut…</FieldFeedback>
  ) : refusal ? (
    <FieldFeedback state="error">{refusal}</FieldFeedback>
  ) : save.error !== null && !isUnavailable(save.error) ? (
    <FieldFeedback state="error">
      {`${toFriendly(save.error)} ${t("settings.shortcut.saveFailed")}`}
    </FieldFeedback>
  ) : saved !== null ? (
    <FieldFeedback state="success">
      {t(saved === "reset" ? "settings.shortcut.resetSaved" : "settings.shortcut.saved")}
    </FieldFeedback>
  ) : unavailable ? (
    <FieldFeedback state="neutral">
      {t("settings.shortcut.unavailable", { accelerator: fallback })}
    </FieldFeedback>
  ) : undefined;
  const resetButton = (
    <ActionButton
      variant="ghost"
      size="icon"
      icon="reset"
      disabled={resetDisabled}
      aria-label={t("settings.shortcut.reset")}
      aria-describedby={`${descriptionId}${feedback ? ` ${feedbackId}` : ""}`}
      aria-busy={save.isPending || undefined}
      title={t("settings.shortcut.reset")}
      onClick={() => {
        setSaved(null);
        save.mutate(fallback);
      }}
    />
  );

  return (
    <div className={styles.root}>
      <SettingsGroupSurface>
        <SettingsRow
          title={t("settings.shortcut.title")}
          descriptionId={descriptionId}
          description={t("settings.shortcut.description")}
          note={feedback ? <span id={feedbackId}>{feedback}</span> : undefined}
        >
          <div className={styles.controlRow}>
            <Button
              ref={buttonRef}
              type="button"
              variant="secondary"
              size="md"
              disabled={shortcutLoading || shortcutUnknown || unavailable || save.isPending}
              aria-label={accessibleName}
              aria-describedby={`${descriptionId}${feedback ? ` ${feedbackId}` : ""}`}
              aria-busy={save.isPending || undefined}
              title={accessibleName}
              onClick={() => {
                setRefusal(null);
                setCapturing((was) => !was);
              }}
              onBlur={() => {
                setCapturing(false);
                setRefusal(null);
              }}
              className={cn(
                styles.captureButton,
                capturing ? styles.capturing : styles.idle,
              )}
            >
              {capturing ? (
                <span className={styles.capturingPrompt}>
                  {t("settings.shortcut.capturingPrompt")}
                </span>
              ) : shortcutUnknown ? (
                <span className={styles.capturingPrompt}>Unavailable</span>
              ) : (
                <Keycaps accelerator={bound} />
              )}
            </Button>

            {resetButton}
          </div>
        </SettingsRow>
      </SettingsGroupSurface>

      {supportsStartup ? <StartupSection /> : null}
    </div>
  );
}

function StartupSection() {
  const { t } = useTranslation();
  const openAtLogin = useOpenAtLogin();
  const save = useSetOpenAtLogin();

  // A read that failed is not a setting that is off. Rendering `?? false` as an
  // ordinary unchecked switch invited the user to "turn on" something that may
  // already be on, and hid that the system never answered.
  const enabled = openAtLogin.data ?? false;
  const unknown = openAtLogin.isError;
  // The write succeeded and the system still disagrees — on Windows, a Startup
  // apps override. Silence here would leave a switch that flicks back with no
  // explanation.
  const blocked =
    save.isSuccess && save.variables === true && save.data === false;

  return (
    <Section title={t("settings.startup.title")}>
      <SwitchRow
        title={t("settings.startup.openAtLogin.title")}
        description={t("settings.startup.openAtLogin.description")}
        id="open-at-login"
        checked={enabled}
        disabled={openAtLogin.isPending || unknown || save.isPending}
        busy={openAtLogin.isPending || save.isPending}
        note={
          openAtLogin.isPending ? (
            <FieldFeedback state="pending">Checking startup setting…</FieldFeedback>
          ) : unknown ? (
            <FieldFeedback state="error">
              {t("settings.startup.unknown")}
            </FieldFeedback>
          ) : blocked ? (
            <FieldFeedback state="warning">
              {t("settings.startup.blocked")}
            </FieldFeedback>
          ) : save.isError ? (
            <FieldFeedback state="error">
              {t("settings.startup.failed")}
            </FieldFeedback>
          ) : undefined
        }
        onChange={(next) => save.mutate(next)}
      />
    </Section>
  );
}
