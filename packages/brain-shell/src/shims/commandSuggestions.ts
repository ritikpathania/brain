import * as vendor from '../../vendor/claude/utils/suggestions/commandSuggestions.js';
import type { Command } from '../../vendor/claude/commands.js';
import type { SuggestionItem } from './PromptInputFooterSuggestions.js';

export * from '../../vendor/claude/utils/suggestions/commandSuggestions.js';

/**
 * Enhanced generateCommandSuggestions that attaches `query` to each suggestion item,
 * matching the canonical Claude runtime contract for search query highlighting.
 */
export function generateCommandSuggestions(
  input: string,
  commands: Command[],
): SuggestionItem[] {
  const items = vendor.generateCommandSuggestions(input, commands);
  if (!items || items.length === 0) return items;

  const query = input.startsWith('/') ? input.slice(1).trim().toLowerCase() : input.trim().toLowerCase();
  if (!query) return items;

  return items.map((item) => ({
    ...item,
    query: item.query ?? query,
  }));
}
