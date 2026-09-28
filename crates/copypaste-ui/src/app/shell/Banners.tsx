/** The unrecoverable key state needs a persistent, screen-wide explanation.
 * Service state is intentionally handled by the compact footer affordance. */
import { StateView } from "@/components/shared/StateView";

import { pickBanner } from "@/lib/banners";
import type { BannerConditions } from "@/lib/banners";
import styles from "./Banners.module.css";

interface BannerBarProps {
  conditions: BannerConditions;
}

export function BannerBar({ conditions }: BannerBarProps) {
  const banner = pickBanner(conditions);
  if (!banner) return null;

  return (
    <StateView
      mode="error"
      placement="inline"
      role="alert"
      data-banner={banner.id}
      className={styles.bar}
      description={banner.message}
    />
  );
}
