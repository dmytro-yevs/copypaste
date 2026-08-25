import { ActionButton } from "@/components/shared";
import {
  Button,
  ControlShortcut,
  Surface,
  TooltipContent,
  TooltipPortal,
  TooltipRoot,
  TooltipTrigger,
} from "@/components/ui";
import { ClipCardBody } from "@/features/history/components/ClipCardBody";
import { HighlightedCode } from "@/features/history/components/HighlightedCode";
import { SourceMeta } from "@/features/history/components/SourceMeta";
import { SourceMetaBadge } from "@/features/history/components/SourceMetaBadge";
import type { OriginDevice } from "@/features/history/model/origin";
import { ClipImageLoader } from "@/features/history/patterns/ClipImageLoader";
import { clipSourceMetadata } from "@/features/history/model";
import { type Item } from "@/lib/ipc";
import { t } from "@/i18n";
import { kindOf } from "@/lib/format";
import styles from "./QuickPasteRow.module.css";

function label(item: Item) {
  if (item.is_sensitive) return t("quickPaste.row.sensitive");
  if (kindOf(item) === "image") return t("quickPaste.row.image");
  if (item.content_type.toLowerCase() === "file") return t("quickPaste.row.file");
  if (item.sensitive_finding) {
    return item.sensitive_finding.redacted_preview.trim() || t("quickPaste.row.empty");
  }
  return item.content?.trim() || t("quickPaste.row.empty");
}

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
  onCopy: () => void;
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
  onCopy,
  onTogglePin,
}: QuickPasteRowProps) {
  const kind = kindOf(item);
  const image = kind === "image";
  const text = label(item);
  const source = clipSourceMetadata(item);
  const potentiallySensitive = !item.is_sensitive && item.sensitive_finding !== null;
  const canPreview = !item.is_sensitive && !potentiallySensitive;
  const cardContent = potentiallySensitive
    ? item.sensitive_finding?.redacted_preview ?? ""
    : item.content ?? "";
  const preview = image ? (
    <ClipImageLoader id={item.id} size="detail" />
  ) : item.truncated ? (
    fullContentFailed ? (
      <p role="status">{t("quickPaste.row.fullUnavailable")}</p>
    ) : fullContent === null ? (
      <p role="status">{t("quickPaste.row.fullLoading")}</p>
    ) : kind === "code" || kind === "json" ? (
      <HighlightedCode content={fullContent} kind={kind} mode="expanded" />
    ) : (
      <pre>{fullContent}</pre>
    )
  ) : kind === "code" || kind === "json" ? (
    <HighlightedCode content={text} kind={kind} mode="expanded" />
  ) : (
    <pre>{text}</pre>
  );

  const copyButton = (
    <Button
      type="button"
      variant="ghost"
      size="sm"
      tabIndex={active ? 0 : -1}
      onPointerDown={(event) => {
        if (event.button !== 0) return;
        event.preventDefault();
        onCopy();
      }}
      onClick={(event) => {
        if (event.detail === 0) onCopy();
      }}
      aria-label={`${t("quickPaste.row.copyPrefix")} ${image ? t("quickPaste.row.image") : text}`}
      className={styles.hit}
    />
  );

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
      {canPreview ? <TooltipTrigger asChild>{copyButton}</TooltipTrigger> : copyButton}
      <div className={styles.content}>
        <div className={styles.body}>
          <ClipCardBody
            kind={kind}
            masked={item.is_sensitive}
            content={cardContent}
            previewLines={previewLines}
            imagePreview={image ? <ClipImageLoader id={item.id} size="fill" /> : undefined}
          />
        </div>
        <SourceMeta
          item={item}
          source={source}
          origin={origin}
          kind={kind}
          content={cardContent}
          density="compact"
          devicePresentation="label"
          extras={
            <>
              {item.pinned ? <SourceMetaBadge icon="pin" label={t("quickPaste.row.pinned")} /> : null}
              {potentiallySensitive ? (
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
      {shortcut !== null ? (
        <ControlShortcut aria-hidden="true" className={styles.shortcut}>
          {shortcut}
        </ControlShortcut>
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
