/** The unrecoverable key state needs a persistent, screen-wide explanation.
 * Service state is intentionally handled by the compact footer affordance. */
import { CircleAlert } from "lucide-react";

import { pickBanner } from "@/lib/banners";
import type { BannerConditions } from "@/lib/banners";

interface BannerBarProps {
  conditions: BannerConditions;
}

export function BannerBar({ conditions }: BannerBarProps) {
  const banner = pickBanner(conditions);
  if (!banner) return null;

  return (
    <div
      role="alert"
      data-banner={banner.id}
      className="flex shrink-0 items-center gap-s-2 border-b border-err/20 bg-err/15 px-s-4 py-s-2 text-sm text-err-strong"
    >
      <CircleAlert size={16} aria-hidden="true" className="shrink-0" />
      <span className="min-w-0 flex-1">{banner.message}</span>
    </div>
  );
}
