import { useEffect, useRef, type ReactNode } from "react";

import styles from "./SettingsDisclosure.module.css";

export function SettingsDisclosure({
  title,
  description,
  revealKey,
  children,
}: {
  title: string;
  description?: string;
  revealKey?: string;
  children: ReactNode;
}) {
  const details = useRef<HTMLDetailsElement>(null);

  useEffect(() => {
    if (revealKey !== undefined && details.current) details.current.open = true;
  }, [revealKey]);

  return (
    <details ref={details} className={styles.root} data-settings-search-target={`section:${title}`}>
      <summary className={styles.trigger}>
        <span className={styles.copy}>
          <strong>{title}</strong>
          {description ? <span>{description}</span> : null}
        </span>
      </summary>
      <div className={styles.content}>{children}</div>
    </details>
  );
}
