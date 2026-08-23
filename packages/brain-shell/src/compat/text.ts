/** Text utilities shared by composer and transcript rendering. */

export function truncatePath(path: string, max: number): string {
  if (path.length <= max) return path;
  const parts = path.split('/');
  let out = parts.at(-1)!;
  for (let i = parts.length - 2; i >= 0; i--) {
    const next = `${parts[i]}/${out}`;
    if (next.length > max - 1) break; // reserve 1 col for ellipsis
    out = next;
  }
  return `…/${out}`.slice(-max);
}

/**
 * Placeholder passthrough — real width-aware wrapping lands with the
 * markdown renderer (Inc 1), which is the only consumer that needs it.
 */
export const wrapAnsi = (text: string, _cols: number): string => text;
