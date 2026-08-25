import { Icon } from "@/components/ui";
import type { SettingsTabIconName } from "@/features/settings/model/settingsNavigation";
import styles from "./SettingsTabIcon.module.css";
const ICONS = {
  cloud: "cloud",
  palette: "palette",
  list: "list",
  keyboard: "keyboard",
  service: "shieldCheck",
  capture: "library",
  devices: "devices",
  storage: "storage",
  diagnostics: "stethoscope",
  help: "info",
} satisfies Record<SettingsTabIconName, Parameters<typeof Icon>[0]["name"]>;

export function SettingsTabIcon({ name }: { name: SettingsTabIconName }) {
  return <Icon name={ICONS[name]} size="sm" className={styles.icon} />;
}
