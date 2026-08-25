import type { InputHTMLAttributes, RefObject } from "react";

import { IconButton } from "./IconButton";
import {
  ControlAdornment,
  ControlSurface,
  Icon,
  Input,
  ShortcutBadge,
  type ControlSurfaceVariants,
} from "@/components/ui";
import { cn } from "@/lib/cn";
import styles from "./SearchField.module.css";

interface SearchFieldProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "className" | "size" | "width"> {
  inputRef?: RefObject<HTMLInputElement | null>;
  mode?: "inline" | "overlay";
  size?: NonNullable<ControlSurfaceVariants["size"]>;
  expanded?: boolean;
  shortcut?: string;
  clearable?: boolean;
  clearLabel?: string;
  closeLabel?: string;
  onClear?: () => void;
  onRequestClose?: () => void;
}

export function SearchField({
  inputRef,
  mode = "inline",
  size = "library",
  expanded = mode === "inline",
  shortcut = "⌘K",
  clearable = true,
  clearLabel = "Clear search",
  closeLabel = "Close search",
  value,
  onChange,
  onClear,
  onRequestClose,
  disabled,
  ...props
}: SearchFieldProps) {
  const hasValue = typeof value === "string" && value.length > 0;
  const adornmentSize = size === "compact" ? "compact" : "regular";
  return (
    <ControlSurface
      size={size}
      width="fill"
      state={disabled ? "disabled" : props["aria-invalid"] ? "invalid" : "normal"}
      className={cn(styles.root, mode === "overlay" && styles.overlay)}
      data-mode={mode}
      data-size={size}
      data-expanded={expanded || undefined}
    >
      <ControlAdornment size={adornmentSize} tone="muted">
        <Icon name="search" size={adornmentSize === "compact" ? "sm" : "md"} />
      </ControlAdornment>
      <Input
        ref={inputRef}
        {...props}
        type="search"
        role="searchbox"
        surface="embedded"
        value={value}
        disabled={disabled}
        onChange={onChange}
        className={styles.input}
      />
      {hasValue && clearable ? (
        <IconButton
          size={size === "compact" ? "compact" : "regular"}
          variant="ghost"
          edge="control"
          label={clearLabel}
          onClick={() => {
            onClear?.();
            inputRef?.current?.focus();
          }}
          icon="close"
        />
      ) : shortcut && mode === "inline" ? (
        <ShortcutBadge size={adornmentSize}>{shortcut}</ShortcutBadge>
      ) : null}
      {mode === "overlay" && !hasValue ? (
        <IconButton
          size={size === "compact" ? "compact" : "regular"}
          variant="ghost"
          edge="control"
          label={closeLabel}
          onClick={onRequestClose}
          icon="close"
        />
      ) : null}
    </ControlSurface>
  );
}
