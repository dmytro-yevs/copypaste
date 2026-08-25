import { useCallback, useEffect, useId, useRef, useState } from "react";
import { Icon } from "@/components/ui/icon";
import { ActionButton, FieldFeedback } from "@/components/shared";
import {
  Button,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  Input,
  Label,
  VisuallyHidden,
} from "@/components/ui";
import { useRenameDevice } from "@/hooks/useDevices";
import { statusDeviceName, useStatus } from "@/hooks/useStatus";
import { useTranslation } from "@/i18n";
import styles from "./DeviceNameField.module.css";

export function DeviceNameField({
  showCurrentName = false,
  inlineTitle = false,
  descriptionId,
}: {
  showCurrentName?: boolean;
  inlineTitle?: boolean;
  descriptionId?: string;
}) {
  const { t } = useTranslation();
  const inputId = useId();
  const errorId = useId();
  const triggerId = useId();
  const status = useStatus(statusDeviceName);
  const rename = useRenameDevice();
  const triggerRef = useRef<HTMLButtonElement>(null);
  const inlineInputRef = useRef<HTMLInputElement>(null);
  const wasInlineEditing = useRef(false);
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const current = status.data ?? "";
  const next = name.trim();

  useEffect(() => {
    if (!rename.isPending) setName(current);
  }, [current, rename.isPending]);

  const disabled = rename.isPending || status.isPending || status.isError;
  const unchanged = !next || next === current;
  const displayedName = status.isPending
    ? "Checking…"
    : status.isError
      ? "Unavailable"
      : current || t("common.noValue");
  const focusTrigger = useCallback(() => {
    (triggerRef.current ?? document.getElementById(triggerId))?.focus();
  }, [triggerId]);

  useEffect(() => {
    if (inlineTitle && open) inlineInputRef.current?.focus();
  }, [inlineTitle, open]);

  useEffect(() => {
    if (inlineTitle && wasInlineEditing.current && !open) {
      focusTrigger();
    }
    wasInlineEditing.current = open;
  }, [focusTrigger, inlineTitle, open]);

  if (inlineTitle) {
    return open ? (
      <form
        className={styles.inlineEditor}
        onSubmit={(event) => {
          event.preventDefault();
          if (unchanged) return;
          rename.mutate(next, {
            onSuccess: () => setOpen(false),
            onError: () => setName(current),
          });
        }}
      >
        <Input
          size="sm"
          ref={inlineInputRef}
          data-device-name-inline-editor="true"
          value={name}
          maxLength={128}
          disabled={disabled}
          aria-label={t("devices.own.rename.label")}
          onChange={(event) => setName(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Escape") {
              event.preventDefault();
              event.stopPropagation();
              setName(current);
              setOpen(false);
              requestAnimationFrame(focusTrigger);
            }
          }}
        />
        <Button type="submit" size="sm" disabled={disabled || unchanged}>Save</Button>
        <Button type="button" size="sm" variant="ghost" disabled={rename.isPending} onClick={() => { setName(current); setOpen(false); requestAnimationFrame(focusTrigger); }}>Cancel</Button>
      </form>
    ) : (
      <span className={styles.inlineTitle}>
        <h2>{displayedName}</h2>
        <ActionButton
          ref={triggerRef}
          id={triggerId}
          type="button"
          size="compactIcon"
          variant="ghost"
          icon="pencil"
          aria-label={t("devices.own.rename.open")}
          title={t("devices.own.rename.open")}
          disabled={disabled}
          onClick={() => { rename.reset(); setName(current); setOpen(true); }}
        />
      </span>
    );
  }

  return (
    <div className={showCurrentName ? styles.currentName : undefined}>
      <Dialog
        open={open}
        onOpenChange={(nextOpen) => {
          if (rename.isPending) return;
          setOpen(nextOpen);
        }}
      >
        <ActionButton
          ref={triggerRef}
          id={triggerId}
          type="button"
          size={showCurrentName ? "sm" : "icon"}
          variant={showCurrentName ? "secondary" : "ghost"}
          icon="pencil"
          aria-label={t("devices.own.rename.open")}
          aria-describedby={descriptionId}
          title={t("devices.own.rename.open")}
          disabled={disabled}
          aria-busy={rename.isPending || undefined}
          onClick={() => {
            rename.reset();
            setOpen(true);
          }}
        >
          {showCurrentName ? "Rename" : null}
        </ActionButton>
        <DialogContent
          onCloseAutoFocus={(event) => {
            event.preventDefault();
            focusTrigger();
          }}
        >
        <DialogHeader>
          <DialogTitle>{t("devices.own.rename.title")}</DialogTitle>
          <DialogDescription>
            {t("devices.own.rename.description")}
          </DialogDescription>
        </DialogHeader>
        <form
          className={styles.form}
          onSubmit={(event) => {
            event.preventDefault();
            if (unchanged) return;
            rename.mutate(next, {
              onSuccess: () => setOpen(false),
              onError: () => setName(current),
            });
          }}
        >
          <div className={styles.field}>
            <Label htmlFor={inputId}>{t("devices.own.rename.label")}</Label>
            <Input
              size="sm"
              id={inputId}
              value={name}
              maxLength={128}
              disabled={disabled}
              aria-invalid={rename.isError || undefined}
              aria-errormessage={rename.isError ? errorId : undefined}
              onChange={(event) => {
                if (rename.isError) rename.reset();
                setName(event.target.value);
              }}
            />
            {rename.isError ? (
              <FieldFeedback id={errorId} state="error">
                Name wasn’t changed. Try again.
              </FieldFeedback>
            ) : null}
          </div>
          <DialogFooter>
            <Button
              type="button"
              variant="ghost"
              disabled={rename.isPending}
              onClick={() => {
                setName(current);
                setOpen(false);
              }}
            >
              {t("common.cancel")}
            </Button>
            <Button
              type="submit"
              disabled={disabled || unchanged}
              aria-busy={rename.isPending || undefined}
            >
              {rename.isPending ? (
                <>
                  <Icon name="spinner"
                    className={styles.spinner}
                    aria-hidden="true"
                  />
                  {t("devices.own.rename.saving")}
                </>
              ) : (
                t("devices.own.rename.action")
              )}
            </Button>
          </DialogFooter>
          <VisuallyHidden role="status" aria-live="polite">
            {rename.isPending ? t("devices.own.rename.saving") : ""}
          </VisuallyHidden>
        </form>
        </DialogContent>
      </Dialog>
    </div>
  );
}
