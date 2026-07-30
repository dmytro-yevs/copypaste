import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { RotateCcw, TriangleAlert } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Row } from "@/components/settings/Row";
import {
  DEFAULT_SHORTCUT,
  acceleratorGlyphs,
  captureAccelerator,
} from "@/lib/accelerator";
import { cn } from "@/lib/cn";
import { isUnavailable, toFriendly } from "@/lib/errors";
import { getShortcut, setShortcut } from "@/lib/ipc";

const SHORTCUT_KEY = ["shortcut"] as const;

function Keycaps({ accelerator }: { accelerator: string }) {
  return (
    <span aria-hidden="true" className="flex items-center gap-1">
      {acceleratorGlyphs(accelerator).map((glyph, index) => (
        <kbd
          key={`${glyph}-${index}`}
          className="inline-flex h-6 min-w-6 items-center justify-center rounded-sm border border-border bg-muted px-1.5 font-sans text-xs"
        >
          {glyph}
        </kbd>
      ))}
    </span>
  );
}

export function ShortcutTab() {
  const qc = useQueryClient();
  const [capturing, setCapturing] = useState(false);
  const [refusal, setRefusal] = useState<string | null>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);

  const current = useQuery({
    queryKey: SHORTCUT_KEY,
    queryFn: getShortcut,
    retry: false,
    staleTime: Infinity,
  });

  const save = useMutation({
    mutationFn: (accelerator: string) => setShortcut(accelerator),
    onSuccess: (_data, accelerator) => qc.setQueryData(SHORTCUT_KEY, accelerator),
  });

  const bound = current.data ?? DEFAULT_SHORTCUT;
  const unavailable =
    (current.error !== null && isUnavailable(current.error)) ||
    (save.error !== null && isUnavailable(save.error));

  useEffect(() => {
    if (!capturing) return;

    function onKeyDown(event: KeyboardEvent) {
      event.preventDefault();
      event.stopPropagation();
      const result = captureAccelerator(event);
      switch (result.kind) {
        case "incomplete":
          return;
        case "cancelled":
          setCapturing(false);
          setRefusal(null);
          buttonRef.current?.blur();
          return;
        case "refused":
          setRefusal(result.reason);
          return;
        case "accelerator":
          setCapturing(false);
          setRefusal(null);
          save.mutate(result.value);
          return;
      }
    }

    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [capturing, save]);

  // A11Y-13: the raw accelerator, not the glyphs — screen readers handle
  // "CmdOrCtrl+Shift+V" and mangle "⌘⇧V" (CopyPaste-8ebg.53).
  const accessibleName = capturing
    ? "Press a key combination"
    : `Current shortcut: ${bound}. Click and press a new key combination.`;

  return (
    <div className="flex flex-col">
      <Row
        title="Quick-paste shortcut"
        description="Opens CopyPaste from anywhere. Hold at least one modifier."
      >
        <div className="flex flex-col items-end gap-s-2">
          <button
            ref={buttonRef}
            type="button"
            aria-label={accessibleName}
            title={accessibleName}
            onClick={() => {
              setRefusal(null);
              setCapturing((was) => !was);
            }}
            onBlur={() => setCapturing(false)}
            className={cn(
              "flex h-10 min-w-[160px] items-center justify-center gap-2 rounded-md border px-s-3 text-sm outline-none focus-visible:ring-[3px] focus-visible:ring-ring",
              capturing
                ? "border-ring bg-selected"
                : "border-border-strong bg-background hover:bg-accent",
            )}
          >
            {capturing ? (
              <span className="text-muted-foreground">Press a key combination…</span>
            ) : (
              <Keycaps accelerator={bound} />
            )}
          </button>

          <Button
            variant="ghost"
            size="sm"
            disabled={bound === DEFAULT_SHORTCUT || save.isPending}
            onClick={() => save.mutate(DEFAULT_SHORTCUT)}
          >
            <RotateCcw aria-hidden="true" />
            Reset to default
          </Button>
        </div>
      </Row>

      {refusal && (
        <p
          role="alert"
          className="flex items-start gap-2 border-b border-divider py-s-3 text-sm text-warn-strong"
        >
          <TriangleAlert size={16} aria-hidden="true" className="mt-px shrink-0" />
          {refusal}
        </p>
      )}

      {save.error !== null && !isUnavailable(save.error) && (
        <p role="alert" className="border-b border-divider py-s-3 text-sm text-err-strong">
          {toFriendly(save.error)} The previous shortcut is still in effect.
        </p>
      )}

      {unavailable && (
        <p className="py-s-3 text-sm text-muted-foreground">
          This build can't change the shortcut yet — the background service owns
          it, and the app has no way to ask it to rebind. {DEFAULT_SHORTCUT} is
          in effect.
        </p>
      )}
    </div>
  );
}
