import type { ComponentProps, ReactNode } from "react";
import { Search } from "lucide-react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/cn";

interface ExpandableSearchProps extends ComponentProps<"div"> {
  expanded: boolean;
  label: string;
  onExpandedChange: (open: boolean) => void;
  children: ReactNode;
}

function ExpandableSearch({
  expanded,
  label,
  onExpandedChange,
  className,
  children,
  ...props
}: ExpandableSearchProps) {
  return (
    <div
      role="search"
      data-search-open={expanded ? "true" : "false"}
      className={cn(
        "flex min-w-0 items-center gap-s-2",
        expanded ? "w-full flex-1 basis-full" : "shrink-0",
        className,
      )}
      {...props}
    >
      {expanded ? (
        children
      ) : (
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          aria-label={label}
          title={label}
          onClick={() => onExpandedChange(true)}
        >
          <Search aria-hidden="true" />
        </Button>
      )}
    </div>
  );
}

export { ExpandableSearch };
