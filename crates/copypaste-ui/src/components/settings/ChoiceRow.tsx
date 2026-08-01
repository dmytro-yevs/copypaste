/** A native `<select>` rather than a Radix listbox: the platform one is
 *  keyboard- and screen-reader-correct on both targets with no roving tabindex
 *  to get wrong, and on Android it opens the system picker. */
import { useId } from "react";

import { useTranslation } from "@/i18n";
import { Row } from "@/components/settings/Row";
import type { Choice } from "@/components/settings/serviceChoices";
import { valuesWith } from "@/components/settings/serviceChoices";

interface ChoiceRowProps {
  title: string;
  description: string;
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
      <select
        aria-label={title}
        aria-describedby={descriptionId}
        aria-invalid={invalid || undefined}
        aria-errormessage={invalid ? errorId : undefined}
        className="h-9 min-w-[9rem] rounded-md border border-input bg-transparent px-2 text-sm outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50"
        disabled={disabled}
        value={value}
        onChange={(event) => onChange(Number(event.target.value))}
      >
        {options.map((choice) => (
          <option key={choice.value} value={choice.value}>
            {t(`settings.service.units.${choice.unit}`, { count: choice.count })}
          </option>
        ))}
      </select>
    </Row>
  );
}
