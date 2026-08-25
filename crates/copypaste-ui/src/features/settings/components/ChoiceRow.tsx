import type { ReactNode } from "react";
import type { IconName } from "@/components/ui/icon";
import { useId } from "react";

import { useTranslation } from "@/i18n";
import { FieldFeedback, SettingsRow } from "@/components/shared";
import type { Choice } from "@/features/settings/model/serviceChoices";
import { valuesWith } from "@/features/settings/model/serviceChoices";
import { Select } from "@/components/ui";

const compactNumber = new Intl.NumberFormat(undefined, {
    notation: "compact",
    maximumFractionDigits: 0,
});

interface ChoiceRowProps {
    title: string;
    description: string;
    icon: IconName;
    badge?: ReactNode;
    note?: ReactNode;
    choices: readonly Choice[];
    value: number;
    disabled?: boolean;
    busy?: boolean;
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
    busy,
    validation,
    onChange,
}: ChoiceRowProps) {
    const { t } = useTranslation();
    const options = valuesWith(choices, value);
    const descriptionId = useId();
    const errorId = useId();
    const noteId = useId();
    const invalid =
        validation !== undefined &&
        (value < validation.min ||
            (validation.max !== undefined && value > validation.max));

    return (
        <SettingsRow
            title={title}
            description={description}
            descriptionId={descriptionId}
            badge={badge}
            note={
                <>
                    {invalid && (
                        <FieldFeedback id={errorId} state="error">
                            {validation.message}
                        </FieldFeedback>
                    )}
                    {note !== undefined && <span id={noteId}>{note}</span>}
                </>
            }
        >
            <Select
                size="sm"
                aria-label={title}
                aria-describedby={`${descriptionId}${note === undefined ? "" : ` ${noteId}`}`}
                aria-invalid={invalid || undefined}
                aria-errormessage={invalid ? errorId : undefined}
                aria-busy={busy || undefined}
                measure="regular"
                disabled={disabled}
                value={String(value)}
                leadingIcon={icon}
                items={options.map((choice) => ({
                    value: String(choice.value),
                    label:
                        choice.unit === "items"
                            ? compactNumber.format(choice.count)
                            : t(`settings.service.units.${choice.unit}`, {
                                  count: choice.count,
                              }),
                }))}
                onValueChange={(next) => onChange(Number(next))}
            />
        </SettingsRow>
    );
}
