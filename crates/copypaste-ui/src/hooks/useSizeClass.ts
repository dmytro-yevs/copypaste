/**
 * DMY-154: the shell picked its chrome from the user agent, so a tablet, a
 * foldable, a landscape phone and a desktop-windowed activity all got phone
 * chrome while the width they actually had went unread. A platform decides
 * which controls exist (`lib/platform.ts`); only width decides how they are
 * arranged.
 */
import { useViewportMetrics, type SizeClass } from "@/hooks/useViewportMetrics";

export { EXPANDED_MIN_PX, EXPANDED_QUERY } from "@/lib/layoutBreakpoints";

export type { SizeClass } from "@/hooks/useViewportMetrics";

/** A media query, not a `resize` listener: crossing the boundary is the only
 *  event that changes the layout, and a window drag fires `resize` for every
 *  pixel of itself. */
export function useSizeClass(): SizeClass {
  return useViewportMetrics().sizeClass;
}
