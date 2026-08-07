import type { ReactNode } from "react";

import { Row } from "@/components/settings/Row";
import { Switch } from "@/components/ui/switch";

interface SwitchRowProps {
  title: string;
  description: string;
  id: string;
  checked: boolean;
  disabled?: boolean;
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
  badge,
  note,
  onChange,
}: SwitchRowProps) {
  return (
    <Row title={title} description={description} badge={badge} note={note}>
      <Switch
        id={id}
        aria-label={title}
        checked={checked}
        disabled={disabled}
        onCheckedChange={onChange}
      />
    </Row>
  );
}
