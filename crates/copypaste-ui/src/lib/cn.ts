import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

/** Later Tailwind utilities win over earlier ones. */
export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}
