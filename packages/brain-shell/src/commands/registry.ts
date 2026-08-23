/**
 * Brain-owned slash-command registry. The palette (Inc 2) reads from here;
 * command modules register themselves at import time.
 */

export interface CommandResult {
  type: 'text' | 'none';
  value?: string;
}

export interface CommandContext {
  args: string[];
  sessionId: string;
}

export interface Command {
  name: string;
  description: string;
  aliases?: string[];
  argumentHint?: string;
  hidden?: boolean;
  supportsNonInteractive?: boolean;
  handler(ctx: CommandContext): Promise<CommandResult>;
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
