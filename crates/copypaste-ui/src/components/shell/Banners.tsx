/** A11Y-5: severity chooses the role, so nothing informational interrupts a
 *  screen reader and nothing urgent waits its turn. */
import { CircleAlert, TriangleAlert, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { useTranslation } from "@/i18n";
import { cn } from "@/lib/cn";
import { pickBanner } from "@/lib/banners";
import type { BannerConditions } from "@/lib/banners";
import { type BannerId, useUi } from "@/store/ui";

interface BannerBarProps {
  conditions: Omit<BannerConditions, "dismissed">;
  onRetry: () => void;
}

export function BannerBar({ conditions, onRetry }: BannerBarProps) {
  const { t } = useTranslation();
  const dismissed = useUi((s) => s.dismissed);
  const dismiss = useUi((s) => s.dismiss);

  const banner = pickBanner({ ...conditions, dismissed });
  if (!banner) return null;

  const Icon = banner.severity === "error" ? CircleAlert : TriangleAlert;

  return (
    <div
      role={banner.severity === "info" ? "status" : "alert"}
      data-banner={banner.id}
      className={cn(
        "flex shrink-0 flex-wrap items-center gap-s-2 border-b px-s-4 py-s-2 text-sm",
        banner.severity === "error"
          ? "border-err/20 bg-err/15 text-err-strong"
          : "border-warn/20 bg-warn/15 text-warn-strong",
      )}
    >
      <Icon size={16} aria-hidden="true" className="shrink-0" />
      <span className="min-w-0 flex-1">{banner.message}</span>

      {banner.action === "retry" && (
        <Button size="sm" variant="outline" onClick={onRetry}>
          {t("common.tryAgain")}
        </Button>
      )}

      {banner.dismissible && (
        <Button
          size="icon-sm"
          variant="ghost"
          aria-label={t("common.dismiss")}
          title={t("common.dismiss")}
          onClick={() => dismiss(banner.id as BannerId)}
        >
          <X aria-hidden="true" />
        </Button>
      )}
    </div>
  );
}
