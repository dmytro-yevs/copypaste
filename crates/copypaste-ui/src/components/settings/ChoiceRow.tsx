/** A native `<select>` rather than a Radix listbox: the platform one is
 *  keyboard- and screen-reader-correct on both targets with no roving tabindex
 *  to get wrong, and on Android it opens the system picker. */
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
  onChange,
}: ChoiceRowProps) {
  const { t } = useTranslation();
  const options = valuesWith(choices, value);

  return (
    <Row title={title} description={description} badge={badge} note={note}>
      <select
        aria-label={title}
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
