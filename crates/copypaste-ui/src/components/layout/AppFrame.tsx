import type { ReactNode } from "react";

import { cn } from "@/lib/cn";
import styles from "./AppFrame.module.css";

export function AppFrame({ navigation, children, layout = "expanded", desktop = false, className }: { navigation: ReactNode; children: ReactNode; layout?: "expanded" | "compact"; desktop?: boolean; className?: string }) {
  return (
    <div className={cn(styles.root, styles[layout], desktop && styles.desktop, className)} data-layout={layout}>
      <div className={styles.navigation}>{navigation}</div>
      <main
        data-content-pane
        className={cn(styles.content, desktop && styles.desktopContent)}
      >
        {children}
      </main>
    </div>
  );
}
