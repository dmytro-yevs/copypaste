import type { ReactNode } from "react";

import { Surface } from "@/components/ui";
import styles from "./SettingsGroupSurface.module.css";

export function SettingsGroupSurface({ children }: { children: ReactNode }) {
  return (
    <Surface
      elevation="raised"
      border="subtle"
      radius="md"
      className={styles.root}
    >
      {children}
    </Surface>
  );
}
