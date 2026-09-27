import { SettingsRow } from "@/components/shared";
import { Button } from "@/components/ui";
import { capturePresentationOf } from "@/features/capture/model";
import { useCaptureState } from "@/hooks/useCapture";
import { useTranslation } from "@/i18n";
import { SettingsHealthNotice } from "@/features/settings/patterns/SettingsHealthNotice";
import { currentPlatform } from "@/lib/platform";
import { useUi } from "@/store/ui";
import { settingsCapabilities } from "@/features/settings/model/settingsNavigation";
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

function ScopedServiceSettings({ scope, revealAdvancedKey }: { scope: ServiceScope; revealAdvancedKey?: string }) {
  const showClipboard = scope === "all" || scope === "clipboard";
  const showPrivacy = scope === "all" || scope === "privacy";
  const showAdvanced = scope === "all" || scope === "advanced";
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

  return (
    <SettingsRow
      title={t("capture.title")}
      description={description}
      note={presentation.tone === "danger" ? snapshot.headline : undefined}
    >
      <Button
        type="button"
        variant="secondary"
        size="sm"
        onClick={() => openOnboardingAt("capture")}
      >
        {t("capture.status.open")}
      </Button>
    </SettingsRow>
  );
}

export function ClipboardServiceSettings({ revealAdvancedKey }: { revealAdvancedKey?: string }) {
  return <ScopedServiceSettings scope="clipboard" revealAdvancedKey={revealAdvancedKey} />;
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
