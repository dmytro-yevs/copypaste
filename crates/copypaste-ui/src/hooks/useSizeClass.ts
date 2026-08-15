/**
 * DMY-154: the shell picked its chrome from the user agent, so a tablet, a
 * foldable, a landscape phone and a desktop-windowed activity all got phone
 * chrome while the width they actually had went unread. A platform decides
 * which controls exist (`lib/platform.ts`); only width decides how they are
 * arranged.
 */
import { useMediaQuery } from "usehooks-ts";

export type SizeClass = "compact" | "expanded";

/** Tailwind's own `sm`. `design/tokens/layout.json` says why there is exactly
 *  one boundary: a second scale beside Tailwind's is how two scales start. */
export const EXPANDED_MIN_PX = 640;

export const EXPANDED_QUERY = `(min-width: ${EXPANDED_MIN_PX}px)`;

/** A media query, not a `resize` listener: crossing the boundary is the only
 *  event that changes the layout, and a window drag fires `resize` for every
 *  pixel of itself. */
export function useSizeClass(): SizeClass {
  return useMediaQuery(EXPANDED_QUERY) ? "expanded" : "compact";
}
