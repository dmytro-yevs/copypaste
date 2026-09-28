import { capturePresentationOf } from "@/features/capture/model";
import { useCaptureState } from "@/hooks/useCapture";
import { useTranslation } from "@/i18n";
import { SettingsHealthNotice } from "@/features/settings/patterns/SettingsHealthNotice";
import { currentPlatform } from "@/lib/platform";
import { useUi } from "@/store/ui";
import { settingsCapabilities } from "@/features/settings/model/settingsNavigation";
import { SettingsSchemaRenderer } from "@/features/settings/components/SettingsSchemaRenderer";
import { settingDefinition } from "@/features/settings/model/settingsSchemaCatalog";
import { AdvancedServiceSection } from "./service/AdvancedServiceSection";
import {
  ClipboardCaptureSection,
  ClipboardNotificationSection,
} from "./service/ClipboardServiceSections";
import { PrivacyServiceSections } from "./service/PrivacyServiceSections";
import { ServiceRestartNotice } from "./service/ServiceRestartNotice";
import { ServiceSettingsProvider } from "./service/ServiceSettingsController";
import styles from "./ServiceTab.module.css";

export type ServiceScope = "all" | "clipboard" | "privacy" | "device-sync";

export function ServiceSettings({ scope, revealAdvancedKey }: { scope: ServiceScope; revealAdvancedKey?: string }) {
  const showClipboard = scope === "all" || scope === "clipboard";
  const showPrivacy = scope === "all" || scope === "privacy";
  const showAdvanced = scope === "all" || scope === "device-sync";
  const supportsCopyNotifications = settingsCapabilities(currentPlatform()).copyNotifications;

  return (
    <ServiceSettingsProvider requiresPrivateMode={showPrivacy}>
      <div className={styles.root}>
        {showClipboard ? <SettingsHealthNotice /> : null}
        <ServiceRestartNotice />
        {showClipboard ? <CaptureSetupEntry /> : null}
        {showClipboard ? <ClipboardCaptureSection revealAdvancedKey={revealAdvancedKey} /> : null}
        {showPrivacy ? <PrivacyServiceSections /> : null}
        {showClipboard ? <ClipboardNotificationSection supportsCopyNotifications={supportsCopyNotifications} /> : null}
        {showAdvanced ? <AdvancedServiceSection /> : null}
      </div>
    </ServiceSettingsProvider>
  );
}

function CaptureSetupEntry() {
  const { t } = useTranslation();
  const capture = useCaptureState();
  const openOnboardingAt = useUi((state) => state.openOnboardingAt);
  const snapshot = capture.data;

  if (snapshot === undefined) return null;
  const presentation = capturePresentationOf(snapshot.health);
  const description = snapshot.detail
    ? `${snapshot.headline} ${snapshot.detail}`
    : snapshot.headline;

  return <SettingsSchemaRenderer groups={[{ id: "capture-status", fields: [{
    kind: "action", definition: settingDefinition("clipboard", "capture.title"),
    help: description, note: presentation.tone === "danger" ? snapshot.headline : undefined,
    label: t("capture.status.open"), onAction: () => openOnboardingAt("capture"),
  }] }]} />;
}
