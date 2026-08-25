import type { ComponentProps, ReactNode, RefObject } from "react";


import { ActionButton } from "@/components/shared";
import { cn } from "@/lib/cn";
import styles from "./ExpandableSearch.module.css";

interface ExpandableSearchProps extends ComponentProps<"div"> {
  expanded: boolean;
  label: string;
  onExpandedChange: (open: boolean) => void;
  triggerRef?: RefObject<HTMLButtonElement | null>;
  children: ReactNode;
}

function ExpandableSearch({
  expanded,
  label,
  onExpandedChange,
  triggerRef,
  className,
  children,
  ...props
}: ExpandableSearchProps) {
  return (
    <div
      role="search"
      data-search-open={expanded ? "true" : "false"}
      className={cn(
        styles.root,
        expanded ? styles.expanded : styles.collapsed,
        className,
      )}
      {...props}
    >
      {expanded ? (
        children
      ) : (
        <ActionButton
          ref={triggerRef}
          type="button"
          size="compactIcon"
          icon="search"
          aria-label={label}
          title={label}
          onClick={() => onExpandedChange(true)}
        />
      )}
    </div>
  );
}

export { ExpandableSearch };
