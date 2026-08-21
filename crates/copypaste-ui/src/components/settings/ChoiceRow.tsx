import { useId } from "react";
import type { LucideIcon } from "lucide-react";

import { useTranslation } from "@/i18n";
import { Row } from "@/components/settings/Row";
import type { Choice } from "@/components/settings/serviceChoices";
import { valuesWith } from "@/components/settings/serviceChoices";
import { Select } from "@/components/ui/select";

interface ChoiceRowProps {
  title: string;
  description: string;
  icon: LucideIcon;
  badge?: React.ReactNode;
  note?: React.ReactNode;
  choices: readonly Choice[];
  value: number;
  disabled?: boolean;
  validation?: {
    readonly min: number;
    readonly max?: number;
    readonly message: string;
  };
  onChange: (value: number) => void;
}

export function ChoiceRow({
  title,
  description,
  icon,
  badge,
  note,
  choices,
  value,
  disabled,
  validation,
  onChange,
}: ChoiceRowProps) {
  const { t } = useTranslation();
  const options = valuesWith(choices, value);
  const descriptionId = useId();
  const errorId = useId();
  const invalid =
    validation !== undefined &&
    (value < validation.min ||
      (validation.max !== undefined && value > validation.max));

  return (
    <Row
      title={title}
      description={description}
      descriptionId={descriptionId}
      badge={badge}
      note={
        <>
          {invalid && (
            <span id={errorId} role="alert" className="text-xs text-destructive">
              {validation.message}
            </span>
          )}
          {note}
        </>
      }
    >
      <Select
        aria-label={title}
        aria-describedby={descriptionId}
        aria-invalid={invalid || undefined}
        aria-errormessage={invalid ? errorId : undefined}
        className="min-w-[9rem]"
        disabled={disabled}
        value={String(value)}
        items={options.map((choice) => ({
          value: String(choice.value),
          label: t(`settings.service.units.${choice.unit}`, { count: choice.count }),
          icon,
        }))}
        onValueChange={(next) => onChange(Number(next))}
      />
    </Row>
  );
}
