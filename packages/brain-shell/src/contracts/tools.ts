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

/** Thinking-budget configuration passed through to the Brain runtime. */
export interface ThinkingConfig {
  maxTokens?: number;
}
