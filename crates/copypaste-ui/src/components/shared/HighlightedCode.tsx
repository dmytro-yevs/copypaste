import { toJsxRuntime } from "hast-util-to-jsx-runtime";
import { Fragment, jsx, jsxs } from "react/jsx-runtime";
import { useMemo, type ReactNode } from "react";

import { detectCode } from "@/lib/codeHighlight";
import { cn } from "@/lib/cn";
import type { Kind } from "@/lib/format";
import styles from "./HighlightedCode.module.css";

export function HighlightedCode({ content, kind, mode, lineClamp, ariaLabel }: {
  content: string;
  kind: Extract<Kind, "code" | "json">;
  mode: "card" | "inspector" | "expanded";
  lineClamp?: number;
  ariaLabel?: string;
}) {
  const result = useMemo(() => detectCode(content, kind, mode), [content, kind, mode]);
  const highlighted = useMemo<ReactNode>(
    () => result.tree === null
      ? result.highlightedText
      : (toJsxRuntime(result.tree, { Fragment, jsx, jsxs }) as ReactNode),
    [result.highlightedText, result.tree],
  );
  return (
    <div className={cn(styles.root, styles[mode])} data-language={result.language ?? "unknown"}>
      <span className={styles.language}>{result.label}</span>
      <pre aria-label={ariaLabel}>
        <code style={lineClamp === undefined ? undefined : { WebkitLineClamp: lineClamp }}>
          {highlighted}
          {mode === "expanded" ? result.remainder : null}
        </code>
      </pre>
    </div>
  );
}
