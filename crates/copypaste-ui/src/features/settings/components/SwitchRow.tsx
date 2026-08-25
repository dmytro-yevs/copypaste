import { useId, type ReactNode } from "react";

import { SettingsRow } from "@/components/shared";
import { Switch } from "@/components/ui";

interface SwitchRowProps {
  title: string;
  description: string;
  id: string;
  checked: boolean;
  disabled?: boolean;
  busy?: boolean;
  badge?: ReactNode;
  note?: ReactNode;
  onChange: (checked: boolean) => void;
}

export function SwitchRow({
  title,
  description,
  id,
  checked,
  disabled,
  busy,
  badge,
  note,
  onChange,
}: SwitchRowProps) {
  const descriptionId = useId();
  const noteId = useId();
  return (
    <SettingsRow
      title={title}
      description={description}
      descriptionId={descriptionId}
      badge={badge}
      note={note === undefined ? undefined : <span id={noteId}>{note}</span>}
    >
      <Switch
        id={id}
        aria-label={title}
        aria-describedby={`${descriptionId}${note === undefined ? "" : ` ${noteId}`}`}
        aria-busy={busy || undefined}
        checked={checked}
        disabled={disabled}
        onCheckedChange={onChange}
      />
    </SettingsRow>
  );
}
