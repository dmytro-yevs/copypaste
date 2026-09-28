import type { ReactNode } from "react";

import { HelpPopover } from "./HelpPopover";
import { StateView } from "./StateView";
import { useTranslation } from "@/i18n";

interface IllustratedErrorStateProps {
  title: string;
  body: string;
  actions: ReactNode;
  compact?: boolean;
  className?: string;
}

/** Temporary export for the shared barrel until all artwork references are removed. */
export function RepairBotArtwork() { return null; }

/** Compatibility adapter while feature callers move to StateView. */
export function IllustratedErrorState({ title, body, actions, compact = false, className }: IllustratedErrorStateProps) {
  const { t } = useTranslation();
  return <StateView
    mode="error"
    placement={compact ? "panel" : "screen"}
    title={<>{title}<HelpPopover content={body} label={t("common.errorDetails")} /></>}
    actions={actions}
    className={className}
  />;
}
