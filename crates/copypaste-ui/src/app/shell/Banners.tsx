/** The unrecoverable key state needs a persistent, screen-wide explanation.
 * Service state is intentionally handled by the compact footer affordance. */
import { Icon } from "@/components/ui/icon";

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
    <div
      role="alert"
      data-banner={banner.id}
      className={styles.bar}
    >
      <div className={styles.layout}>
        <Icon name="alert" size="sm" className={styles.icon} />
        <span className={styles.message}>{banner.message}</span>
      </div>
    </div>
  );
}
