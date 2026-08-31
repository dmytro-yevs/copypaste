import { useTranslation } from "@/i18n";
import { useUi } from "@/store/ui";
import { navigationRoutes } from "@/app/routes/routeMetadata";
import { NavigationItem } from "@/app/shell/NavigationItem";
import styles from "./MobileDock.module.css";

const ITEMS = navigationRoutes("dock");

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
