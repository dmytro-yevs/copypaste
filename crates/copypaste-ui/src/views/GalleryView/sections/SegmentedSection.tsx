import { useState } from "react";

// Segmented control primitive (`.seg`, task 6.6) — a standalone documented
// example, distinct from the gallery's own theme switcher which also happens
// to use `.seg` for its Theme group.
export function SegmentedSection() {
  const [value, setValue] = useState<"recency" | "device">("recency");
  return (
    <section id="gallery-segmented">
      <h2>Segmented control</h2>
      <div className="seg" role="group" aria-label="Sort order">
        <button
          type="button"
          className={value === "recency" ? "on" : undefined}
          aria-pressed={value === "recency"}
          onClick={() => setValue("recency")}
        >
          By time
        </button>
        <button
          type="button"
          className={value === "device" ? "on" : undefined}
          aria-pressed={value === "device"}
          onClick={() => setValue("device")}
        >
          By device
        </button>
      </div>
    </section>
  );
}
