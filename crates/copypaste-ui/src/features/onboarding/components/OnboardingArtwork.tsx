import { Icon, type IconName } from "@/components/ui/icon";
import { useId } from "react";

import styles from "./OnboardingArtwork.module.css";











export function WelcomeArtwork() {
  return (
    <div className={styles.welcome} aria-hidden="true">
      <FloatingClip className={styles.clipBack} source="Safari · just now">
        Research notes for the new product direction…
      </FloatingClip>
      <FloatingClip className={styles.clipMiddle} source="Messages · 2m">
        Meet at 17:30 near the studio.
      </FloatingClip>
      <FloatingClip className={styles.clipFront} source="Figma · 4m">
        Memory Stream — selected direction
      </FloatingClip>
    </div>
  );
}

function FloatingClip({
  className,
  source,
  children,
}: {
  className: string;
  source: string;
  children: string;
}) {
  return (
    <article className={`${styles.floatingClip} ${className}`}>
      <span className={styles.floatingTitle}>{source}</span>
      <p>{children}</p>
    </article>
  );
}

export function CaptureArtwork() {
  const gradientId = `onboarding-capture-${useId().replace(/:/g, "")}`;

  return (
    <div className={styles.captureScene} aria-hidden="true">
      <svg className={styles.flow} viewBox="0 0 330 330">
        <defs>
          <linearGradient id={gradientId} x1="0" y1="0" x2="1" y2="1">
            <stop stopColor="var(--ok)" />
            <stop offset=".52" stopColor="var(--accent-2)" />
            <stop offset="1" stopColor="var(--err)" />
          </linearGradient>
        </defs>
        <path style={{ stroke: `url(#${gradientId})` }} d="M77 76 C116 91 126 120 150 146" />
        <path style={{ stroke: `url(#${gradientId})` }} d="M270 111 C228 118 214 134 185 151" />
        <path style={{ stroke: `url(#${gradientId})` }} d="M104 278 C123 230 138 209 155 184" />
      </svg>

      <CaptureCard className={styles.captureA} icon="messages" title="Meet at 17:30" meta="Messages · now" />
      <CaptureCard className={styles.captureB} icon="fileImage" title="Kyiv sunset" meta="Photos · 8m" />
      <CaptureCard className={styles.captureC} icon="code" title="queryClient…" meta="VS Code · 21m" />

      <span className={styles.captureCore}>
        <Icon name="library" weight="duotone" size="lg" />
        <span className={styles.secureBadge}><Icon name="lock" weight="fill" size="md" /></span>
      </span>
    </div>
  );
}

function CaptureCard({
  className,
  icon: iconName,
  title,
  meta,
}: {
  className: string;
  icon: IconName;
  title: string;
  meta: string;
}) {
  return (
    <span className={`${styles.captureCard} ${className}`}>
      <span className={styles.captureCardIcon}><Icon name={iconName} weight="duotone" size="md" /></span>
      <span className={styles.captureCardCopy}>
        <strong>{title}</strong>
        <small>{meta}</small>
      </span>
    </span>
  );
}

export function NetworkArtwork({ pairingLimit }: { pairingLimit: number }) {
  const gradientId = `onboarding-network-${useId().replace(/:/g, "")}`;

  return (
    <div className={styles.networkScene} aria-hidden="true">
      <svg className={styles.networkPaths} viewBox="0 0 330 350">
        <defs>
          <linearGradient id={gradientId} x1="0" y1="0" x2="1" y2="1">
            <stop stopColor="var(--ok)" />
            <stop offset=".45" stopColor="var(--accent-2)" />
            <stop offset="1" stopColor="var(--err)" />
          </linearGradient>
        </defs>
        <path style={{ stroke: `url(#${gradientId})` }} d="M50 62 C93 77 112 123 154 158" />
        <path style={{ stroke: `url(#${gradientId})` }} d="M286 40 C248 78 221 112 177 151" />
        <path style={{ stroke: `url(#${gradientId})` }} d="M288 282 C242 245 213 209 177 172" />
        <path style={{ stroke: `url(#${gradientId})` }} d="M53 294 C83 238 112 201 150 173" />
      </svg>

      <span className={styles.networkHub}><Icon name="cloudOff" weight="duotone" size="lg" /></span>
      <DeviceNode className={styles.deviceMac} icon="laptop" />
      <DeviceNode className={styles.devicePhone} icon="mobile" />
      <DeviceNode className={styles.deviceTablet} icon="tablet" />
      <DeviceNode className={styles.deviceDesktop} icon="monitor" />
      <span className={styles.networkNote}>
        <strong>Up to {pairingLimit} other devices</strong> · encrypted end to end
      </span>
    </div>
  );
}

function DeviceNode({
  className,
  icon: iconName,
}: {
  className: string;
  icon: IconName;
}) {
  return (
    <span className={`${styles.deviceNode} ${className}`}>
      <Icon name={iconName} weight="duotone" size="md" />
    </span>
  );
}
