/**
 * Model Selection & Gateway Routing (Layer 2 Brain Adapter)
 *
 * Provides model resolution, catalog enumeration, and mapping between
 * Claude canonical model IDs and Brain reasoning backends.
 */

export interface BrainModelDescriptor {
  id: string;
  name: string;
  provider: 'local-ollama' | 'local-vllm' | 'anthropic' | 'gemini' | 'openai' | 'mock';
  contextWindow: number;
  maxOutputTokens: number;
  supportsThinking: boolean;
  isDefault?: boolean;
}

export const DEFAULT_BRAIN_MODELS: BrainModelDescriptor[] = [
  {
    id: 'brain-default',
    name: 'Brain Default (Local Engine)',
    provider: 'local-ollama',
    contextWindow: 128000,
    maxOutputTokens: 8192,
    supportsThinking: true,
    isDefault: true,
  },
  {
    id: 'claude-3-7-sonnet-latest',
    name: 'Claude 3.7 Sonnet (Hybrid/Gateway)',
    provider: 'anthropic',
    contextWindow: 200000,
    maxOutputTokens: 64000,
    supportsThinking: true,
  },
  {
    id: 'claude-3-5-haiku-latest',
    name: 'Claude 3.5 Haiku (Fast)',
    provider: 'anthropic',
    contextWindow: 200000,
    maxOutputTokens: 8192,
    supportsThinking: false,
  },
  {
    id: 'deepseek-r1:latest',
    name: 'DeepSeek R1 (Local Reasoning)',
    provider: 'local-ollama',
    contextWindow: 64000,
    maxOutputTokens: 16384,
    supportsThinking: true,
  },
  {
    id: 'qwen2.5-coder:32b',
    name: 'Qwen 2.5 Coder 32B (Local Coding)',
    provider: 'local-ollama',
    contextWindow: 128000,
    maxOutputTokens: 8192,
    supportsThinking: false,
  },
];

export class ModelGateway {
  private models: Map<string, BrainModelDescriptor> = new Map();

  constructor(customModels?: BrainModelDescriptor[]) {
    const list = customModels || DEFAULT_BRAIN_MODELS;
    for (const m of list) {
      this.models.set(m.id, m);
    }
  }

  /**
   * List all currently registered models in the gateway.
   */
  getAvailableModels(): BrainModelDescriptor[] {
    return Array.from(this.models.values());
  }

  /**
   * Resolve an incoming model string into a normalized model ID and descriptor.
   */
  resolveModel(modelQuery?: string): BrainModelDescriptor {
    if (!modelQuery || modelQuery.trim() === '') {
      return this.getDefaultModel();
    }

    const query = modelQuery.trim().toLowerCase();

    // Direct match
    if (this.models.has(query)) {
      return this.models.get(query)!;
    }

    // Alias matches
    if (query === 'sonnet' || query === 'default') {
      return this.models.get('claude-3-7-sonnet-latest') || this.getDefaultModel();
    }
    if (query === 'haiku' || query === 'fast') {
      return this.models.get('claude-3-5-haiku-latest') || this.getDefaultModel();
    }
    if (query === 'deepseek' || query === 'r1') {
      return this.models.get('deepseek-r1:latest') || this.getDefaultModel();
    }
    if (query === 'qwen' || query === 'coder') {
      return this.models.get('qwen2.5-coder:32b') || this.getDefaultModel();
    }

    // Partial substring search
    for (const [id, descriptor] of this.models.entries()) {
      if (id.includes(query) || descriptor.name.toLowerCase().includes(query)) {
        return descriptor;
      }
    }

    // Fallback descriptor for arbitrary custom model strings
    return {
      id: modelQuery,
      name: `Custom (${modelQuery})`,
      provider: 'local-ollama',
      contextWindow: 64000,
      maxOutputTokens: 8192,
      supportsThinking: true,
    };
  }

  /**
   * Get the designated default model.
   */
  getDefaultModel(): BrainModelDescriptor {
    for (const m of this.models.values()) {
      if (m.isDefault) return m;
    }
    return DEFAULT_BRAIN_MODELS[0];
  }
}
