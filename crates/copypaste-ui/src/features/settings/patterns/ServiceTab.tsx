import { ServiceCaptureControls } from "@/features/settings/patterns/ServiceCaptureControls";
import { SettingsHealthNotice } from "@/features/settings/patterns/SettingsHealthNotice";
import { AdvancedServiceSection } from "./service/AdvancedServiceSection";
import {
  ClipboardCaptureSection,
  ClipboardNotificationSection,
} from "./service/ClipboardServiceSections";
import { PrivacyServiceSections } from "./service/PrivacyServiceSections";
import { ServiceRestartNotice } from "./service/ServiceRestartNotice";
import { ServiceSettingsProvider } from "./service/ServiceSettingsController";
import styles from "./ServiceTab.module.css";

type ServiceScope = "all" | "clipboard" | "privacy" | "advanced";

function ScopedServiceSettings({ scope }: { scope: ServiceScope }) {
  const showClipboard = scope === "all" || scope === "clipboard";
  const showPrivacy = scope === "all" || scope === "privacy";
  const showAdvanced = scope === "all" || scope === "advanced";

  return (
    <ServiceSettingsProvider requiresPrivateMode={showPrivacy}>
      <div className={styles.root}>
        {showClipboard ? <SettingsHealthNotice /> : null}
        <ServiceRestartNotice />
        {showClipboard ? <ServiceCaptureControls /> : null}
        {showClipboard ? <ClipboardCaptureSection /> : null}
        {showPrivacy ? <PrivacyServiceSections /> : null}
        {showClipboard ? <ClipboardNotificationSection /> : null}
        {showAdvanced ? <AdvancedServiceSection /> : null}
      </div>
    </ServiceSettingsProvider>
  );
}

export function ClipboardServiceSettings() {
  return <ScopedServiceSettings scope="clipboard" />;
}

export function PrivacyServiceSettings() {
  return <ScopedServiceSettings scope="privacy" />;
}

export function AdvancedServiceSettings() {
  return <ScopedServiceSettings scope="advanced" />;
}

export function ServiceTab() {
  return <ScopedServiceSettings scope="all" />;
}
