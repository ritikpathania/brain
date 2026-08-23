/**
 * Brain-owned tool-descriptor vocabulary (UI presentation + permission mapping input).
 */

export interface ToolPermissionContext {
  mode: 'default' | 'acceptEdits' | 'plan' | 'bypassPermissions';
  alwaysAllowRules: string[];
  alwaysDenyRules: string[];
}

export interface ToolUseContext {
  sessionId: string;
  workingDirectory: string;
  abortController?: AbortController;
}

export interface Tool<TInput = Record<string, unknown>> {
  name: string;
  description: string;
  inputSchema: TInput;
  isReadOnly(input: TInput): boolean;
  isConcurrencySafe(input: TInput): boolean;
}

/**
 * Thinking-budget configuration passed through to the Brain runtime.
 * Accepts both spellings callers use: the runtime-native `mode` switch and
 * the explicit `type` + `budgetTokens` pair; normalizeThinkingConfig folds
 * them into BrainThinkingConfig.
 */
export interface ThinkingConfig {
  mode?: 'auto' | 'off';
  type?: 'enabled' | 'disabled';
  budgetTokens?: number;
  maxTokens?: number;
}
