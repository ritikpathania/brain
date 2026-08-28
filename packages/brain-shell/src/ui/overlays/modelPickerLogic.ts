/**
 * Pure presentation-VM transforms and keyboard decision logic for the
 * interactive /model picker overlay.
 */
import type { BrainModelDescriptor } from '../../client/BrainBackendClient.js';

export interface ModelItemVM {
  id: string;
  name: string;
  provider: string;
  contextWindow: number;
  maxOutputTokens: number;
  supportsThinking: boolean;
  supportsTools: boolean;
  isDefault: boolean;
  isActive: boolean;
  contextLabel: string;
}

export function formatContextTokens(tokens: number): string {
  if (tokens >= 1000) {
    const k = Math.round(tokens / 1000);
    return `${k}k ctx`;
  }
  return `${tokens} ctx`;
}

export function toModelItemVM(
  models: BrainModelDescriptor[],
  activeModelId?: string,
): ModelItemVM[] {
  return models.map((m) => {
    const isActive = activeModelId ? m.id === activeModelId : m.isDefault;
    return {
      id: m.id,
      name: m.name,
      provider: m.provider,
      contextWindow: m.contextWindow,
      maxOutputTokens: m.maxOutputTokens,
      supportsThinking: m.supportsThinking,
      supportsTools: m.supportsTools,
      isDefault: m.isDefault,
      isActive,
      contextLabel: formatContextTokens(m.contextWindow),
    };
  });
}

export function clampModelSelection(index: number, count: number): number {
  if (count <= 0) return 0;
  return Math.max(0, Math.min(count - 1, index));
}

export function modelListDecision(
  action: string,
  selected: number,
  count: number,
): { nextSelected: number; committed?: boolean; cancelled?: boolean } {
  if (count <= 0) {
    return { nextSelected: 0, cancelled: action === 'overlay:cancel' || action === 'cancel' };
  }
  if (action === 'overlay:up' || action === 'up') {
    return { nextSelected: Math.max(0, selected - 1) };
  }
  if (action === 'overlay:down' || action === 'down') {
    return { nextSelected: Math.min(count - 1, selected + 1) };
  }
  if (action === 'overlay:commit' || action === 'commit') {
    return { nextSelected: selected, committed: true };
  }
  if (action === 'overlay:cancel' || action === 'cancel') {
    return { nextSelected: selected, cancelled: true };
  }
  return { nextSelected: selected };
}
