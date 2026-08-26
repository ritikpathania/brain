/**
 * Brain-owned slash-command catalog — the single source of truth for the
 * palette, /help output, and command execution. Commands are pure data +
 * a sync `run` returning a declarative result; the shell interprets results
 * into state (Inc 21). Built-ins self-register from ./builtin.js.
 */

export type CommandAction = 'clear' | 'quit' | 'resume' | 'theme';
export type CommandOverlay = 'doctor' | 'memory';

export type CommandResult =
  | { type: 'text'; value: string }
  | { type: 'none' }
  | { type: 'action'; action: CommandAction }
  | { type: 'overlay'; overlay: CommandOverlay };

export interface CommandContext {
  args: string[];
  sessionId?: string;
}

export interface Command {
  /** Name without the leading '/'. Lowercase `[a-z0-9_-]+`. */
  name: string;
  /** One-line description shown in the palette and /help output. */
  description: string;
  aliases?: string[];
  argumentHint?: string;
  hidden?: boolean;
  run(ctx: CommandContext): CommandResult;
}

const registry = new Map<string, Command>();

export function registerCommand(cmd: Command): void {
  registry.set(cmd.name, cmd);
  for (const alias of cmd.aliases ?? []) registry.set(alias, cmd);
}

export function getCommands(): Command[] {
  return [...new Set(registry.values())].sort((a, b) => a.name.localeCompare(b.name));
}

export function getCommand(name: string): Command | undefined {
  return registry.get(name);
}
