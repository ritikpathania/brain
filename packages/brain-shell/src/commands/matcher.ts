/**
 * Shell-local static command set + fuzzy matcher for the slash palette.
 * Pure data + functions: no I/O, no React. Distinct from the dynamic
 * registration contract in ./registry.ts — those are daemon-side command
 * modules; these three are handled entirely inside the shell.
 */

export interface BrainCommand {
  /** Name without the leading '/'. Lowercase `[a-z0-9_-]+`. */
  name: string;
  /** One-line description shown in the palette and /help output. */
  description: string;
  aliases?: readonly string[];
}

export interface CommandMatch {
  command: BrainCommand;
  score: number;
}

/** The Inc 2 command set. Later increments extend this list. */
export const COMMANDS: readonly BrainCommand[] = [
  { name: 'help', description: 'List available slash commands' },
  { name: 'clear', description: 'Clear the transcript' },
  { name: 'quit', description: 'Exit Brain shell', aliases: ['q'] },
];

/**
 * The palette is open iff the whole buffer is a bare slash token:
 * '/', '/c', '/clear' — never '/clear now' (args started) or 'x/y'.
 * Returns the query text without the leading '/', or null when closed.
 */
export function parseCommandQuery(value: string): string | null {
  const m = /^\/([a-z0-9_-]*)$/.exec(value);
  return m ? m[1]! : null;
}

/** Every char of needle appears in hay in order. */
function isSubsequence(needle: string, hay: string): boolean {
  let i = 0;
  for (const ch of hay) {
    if (ch === needle[i]) i++;
    if (i === needle.length) return true;
  }
  return needle.length === 0;
}

/**
 * Score tiers (higher wins): name exact 100 > alias exact 85 > name prefix
 * 80 > alias prefix 70 > name subsequence 60 > alias subsequence 50 >
 * description word prefix 30 > description word subsequence 20; '' matches
 * everything at 10. Ties break by name ascending — deterministic regardless
 * of list order.
 */
export function fuzzyMatchCommands(
  query: string,
  commands: readonly BrainCommand[] = COMMANDS,
): CommandMatch[] {
  const q = query.toLowerCase();
  const matches: CommandMatch[] = [];
  for (const command of commands) {
    let score = 10; // empty query lists all
    if (q.length > 0) {
      const name = command.name.toLowerCase();
      const aliases = (command.aliases ?? []).map((a) => a.toLowerCase());
      if (name === q) score = 100;
      else if (aliases.includes(q)) score = 85;
      else if (name.startsWith(q)) score = 80;
      else if (aliases.some((a) => a.startsWith(q))) score = 70;
      else if (isSubsequence(q, name)) score = 60;
      else if (aliases.some((a) => isSubsequence(q, a))) score = 50;
      else {
        const words = command.description.toLowerCase().split(/\s+/).filter(Boolean);
        if (words.some((w) => w.startsWith(q))) score = 30;
        else if (words.some((w) => isSubsequence(q, w))) score = 20;
        else score = 0;
      }
    }
    if (score > 0) matches.push({ command, score });
  }
  return matches.sort(
    (a, b) => b.score - a.score || a.command.name.localeCompare(b.command.name),
  );
}
