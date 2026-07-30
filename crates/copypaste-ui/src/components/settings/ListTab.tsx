/**
 * Settings › List — how the history list is drawn.
 *
 * The preview-lines slider is the one setting that changes layout: every row's
 * reserved height is a function of it (INV-5), so lowering it shrinks total
 * content height, which is exactly the case INV-6's clamp exists for (AT-4).
 * The value is formatted with its unit — a bare number told the user nothing
 * (CopyPaste-8ebg.63).
 */
import { Slider } from "@/components/ui/slider";
import { Switch } from "@/components/ui/switch";
import { Label } from "@/components/ui/label";
import { MAX_PREVIEW_LINES, MIN_PREVIEW_LINES } from "@/lib/layout";
import { usePrefs } from "@/store/prefs";
import { Row } from "@/components/settings/Row";

export function ListTab() {
  const previewLines = usePrefs((s) => s.previewLines);
  const warnBeforeReveal = usePrefs((s) => s.warnBeforeReveal);
  const set = usePrefs((s) => s.set);

  return (
    <div className="flex flex-col">
      <Row
        title="Preview lines"
        description="How many lines of a clip each row shows. Every row reserves this much space whether it fills it or not, so the list never overlaps itself."
      >
        <div className="flex w-[220px] items-center gap-s-3">
          <Slider
            aria-label="Preview lines"
            value={[previewLines]}
            min={MIN_PREVIEW_LINES}
            max={MAX_PREVIEW_LINES}
            step={1}
            onValueChange={([value]) =>
              value !== undefined && set("previewLines", value)
            }
          />
          <span className="w-14 shrink-0 text-right text-sm tabular-nums text-muted-foreground">
            {previewLines} line{previewLines === 1 ? "" : "s"}
          </span>
        </div>
      </Row>

      <Row
        title="Warn before revealing"
        description="Ask for confirmation before showing an item that looks like a password, key or token. Revealed items hide themselves again after 10 seconds, and whenever this window loses focus."
      >
        <div className="flex items-center gap-s-2">
          <Switch
            id="warn-before-reveal"
            checked={warnBeforeReveal}
            onCheckedChange={(value) => set("warnBeforeReveal", value)}
          />
          <Label htmlFor="warn-before-reveal">
            {warnBeforeReveal ? "On" : "Off"}
          </Label>
        </div>
      </Row>
    </div>
  );
}
