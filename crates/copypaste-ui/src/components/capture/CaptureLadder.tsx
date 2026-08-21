/** Every step states where it stands in words — "Done", "Next", "Not done yet"
 *  — beside the glyph: a checklist whose only difference is a tick colour is
 *  not readable to everyone (A11Y-10). */
import { Check, Circle, Dot } from "lucide-react";

import { Stepper, type StepperItem } from "@/components/ui/stepper";
import { useTranslation } from "@/i18n";
import type { LadderRung, LadderStep } from "@/lib/capture";

const LABEL = {
  install: "capture.setup.ladder.install",
  start: "capture.setup.ladder.start",
  permission: "capture.setup.ladder.permission",
  armed: "capture.setup.ladder.armed",
} as const satisfies Record<LadderStep, string>;

export function CaptureLadder({ rungs }: { rungs: readonly LadderRung[] }) {
  const { t } = useTranslation();
  const items: StepperItem[] = rungs.map((rung) => ({
    id: rung.id,
    icon: rung.done ? Check : rung.current ? Dot : Circle,
    label: t(LABEL[rung.id]),
    stateLabel: t(
      rung.done
        ? "capture.setup.ladder.done"
        : rung.current
          ? "capture.setup.ladder.next"
          : "capture.setup.ladder.todo",
    ),
    done: rung.done,
    current: rung.current,
  }));

  return (
    <section
      data-settings-search-target={`section:${t("capture.setup.ladder.title")}`}
      className="flex flex-col gap-s-2"
    >
      <h2 className="text-sm font-medium">{t("capture.setup.ladder.title")}</h2>

      <Stepper label={t("capture.setup.ladder.label")} items={items} />
    </section>
  );
}
