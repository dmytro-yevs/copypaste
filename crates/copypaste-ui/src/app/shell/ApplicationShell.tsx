import { AppFrame } from "@/components/layout";
import { BannerBar } from "@/app/shell/Banners";
import { Boundary } from "@/app/shell/Boundary";
import { useSizeClass } from "@/hooks/useSizeClass";
import { useTranslation } from "@/i18n";
import type { ErrorKind } from "@/lib/errors";
import { isAndroidPlatform } from "@/lib/platform";
import { ScreenRouter } from "@/app/routes/ScreenRouter";
import { DesktopSidebar } from "./DesktopSidebar";
import { MobileDock } from "./MobileDock";
import styles from "./ApplicationShell.module.css";

export function ApplicationShell({ navigationReady, pushLive, statusKind }: { navigationReady: boolean; pushLive: boolean; statusKind: ErrorKind | null }) {
  const { t } = useTranslation();
  const compact = useSizeClass() === "compact";
  const navigation = compact ? (
    <div className={styles.mobileNavigation}>
      <MobileDock navigationReady={navigationReady} />
    </div>
  ) : (
    <DesktopSidebar navigationReady={navigationReady} />
  );
  return (
    <AppFrame layout={compact ? "compact" : "expanded"} desktop={!isAndroidPlatform()} navigation={
      <Boundary label={t("shell.boundary.navigation")}>{navigation}</Boundary>
    }>
      <BannerBar conditions={{ historyUnreadable: statusKind === "key_unusable" ? statusKind : null }} />
      <ScreenRouter pushLive={pushLive} />
    </AppFrame>
  );
}
