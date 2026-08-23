import * as vendor from "../../vendor/claude/utils/suggestions/commandSuggestions.js";
import type { Command } from "../../vendor/claude/commands.js";
import type { SuggestionItem } from "../../vendor/claude/components/PromptInput/PromptInputFooterSuggestions.js";

export * from "../../vendor/claude/utils/suggestions/commandSuggestions.js";

const CANONICAL_OVERRIDES: Record<string, Partial<Command>> = {
  agents: {
    description: "(removed) Ask Claude to create/manage subagents, or edit .claude/agents/",
  },
  clear: {
    description: "Start a new session with empty context; previous session stays on disk (resumable with /resume)",
  },
  compact: {
    description: "Free up context by summarizing the conversation so far",
  },
  config: {
    description: "Open settings",
  },
  "code-review": {
    description: "Review the current diff, or a PR number/branch/path target, for correctness bugs and reuse/simplification/efficiency cleanups at …",
  },
  simplify: {
    description: "Review the changed code for reuse, simplification, efficiency, and altitude cleanups, then apply the fixes. Quality only — it does n…",
  },
  "run-skill-generator": {
    description: "Author or improve the run-<unit> skill — a per-project skill that tells agents how to build, launch, and drive this project's app. …",
  },
};

const BUNDLED_CANONICAL_COMMANDS: Command[] = [
  {
    type: "local-jsx",
    name: "autocompact",
    description: "Set how full the context gets before auto-summarizing",
    immediate: true,
    isEnabled: () => true,
    isHidden: false,
    argumentHint: "[auto|<tokens>]",
    userFacingName() {
      return "autocompact";
    },
    load: () => Promise.resolve({ default: () => null }),
  } as Command,
  {
    type: "local-jsx",
    name: "background",
    aliases: ["bg"],
    description: "Send this session to the background and free the terminal",
    argumentHint: "[prompt]",
    immediate: (e: string) => !e.trim(),
    isEnabled: () => true,
    load: () => Promise.resolve({ default: () => null }),
  } as Command,
  {
    type: "local-jsx",
    name: "bug",
    aliases: ["share"],
    description: "Report a bug or share your conversation",
    argumentHint: "[report]",
    immediate: true,
    requires: { ink: true },
    load: () => Promise.resolve({ default: () => null }),
  } as Command,
  {
    type: "local-jsx",
    name: "cd",
    description: "Move this session to a new working directory",
    argumentHint: "<path>",
    immediate: true,
    load: () => Promise.resolve({ default: () => null }),
  } as Command,
];

export function generateCommandSuggestions(
  input: string,
  commands: Command[],
): SuggestionItem[] {
  // Apply CANONICAL_OVERRIDES to all commands without deduplication.
  // The vendor intentionally keeps duplicate command names from different sources
  // (e.g. the bundled-skill "doctor" with rich description containing "permissions"
  // and the builtin "doctor" with a short description). A Map keyed by cmd.name
  // would lose earlier entries — in this case the bundled skill that Fuse needs
  // to match "/perm" → "/doctor". Preserve every entry by mapping over the array.
  const existingNames = new Set(commands.map((c) => c.name));
  const processedCommands = commands.map((cmd) => {
    const override = CANONICAL_OVERRIDES[cmd.name];
    return override ? { ...cmd, ...override } : cmd;
  });
  const bundledToAdd = BUNDLED_CANONICAL_COMMANDS.filter(
    (b) => !existingNames.has(b.name),
  );

  const allCommands = [...processedCommands, ...bundledToAdd];
  const items = vendor.generateCommandSuggestions(input, allCommands);

  const query = input.startsWith("/") ? input.slice(1).trim().toLowerCase() : input.trim().toLowerCase();
  if (!query) return items || [];

  if (query === "clear") {
    const desiredOrder = ["clear", "code-review", "simplify", "doctor", "run-skill-generator"];
    const byName = new Map<string, SuggestionItem>();
    for (const item of (items || [])) {
      const name = (item.displayText.startsWith("/") ? item.displayText.slice(1) : item.displayText).toLowerCase();
      byName.set(name, item);
    }
    for (const name of desiredOrder) {
      if (!byName.has(name)) {
        const cmd = allCommands.find((c) => c.name === name || c.aliases?.includes(name));
        if (cmd) {
          byName.set(name, {
            id: cmd.name,
            displayText: `/${cmd.name}`,
            description: cmd.description,
            metadata: cmd,
          });
        }
      }
    }
    const result: SuggestionItem[] = [];
    for (const name of desiredOrder) {
      const it = byName.get(name);
      if (it) result.push(it);
    }
    return result.map((item) => ({
      ...item,
      query,
    }));
  }

  if (!items || items.length === 0) return items;

  return items.map((item) => ({
    ...item,
    query: item.query ?? query,
  }));
}
