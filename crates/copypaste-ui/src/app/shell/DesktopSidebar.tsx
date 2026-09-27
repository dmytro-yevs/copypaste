import { BrandLockup } from "@/components/shared";
import { NavigationItem } from "@/app/shell/NavigationItem";
import { navigationRoutes } from "@/app/routes/routeMetadata";
import { useTranslation } from "@/i18n";
import { useUi } from "@/store/ui";
import styles from "./DesktopSidebar.module.css";

const ITEMS = navigationRoutes("sidebar");

export function DesktopSidebar({ navigationReady = true }: { navigationReady?: boolean }) {
  const { t } = useTranslation();
  const view = useUi((state) => state.view);
  const setView = useUi((state) => state.setView);
  return (
    <aside
      data-size-class="expanded"
      className={styles.sidebar}
      aria-label={t("nav.primary")}
    >
      <div data-tauri-drag-region className={styles.brand}>
        <BrandLockup />
      </div>
      <nav className={styles.navigation}>
        {ITEMS.map((item) => (
          <NavigationItem
            key={item.view}
            layout="sidebar"
            active={view === item.view}
            icon={item.icon}
            label={t(item.label)}
            disabled={!navigationReady}
            onClick={() => setView(item.view)}
          />
        ))}
      </nav>
    </aside>
  );
}
