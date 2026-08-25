import { useTranslation } from "@/i18n";
import { type View, useUi } from "@/store/ui";
import { NavigationItem } from "@/app/shell/NavigationItem";
import type { IconName } from "@/components/ui";
import styles from "./MobileDock.module.css";

const ITEMS = [
  { view: "devices", label: "nav.devices", icon: "devices" },
  { view: "history", label: "nav.history", icon: "library" },
  { view: "settings", label: "nav.settings", icon: "settings" },
] as const satisfies ReadonlyArray<{ view: View; label: string; icon: IconName }>;

export function MobileDock({ navigationReady = true }: { navigationReady?: boolean }) {
  const { t } = useTranslation();
  const view = useUi((state) => state.view);
  const setView = useUi((state) => state.setView);

  return (
    <nav className={styles.dock} aria-label={t("nav.primary")}>
      {ITEMS.map((item) => (
        <NavigationItem
          key={item.view}
          layout="dock"
          active={view === item.view}
          icon={item.icon}
          label={t(item.label)}
          disabled={!navigationReady}
          onClick={() => setView(item.view)}
        />
      ))}
    </nav>
  );
}
