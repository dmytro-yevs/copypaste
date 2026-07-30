import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

/** Conditional class names with later Tailwind utilities winning over earlier
 *  ones. Both halves are the maintained libraries; neither is reimplemented. */
export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}
