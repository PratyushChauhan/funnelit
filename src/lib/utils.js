import { clsx } from "clsx";
import { twMerge } from "tailwind-merge";

/**
 * Inputs: class values. Outputs: merged Tailwind class string.
 */
export function cn(...inputs) {
	return twMerge(clsx(inputs));
}
