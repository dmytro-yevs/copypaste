import * as TooltipPrimitive from "@radix-ui/react-tooltip";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { RotateCcw, TriangleAlert } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { Button } from "@/components/ui/button";
import { Row } from "@/components/settings/Row";
import { Section } from "@/components/settings/Section";
import { SwitchRow } from "@/components/settings/SwitchRow";
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

const SHORTCUT_KEY = ["shortcut"] as const;
const DEFAULT_SHORTCUT_KEY = ["default-shortcut"] as const;

function Keycaps({ accelerator }: { accelerator: string }) {
  return (
    <span aria-hidden="true" className="flex items-center gap-1">
      {acceleratorGlyphs(accelerator).map((glyph, index) => (
        <kbd
          key={`${glyph}-${index}`}
          className="inline-flex h-6 min-w-6 items-center justify-center rounded-sm border border-border bg-muted px-1.5 font-sans text-xs"
        >
          {glyph}
        </kbd>
      ))}
    </span>
  );
}

export function ShortcutTab() {
  const { t } = useTranslation();
  const qc = useQueryClient();
  const [capturing, setCapturing] = useState(false);
  const [refusal, setRefusal] = useState<string | null>(null);
  const [saved, setSaved] = useState<"saved" | "reset" | null>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);

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
  const resetDisabled = bound === fallback || save.isPending;
  const unavailable =
    (current.error !== null && isUnavailable(current.error)) ||
    (save.error !== null && isUnavailable(save.error));

  useEffect(() => {
    if (!capturing) return;

    function onKeyDown(event: KeyboardEvent) {
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
          if (result.value !== bound) save.mutate(result.value);
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
  const resetButton = (
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label={t("settings.shortcut.reset")}
      disabled={resetDisabled}
      onClick={() => save.mutate(fallback)}
    >
      <RotateCcw aria-hidden="true" />
    </Button>
  );

  return (
    <div className="flex flex-col">
      <Row
        title={t("settings.shortcut.title")}
        description={t("settings.shortcut.description")}
      >
        <div className="flex items-center gap-s-2">
          <button
            ref={buttonRef}
            type="button"
            aria-label={accessibleName}
            title={accessibleName}
            onClick={() => {
              setRefusal(null);
              setCapturing((was) => !was);
            }}
            onBlur={() => setCapturing(false)}
            className={cn(
              "flex h-[var(--tap-min)] min-w-[160px] items-center justify-center gap-2 rounded-md border px-s-3 text-sm outline-none focus-visible:ring-[3px] focus-visible:ring-ring",
              capturing
                ? "border-ring bg-selected"
                : "border-border-strong bg-background hover:bg-accent",
            )}
          >
            {capturing ? (
              <span className="text-muted-foreground">
                {t("settings.shortcut.capturingPrompt")}
              </span>
            ) : (
              <Keycaps accelerator={bound} />
            )}
          </button>

          <TooltipPrimitive.Provider>
            <TooltipPrimitive.Root disableHoverableContent>
              <TooltipPrimitive.Trigger asChild>
                {resetDisabled ? (
                  <span className="inline-flex">{resetButton}</span>
                ) : (
                  resetButton
                )}
              </TooltipPrimitive.Trigger>
              <TooltipPrimitive.Portal>
                <TooltipPrimitive.Content
                  sideOffset={8}
                  collisionPadding={8}
                  className="pointer-events-none z-[var(--z-popover)] whitespace-nowrap rounded-md border border-border-strong bg-popover px-s-2 py-s-1 text-xs text-popover-foreground shadow-lg motion-safe:animate-in motion-safe:fade-in-0 motion-safe:zoom-in-95"
                >
                  {t("settings.shortcut.reset")}
                </TooltipPrimitive.Content>
              </TooltipPrimitive.Portal>
            </TooltipPrimitive.Root>
          </TooltipPrimitive.Provider>
        </div>
      </Row>

      {refusal && (
        <p
          role="alert"
          className="flex items-start gap-2 border-b border-divider py-s-3 text-sm text-warn-strong"
        >
          <TriangleAlert size={16} aria-hidden="true" className="mt-px shrink-0" />
          {refusal}
        </p>
      )}

      {save.error !== null && !isUnavailable(save.error) && (
        <p role="alert" className="border-b border-divider py-s-3 text-sm text-err-strong">
          {`${toFriendly(save.error)} ${t("settings.shortcut.saveFailed")}`}
        </p>
      )}

      {saved !== null && (
        <p role="status" aria-live="polite" className="border-b border-divider py-s-3 text-sm text-ok-strong">
          {t(saved === "reset" ? "settings.shortcut.resetSaved" : "settings.shortcut.saved")}
        </p>
      )}

      {unavailable && (
        <p className="py-s-3 text-sm text-muted-foreground">
          {t("settings.shortcut.unavailable", { accelerator: fallback })}
        </p>
      )}

      <StartupSection />
    </div>
  );
}

function StartupSection() {
  const { t } = useTranslation();
  const openAtLogin = useOpenAtLogin();
  const save = useSetOpenAtLogin();

  const enabled = openAtLogin.data ?? false;
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
        disabled={openAtLogin.isPending || save.isPending}
        note={
          blocked ? (
            <span role="status" className="text-xs text-warn-strong">
              {t("settings.startup.blocked")}
            </span>
          ) : save.isError ? (
            <span role="alert" className="text-xs text-err-strong">
              {t("settings.startup.failed")}
            </span>
          ) : undefined
        }
        onChange={(next) => save.mutate(next)}
      />
    </Section>
  );
}
