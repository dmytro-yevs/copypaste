import { useState } from "react";

import { Button } from "@/components/ui";
import { absoluteTime, shortAge } from "@/lib/format";
import styles from "./TimestampToggle.module.css";

interface TimestampToggleProps {
  createdAt: number;
  tabIndex?: number;
}

export function TimestampToggle({
  createdAt,
  tabIndex = -1,
}: TimestampToggleProps) {
  const [exact, setExact] = useState(false);
  const concise = shortAge(createdAt);
  const full = absoluteTime(createdAt);
  const label = exact
    ? `Copied ${full}. Activate to show relative time.`
    : `Copied ${concise}. Activate to show exact time.`;

  return (
    <Button
      variant="ghost"
      size="sm"
      tabIndex={tabIndex}
      aria-pressed={exact}
      aria-label={label}
      title={label}
      onClick={(event) => {
        event.stopPropagation();
        setExact((shown) => !shown);
      }}
      onKeyDown={(event) => event.stopPropagation()}
      className={styles.root}
    >
      <time dateTime={new Date(createdAt).toISOString()}>{exact ? full : concise}</time>
    </Button>
  );
}
