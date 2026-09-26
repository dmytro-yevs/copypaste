import { useLayoutEffect, useRef } from "react";

import {
  ActionButton,
  ClipBodyPreview,
  HighlightedCode,
  SourceMeta,
  SourceMetaBadge,
} from "@/components/shared";
import { ClipImageLoader } from "@/features/clip-content";
import {
    Button,
  iconComponent,
  ShortcutBadge,
  Surface,
  TooltipContent,
  TooltipPortal,
  TooltipRoot,
  TooltipTrigger,
} from "@/components/ui";
import type { OriginDevice } from "@/lib/itemOrigin";
import { resolveClipBodyPresentation } from "@/lib/clipPresentation";
import { clipSourceMetadata } from "@/lib/clipSourcePresentation";
import { SourceAppIcon } from "@/features/source-apps";
import { quickPastePresentation } from "@/features/quick-paste/model/quickPastePresentation";
import { clipboardCopyPresentation } from "@/features/history/model/clipPresentation";
import { useClipboardWriteAvailability } from "@/hooks/useClipboardWriteAvailability";
import { type Item } from "@/lib/ipc";
import { t } from "@/i18n";
import { kindOf } from "@/lib/format";
import styles from "./QuickPasteRow.module.css";

interface QuickPasteRowProps {
  item: Item;
  active: boolean;
  previewLines: number;
  shortcut: string | null;
  pinPending: boolean;
  origin: OriginDevice | null;
  fullContent: string | null;
  fullContentFailed: boolean;
  onSelect: () => void;
  onSelectFromKeyboard: () => void;
  onCopy: (plainText?: boolean) => void;
  onTogglePin: () => void;
}

export function QuickPasteRow({
  item,
  active,
  previewLines,
  shortcut,
  pinPending,
  origin,
  fullContent,
  fullContentFailed,
  onSelect,
  onSelectFromKeyboard,
  onCopy,
  onTogglePin,
}: QuickPasteRowProps) {
  const copyHitRef = useRef<HTMLButtonElement>(null);
  const restoreCopyFocusRef = useRef(false);
  const kind = kindOf(item);
  const availability = useClipboardWriteAvailability(item.content_type);
  const plainTextAvailability = useClipboardWriteAvailability(item.content_type, "plain_text");
  const copyAvailability = clipboardCopyPresentation(
    availability.isPending
      ? { status: "loading" }
      : availability.isError
        ? { status: "failed" }
        : { status: "resolved", availability: availability.data },
  );
  const plainTextCopyAvailability = clipboardCopyPresentation(
    plainTextAvailability.isPending
      ? { status: "loading" }
      : plainTextAvailability.isError
        ? { status: "failed" }
        : { status: "resolved", availability: plainTextAvailability.data },
  );
  useLayoutEffect(() => {
    if (!active) {
      restoreCopyFocusRef.current = false;
      return;
    }
    if (!copyAvailability.canCopy || !restoreCopyFocusRef.current) return;
    restoreCopyFocusRef.current = false;
    copyHitRef.current?.focus();
  }, [active, copyAvailability.canCopy]);
  const image = kind === "image";
  const { rowLabel } = quickPastePresentation(item);
  const source = clipSourceMetadata(item);
  const body = resolveClipBodyPresentation({
    item,
    fullContent,
    fullContentFailed,
    revealedContent: null,
  });
  const hasPotentialFinding = item.sensitive_finding !== null;
  const canPreview =
    body.state === "unavailable" ||
    (body.state === "content" && !hasPotentialFinding && kind !== "unknown");
  const cardContent =
    body.state === "content" && body.source === "redacted"
      ? body.content
      : body.state === "unavailable" || body.state === "masked"
        ? ""
        : kind === "unknown"
          ? rowLabel
          : item.content ?? "";
  const previewContent = body.state === "content" ? body.content : "";
  const preview = image ? (
    <ClipImageLoader id={item.id} size="detail" />
  ) : body.state === "unavailable" ? (
    <p role="status">{t("quickPaste.row.fullUnavailable")}</p>
  ) : item.truncated && body.state === "content" && body.source === "preview" ? (
    <p role="status">{t("quickPaste.row.fullLoading")}</p>
  ) : kind === "code" || kind === "json" ? (
    <HighlightedCode content={previewContent} kind={kind} mode="expanded" />
  ) : (
    <pre>{previewContent}</pre>
  );

  const copyButton = (
    <Button
      ref={copyHitRef}
      type="button"
      variant="ghost"
      size="sm"
      tabIndex={active ? 0 : -1}
      disabled={!copyAvailability.canCopy}
      onPointerDown={(event) => {
        if (!copyAvailability.canCopy || event.button !== 0) return;
        event.preventDefault();
        onCopy(false);
      }}
      onKeyDown={(event) => {
        if (!copyAvailability.canCopy || event.key !== "Enter") return;
        event.preventDefault();
        event.stopPropagation();
        onCopy(event.altKey);
      }}
      onClick={(event) => {
        if (copyAvailability.canCopy && event.detail === 0) onCopy(false);
      }}
      aria-label={`${t("quickPaste.row.copyPrefix")} ${image ? t("quickPaste.row.image") : rowLabel}`}
      className={styles.hit}
    />
  );

  const selectButton = (
    <Button
      type="button"
      variant="ghost"
      size="sm"
      tabIndex={active ? 0 : -1}
      onFocus={() => { restoreCopyFocusRef.current = true; }}
      onBlur={() => { restoreCopyFocusRef.current = false; }}
      onPointerUp={(event) => {
        if (event.button === 0) onSelect();
      }}
      onKeyDown={(event) => {
        if (event.key === "Enter") {
          event.preventDefault();
          event.stopPropagation();
          onSelectFromKeyboard();
        } else if (event.key === " ") {
          event.stopPropagation();
        }
      }}
      onClick={(event) => {
        if (event.detail === 0) onSelectFromKeyboard();
      }}
      aria-label={`${t("quickPaste.row.selectPrefix")} ${image ? t("quickPaste.row.image") : rowLabel}`}
      className={`${styles.hit} ${styles.selectHit}`}
    />
  );
  const activeHit = copyAvailability.canCopy ? copyButton : selectButton;

  const row = (
    <Surface
      role="listitem"
      elevation="raised"
      border="subtle"
      radius="md"
      aria-current={active || undefined}
      data-state={active ? "selected" : "idle"}
      data-kind={kind}
      onMouseEnter={onSelect}
      className={styles.root}
    >
      {!copyAvailability.canCopy ? copyButton : null}
      {canPreview ? <TooltipTrigger asChild>{activeHit}</TooltipTrigger> : activeHit}
      <div className={styles.content}>
        <div className={styles.body}>
          <ClipBodyPreview
            kind={kind}
            masked={body.state === "masked"}
            content={cardContent}
            previewLines={previewLines}
            imagePreview={image ? <ClipImageLoader id={item.id} size="fill" /> : undefined}
          />
        </div>
        {active && copyAvailability.reason !== null ? (
          <div className={styles.availability} role="status">
            <span>{copyAvailability.reason}</span>
            {copyAvailability.canRetry ? (
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onFocus={() => { restoreCopyFocusRef.current = true; }}
                onBlur={() => { restoreCopyFocusRef.current = false; }}
                onClick={() => void availability.refetch()}
              >
                {t("history.copyAvailability.retry")}
              </Button>
            ) : null}
          </div>
        ) : null}
        {active && copyAvailability.canCopy && plainTextCopyAvailability.reason !== null ? (
          <div className={styles.availability} role="status">
            <span>{t("quickPaste.row.plainTextPrefix")}: {plainTextCopyAvailability.reason}</span>
            {plainTextCopyAvailability.canRetry ? (
              <Button type="button" variant="ghost" size="sm" onClick={() => void plainTextAvailability.refetch()}>
                {t("history.copyAvailability.retry")}
              </Button>
            ) : null}
          </div>
        ) : null}
        <SourceMeta
          source={source}
          createdAt={item.created_at}
          sourceIcon={
            <SourceAppIcon
              bundleId={item.source_app_bundle_id}
              Fallback={iconComponent(source.icon)}
              fallbackText={source.label.slice(0, 2)}
              size="xs"
            />
          }
          origin={origin}
          kind={kind}
          content={cardContent}
          density="compact"
          devicePresentation="label"
          extras={
            <>
              {item.pinned ? <SourceMetaBadge icon="pin" label={t("quickPaste.row.pinned")} /> : null}
              {hasPotentialFinding ? (
                <SourceMetaBadge
                  icon="sensitive"
                  tone="warning"
                  label={t("quickPaste.row.potentialSensitive")}
                />
              ) : null}
            </>
          }
        />
      </div>
      {shortcut !== null && copyAvailability.canCopy ? (
        <ShortcutBadge aria-hidden="true" className={styles.shortcut}>
          {shortcut}
        </ShortcutBadge>
      ) : null}
      <ActionButton
        variant="ghost"
        size="compactIcon"
        icon={item.pinned ? "unpin" : "pin"}
        aria-pressed={item.pinned}
        aria-label={t(item.pinned ? "quickPaste.row.unpin" : "quickPaste.row.pin")}
        title={t(item.pinned ? "quickPaste.row.unpin" : "quickPaste.row.pin")}
        disabled={pinPending}
        onClick={onTogglePin}
        className={styles.pinAction}
      />
    </Surface>
  );

  if (!canPreview) return row;

  return (
    <TooltipRoot delayDuration={220}>
      {row}
      <TooltipPortal>
        <TooltipContent
          side="bottom"
          align="center"
          sideOffset={8}
          collisionPadding={12}
          className={styles.previewPopup}
        >
          {preview}
        </TooltipContent>
      </TooltipPortal>
    </TooltipRoot>
  );
}
