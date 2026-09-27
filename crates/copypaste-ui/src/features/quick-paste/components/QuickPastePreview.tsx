import { ClipBodyPreview, PreviewSurface } from "@/components/shared";
import { ClipImageLoader } from "@/features/clip-content";
import { resolveClipBodyPresentation } from "@/lib/clipPresentation";
import { kindOf } from "@/lib/format";
import type { Item, QuickPastePreviewLayout } from "@/lib/ipc";
import { t } from "@/i18n";
import styles from "./QuickPastePreview.module.css";

export function QuickPastePreview({
  item,
  fullContent,
  fullContentFailed,
  layout,
}: {
  item: Item;
  fullContent: string | null;
  fullContentFailed: boolean;
  layout: QuickPastePreviewLayout;
}) {
  const kind = kindOf(item);
  const body = resolveClipBodyPresentation({
    item,
    fullContent,
    fullContentFailed,
    revealedContent: null,
  });
  const loading = item.truncated && body.state === "content" && body.source === "preview";

  return (
    <aside
      className={styles.pane}
      data-side={layout.side}
      aria-label={t("quickPaste.preview.label")}
      style={{ inlineSize: `${layout.width}px` }}
    >
      <PreviewSurface elevation="raised" border="subtle" radius="lg" padding="compact" scroll className={styles.surface}>
        {body.state === "unavailable" ? (
          <p role="status">{t("quickPaste.row.fullUnavailable")}</p>
        ) : loading ? (
          <p role="status">{t("quickPaste.row.fullLoading")}</p>
        ) : body.state === "content" ? (
          <ClipBodyPreview
            kind={kind}
            content={body.content}
            previewLines={3}
            imagePreview={kind === "image" ? <ClipImageLoader id={item.id} size="fill" /> : undefined}
          />
        ) : null}
      </PreviewSurface>
    </aside>
  );
}
