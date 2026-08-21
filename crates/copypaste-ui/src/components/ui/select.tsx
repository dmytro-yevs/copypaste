import * as SelectPrimitive from "@radix-ui/react-select";
import { Check, ChevronDown, type LucideIcon } from "lucide-react";

import { cn } from "@/lib/cn";

export interface SelectItem {
  readonly value: string;
  readonly label: string;
  readonly icon?: LucideIcon;
}

interface SelectProps {
  value: string;
  items: readonly SelectItem[];
  onValueChange: (value: string) => void;
  className?: string;
  contentClassName?: string;
  optionClassName?: string;
  labelClassName?: string;
  placeholder?: string;
  disabled?: boolean;
  id?: string;
  "aria-label"?: string;
  "aria-labelledby"?: string;
  "aria-describedby"?: string;
  "aria-invalid"?: boolean;
  "aria-errormessage"?: string;
}

export function Select({
  value,
  items,
  onValueChange,
  className,
  contentClassName,
  optionClassName,
  labelClassName,
  placeholder,
  disabled = false,
  ...aria
}: SelectProps) {
  const selected =
    items.find((item) => item.value === value) ??
    (placeholder ? { value: "", label: placeholder } : items[0]);
  const SelectedIcon = selected?.icon;

  return (
    <SelectPrimitive.Root value={value} onValueChange={onValueChange} disabled={disabled}>
      <SelectPrimitive.Trigger
        {...aria}
        data-slot="select-trigger"
        data-value={value}
        className={cn(
          "inline-flex h-9 min-h-[var(--tap-min)] max-w-full items-center justify-between gap-2 rounded-md border border-border-strong bg-panel px-3 text-sm font-normal text-foreground shadow-none outline-none transition-all focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50",
          className,
        )}
      >
        <span className="flex min-w-0 items-center gap-2">
          {SelectedIcon ? <SelectedIcon aria-hidden="true" className="size-4 shrink-0" /> : null}
          <span className={cn("truncate", labelClassName)}>{selected?.label}</span>
        </span>
        <SelectPrimitive.Icon asChild>
          <ChevronDown aria-hidden="true" className="size-4 shrink-0 text-muted-foreground" />
        </SelectPrimitive.Icon>
      </SelectPrimitive.Trigger>
      <SelectPrimitive.Portal>
        <SelectPrimitive.Content
          position="popper"
          sideOffset={8}
          collisionPadding={8}
          className={cn(
            "z-[var(--z-popover)] max-h-[min(320px,var(--radix-select-content-available-height))] min-w-[var(--radix-select-trigger-width)] max-w-[calc(100vw-16px)] overflow-hidden rounded-xl border border-border-strong bg-popover shadow-2 motion-safe:animate-in motion-safe:fade-in-0 motion-safe:zoom-in-95",
            contentClassName,
          )}
        >
          <SelectPrimitive.Viewport className="max-h-[min(320px,var(--radix-select-content-available-height))] p-1">
            {items.map((item) => {
              const Icon = item.icon;
              return (
                <SelectPrimitive.Item
                  key={item.value}
                  value={item.value}
                  data-value={item.value}
                  className={cn(
                    "relative flex cursor-default items-center gap-2 rounded-lg py-s-2 pr-8 pl-s-2 text-sm outline-none hover:bg-selected focus:bg-selected focus:text-foreground data-[state=checked]:bg-selected",
                    optionClassName,
                  )}
                >
                  {Icon ? <Icon aria-hidden="true" className="size-4 shrink-0" /> : null}
                  <SelectPrimitive.ItemText asChild>
                    <span className="min-w-0 flex-1 truncate">{item.label}</span>
                  </SelectPrimitive.ItemText>
                  <SelectPrimitive.ItemIndicator className="absolute right-s-2 inline-flex items-center">
                    <Check aria-hidden="true" className="size-4 shrink-0 text-brand-2" />
                  </SelectPrimitive.ItemIndicator>
                </SelectPrimitive.Item>
              );
            })}
          </SelectPrimitive.Viewport>
        </SelectPrimitive.Content>
      </SelectPrimitive.Portal>
    </SelectPrimitive.Root>
  );
}
