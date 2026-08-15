/** jsdom keeps `window.innerWidth` but never re-evaluates a media query from
 *  it; the width-aware `matchMedia` in `setup.ts` listens for `resize`. */
import { act } from "@testing-library/react";

/** jsdom's own default, and what every test that says nothing about width
 *  gets: an expanded window. */
export const DEFAULT_TEST_WIDTH = 1024;

export function setViewportWidth(width: number): void {
  (window as unknown as { innerWidth: number }).innerWidth = width;
  act(() => {
    window.dispatchEvent(new Event("resize"));
  });
}

/** For `afterEach`, which runs before React Testing Library's cleanup: telling
 *  a tree that is about to be unmounted about a resize only queues work for
 *  the next test to trip over. */
export function resetViewportWidth(): void {
  (window as unknown as { innerWidth: number }).innerWidth = DEFAULT_TEST_WIDTH;
}
