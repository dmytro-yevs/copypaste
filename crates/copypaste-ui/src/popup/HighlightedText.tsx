// ── HighlightedText ───────────────────────────────────────────────────────────
// Fuzzy-matched chars wrapped in accent colour+bg. DROP bold weight (causes width-shift).
import React from "react";

export function HighlightedText({ text, positions }: { text: string; positions: number[] }): React.ReactElement {
  if (positions.length === 0) {
    return <>{text}</>;
  }
  const posSet = new Set(positions);
  const nodes: React.ReactNode[] = [];
  let i = 0;
  while (i < text.length) {
    if (posSet.has(i)) {
      let j = i;
      while (j < text.length && posSet.has(j)) j++;
      nodes.push(
        <span
          key={i}
          style={{
            background: "color-mix(in srgb, var(--accent) 30%, transparent)",
            color: "var(--text)",
            borderRadius: "var(--r-xs)",
          }}
        >
          {text.slice(i, j)}
        </span>
      );
      i = j;
    } else {
      let j = i;
      while (j < text.length && !posSet.has(j)) j++;
      nodes.push(text.slice(i, j));
      i = j;
    }
  }
  return <>{nodes}</>;
}
