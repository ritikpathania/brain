/**
 * Brain CallModel Adapter (Phase 5.5 — Thinking & Reasoning Support)
 *
 * Conforms strictly to QueryDeps.callModel (CallModelFn) seam,
 * streaming native reasoning blocks without fabricating signatures.
 */

import type { QueryDeps } from '../contracts/query.js';
import type { Message } from '../contracts/messages.js';
import type { Tool } from '../contracts/tools.js';
import type { ThinkingConfig } from '../contracts/tools.js';
import { getSessionId } from '../contracts/session.js';
import { createAssistantMessage, createAssistantAPIErrorMessage } from '../contracts/messages.js';
import type {
  BrainBackendClient,
  BrainChatMessage,
  BrainToolDefinition,
  BrainContentBlock,
  BrainThinkingConfig,
} from '../client/BrainBackendClient.js';

export function normalizeToolsForBrain(tools?: Tool[]): BrainToolDefinition[] {
  if (!tools || tools.length === 0) return [];
  return tools.map((t) => ({
    name: t.name,
    description: t.description,
    inputSchema: (t as any).inputJSONSchema || (t as any).inputSchema,
  }));
}

export function normalizeThinkingConfig(config?: ThinkingConfig): BrainThinkingConfig {
  if (!config || (config as any).mode === 'off' || config.type === 'disabled') {
    return { mode: 'disabled' };
  }
  if (config.type === 'enabled') {
    return { mode: 'enabled', budgetTokens: config.budgetTokens };
  }
  return { mode: 'adaptive' };
}

export function normalizeMessagesForBrain(messages: Message[]): BrainChatMessage[] {
  const result: BrainChatMessage[] = [];

  for (const msg of messages) {
    if (msg.type === 'user') {
      const rawContent = (msg as any).message?.content;
      if (typeof rawContent === 'string') {
        result.push({ role: 'user', content: rawContent });
      } else if (Array.isArray(rawContent)) {
        const blocks: BrainContentBlock[] = [];
        for (const item of rawContent) {
          if (item && item.type === 'text' && typeof item.text === 'string') {
            blocks.push({ type: 'text', text: item.text });
          } else if (item && item.type === 'tool_result') {
            blocks.push({
              type: 'tool_result',
              tool_use_id: item.tool_use_id,
              content: typeof item.content === 'string' ? item.content : JSON.stringify(item.content),
              is_error: item.is_error,
            });
          }
        }
        if (blocks.length === 1 && blocks[0].type === 'text') {
          result.push({ role: 'user', content: blocks[0].text });
        } else {
          result.push({ role: 'user', content: blocks.length > 0 ? blocks : '' });
        }
      }
    } else if (msg.type === 'assistant') {
      const rawContent = (msg as any).message?.content;
      if (typeof rawContent === 'string') {
        result.push({ role: 'assistant', content: rawContent });
      } else if (Array.isArray(rawContent)) {
        const blocks: BrainContentBlock[] = [];
        for (const item of rawContent) {
          if (item && item.type === 'text' && typeof item.text === 'string') {
            blocks.push({ type: 'text', text: item.text });
          } else if (item && item.type === 'thinking' && typeof item.thinking === 'string') {
            blocks.push({
              type: 'thinking',
              thinking: item.thinking,
              signature: item.signature,
            });
          } else if (item && item.type === 'redacted_thinking' && typeof item.data === 'string') {
            blocks.push({
              type: 'redacted_thinking',
              data: item.data,
            });
          } else if (item && item.type === 'tool_use') {
            blocks.push({
              type: 'tool_use',
              id: item.id,
              name: item.name,
              input: item.input || {},
            });
          }
        }
        if (blocks.length === 1 && blocks[0].type === 'text') {
          result.push({ role: 'assistant', content: blocks[0].text });
        } else {
          result.push({ role: 'assistant', content: blocks.length > 0 ? blocks : '' });
        }
      }
    }
  }

  return result;
}

function extractSystemPromptText(systemPrompt: any): string {
  if (typeof systemPrompt === 'string') {
    return systemPrompt;
  }
  if (Array.isArray(systemPrompt)) {
    if (systemPrompt.every((item) => typeof item === 'string' && item.length === 1)) {
      return systemPrompt.join('');
    }
    const text = systemPrompt
      .map((item) => {
        if (typeof item === 'string') return item;
        if (item && typeof item.text === 'string') return item.text;
        return '';
      })
      .filter(Boolean)
      .join('\n\n');
    if (text) return text;
  }
  return 'You are Brain, the memory-first agent runtime.';
}

import type { BrainContextProvider } from './BrainContextProvider.js';
import type { CompiledContext } from '../client/BrainBackendClient.js';
import type { ToolFeedbackEmitter } from './ToolFeedbackEmitter.js';

export function createBrainCallModel(
  client: BrainBackendClient,
  sessionStore?: { recordAssistantTurn: (response: string, metrics?: any, turnId?: string, sessionId?: string) => Promise<any> },
  contextProvider?: BrainContextProvider,
  toolFeedbackEmitter?: ToolFeedbackEmitter
): QueryDeps['callModel'] {
  return async function* (params) {
    if (params.signal?.aborted) {
      return;
    }

    const formattedMessages = normalizeMessagesForBrain(params.messages);

    // If tool results are present in incoming messages, emit tool feedback
    if (toolFeedbackEmitter) {
      const sessionId = (params as any).sessionId || 'default_session';
      for (const msg of params.messages) {
        if (msg.type === 'user') {
          const rawContent = (msg as any).message?.content;
          if (Array.isArray(rawContent)) {
            for (const item of rawContent) {
              if (item && item.type === 'tool_result' && item.tool_use_id) {
                toolFeedbackEmitter.emitToolFeedback({
                  sessionId,
                  turnId: (params as any).turnId || `turn_${Date.now()}`,
                  toolUseId: item.tool_use_id,
                  toolName: (item as any).tool_name || 'Tool',
                  output: item.content,
                  isError: item.is_error || false,
                });
              }
            }
          }
        }
      }
    }
    let systemPrompt = extractSystemPromptText(params.systemPrompt);
    const tools = normalizeToolsForBrain(params.tools);
    const thinkingConfig = normalizeThinkingConfig(params.thinkingConfig as any);
    let modelName = params.options?.model || 'brain-default';

    yield { type: 'stream_request_start' as const };

    const messageId = `msg_brain_${Date.now()}`;
    let isMessageStarted = false;
    let blockIndex = 0;
    let isTextBlockOpen = false;
    let activeText = '';
    let isThinkingBlockOpen = false;
    let activeThinking = '';
    let activeSignature = '';
    const contentBlocks: any[] = [];
    let outputTokens = 0;
    let inputTokens = 10;

    const closeOpenBlocks = function* () {
      if (isTextBlockOpen) {
        yield {
          type: 'stream_event' as const,
          event: { type: 'content_block_stop' as const, index: blockIndex },
        };
        contentBlocks.push({ type: 'text', text: activeText });
        isTextBlockOpen = false;
        blockIndex++;
      }
      if (isThinkingBlockOpen) {
        yield {
          type: 'stream_event' as const,
          event: { type: 'content_block_stop' as const, index: blockIndex },
        };
        contentBlocks.push({
          type: 'thinking',
          thinking: activeThinking,
          signature: activeSignature || '',
        });
        isThinkingBlockOpen = false;
        blockIndex++;
      }
    };

    try {
      let currentSessionId: string | undefined;
      try {
        currentSessionId = (params as any).sessionId || getSessionId();
      } catch {
        currentSessionId = undefined;
      }

      // Context compilation seam
      let compiledContext: CompiledContext | undefined = (params as any).compiledContext;
      if (!compiledContext && contextProvider) {
        // Extract the last user prompt from formattedMessages
        let lastUserQuery = '';
        for (let i = formattedMessages.length - 1; i >= 0; i--) {
          if (formattedMessages[i].role === 'user') {
            const c = formattedMessages[i].content;
            lastUserQuery = typeof c === 'string' ? c : JSON.stringify(c);
            break;
          }
        }
        if (lastUserQuery) {
          try {
            compiledContext = await contextProvider.buildForTurn({
              sessionId: currentSessionId,
              userQuery: lastUserQuery,
            });
          } catch (err: any) {
            // Malformed data errors from backend must fail the dispatch
            if (err.message && err.message.includes('malformed')) {
              yield createAssistantAPIErrorMessage({
                content: err.message,
                apiError: 'internal_server_error' as any,
              });
              return;
            }
          }
        }
      }

      if (compiledContext && compiledContext.hasContext) {
        systemPrompt = `${systemPrompt}\n\n${compiledContext.serializedPromptSection}`;
      }

      const stream = client.streamText({
        sessionId: currentSessionId,
        messages: formattedMessages,
        systemPrompt,
        tools,
        thinkingConfig,
        model: modelName,
        signal: params.signal,
      });

      for await (const chunk of stream) {
        if (params.signal?.aborted) {
          break;
        }

        if (chunk.type === 'error') {
          yield createAssistantAPIErrorMessage({
            content: chunk.error || 'Brain backend execution failure',
            apiError: 'internal_server_error' as any,
          });
          return;
        }

        if (!isMessageStarted) {
          isMessageStarted = true;
          if (chunk.metadata?.inputTokens) {
            inputTokens = chunk.metadata.inputTokens;
          }
          if (chunk.metadata?.model) {
            modelName = chunk.metadata.model;
          }

          yield {
            type: 'stream_event' as const,
            event: {
              type: 'message_start' as const,
              message: {
                id: messageId,
                type: 'message' as const,
                role: 'assistant' as const,
                content: [],
                model: modelName,
                stop_reason: null,
                stop_sequence: null,
                usage: { input_tokens: inputTokens, output_tokens: 1 },
              },
            },
          };
        }

        if ((chunk.type === 'thinking_delta' && chunk.text) || (chunk.type === 'thinking' && typeof chunk.thinking === 'string')) {
          const text = chunk.text || chunk.thinking;
          if (!isThinkingBlockOpen) {
            yield* closeOpenBlocks();
            yield {
              type: 'stream_event' as const,
              event: {
                type: 'content_block_start' as const,
                index: blockIndex,
                content_block: { type: 'thinking' as const, thinking: '' },
              },
            };
            isThinkingBlockOpen = true;
          }

          activeThinking += text;
          yield {
            type: 'stream_event' as const,
            event: {
              type: 'content_block_delta' as const,
              index: blockIndex,
              delta: { type: 'thinking_delta' as const, thinking: text },
            },
          };
          outputTokens += 1;
        } else if (chunk.type === 'signature_delta' && chunk.text) {
          activeSignature += chunk.text;
          yield {
            type: 'stream_event' as const,
            event: {
              type: 'content_block_delta' as const,
              index: blockIndex,
              delta: { type: 'signature_delta' as const, signature: chunk.text },
            },
          };
        } else if (chunk.type === 'redacted_thinking' && chunk.redactedData) {
          yield* closeOpenBlocks();

          yield {
            type: 'stream_event' as const,
            event: {
              type: 'content_block_start' as const,
              index: blockIndex,
              content_block: {
                type: 'redacted_thinking' as const,
                data: chunk.redactedData,
              },
            },
          };

          yield {
            type: 'stream_event' as const,
            event: { type: 'content_block_stop' as const, index: blockIndex },
          };

          contentBlocks.push({
            type: 'redacted_thinking',
            data: chunk.redactedData,
          });
          blockIndex++;
          outputTokens += 5;
        } else if ((chunk.type === 'text_delta' && chunk.text) || (chunk.type === 'token' && typeof chunk.token === 'string')) {
          const text = chunk.text || chunk.token;
          if (!isTextBlockOpen) {
            yield* closeOpenBlocks();
            yield {
              type: 'stream_event' as const,
              event: {
                type: 'content_block_start' as const,
                index: blockIndex,
                content_block: { type: 'text' as const, text: '' },
              },
            };
            isTextBlockOpen = true;
          }

          activeText += text;
          yield {
            type: 'stream_event' as const,
            event: {
              type: 'content_block_delta' as const,
              index: blockIndex,
              delta: { type: 'text_delta' as const, text },
            },
          };
          outputTokens += 1;
        } else if (chunk.type === 'tool_use' && chunk.toolUse) {
          yield* closeOpenBlocks();

          // Yield content_block_start for tool_use
          yield {
            type: 'stream_event' as const,
            event: {
              type: 'content_block_start' as const,
              index: blockIndex,
              content_block: {
                type: 'tool_use' as const,
                id: chunk.toolUse.id,
                name: chunk.toolUse.name,
                input: {},
              },
            },
          };

          // Stream input JSON
          yield {
            type: 'stream_event' as const,
            event: {
              type: 'content_block_delta' as const,
              index: blockIndex,
              delta: {
                type: 'input_json_delta' as const,
                partial_json: JSON.stringify(chunk.toolUse.input),
              },
            },
          };

          // Close tool_use block
          yield {
            type: 'stream_event' as const,
            event: { type: 'content_block_stop' as const, index: blockIndex },
          };

          contentBlocks.push({
            type: 'tool_use',
            id: chunk.toolUse.id,
            name: chunk.toolUse.name,
            input: chunk.toolUse.input,
          });
          blockIndex++;
          outputTokens += 10;
        } else if (chunk.type === 'error') {
          yield createAssistantAPIErrorMessage({
            content: chunk.error || 'Brain backend execution failure',
            apiError: 'internal_server_error' as any,
          });
          return;
        }
      }

      if (isMessageStarted && !params.signal?.aborted) {
        yield* closeOpenBlocks();

        const hasToolUse = contentBlocks.some((b) => b.type === 'tool_use');
        const stopReason = hasToolUse ? 'tool_use' : 'end_turn';

        yield {
          type: 'stream_event' as const,
          event: {
            type: 'message_delta' as const,
            delta: { stop_reason: stopReason, stop_sequence: null },
            usage: { output_tokens: Math.max(1, outputTokens) },
          },
        };

        yield {
          type: 'stream_event' as const,
          event: { type: 'message_stop' as const },
        };

        // Notify BrainSessionStore of completed assistant turn if registered
        if (sessionStore && typeof sessionStore.recordAssistantTurn === 'function') {
          const finalText = activeText || (contentBlocks.find((b) => b.type === 'text')?.text) || '';
          sessionStore.recordAssistantTurn(
            finalText,
            { inputTokens, outputTokens },
            undefined,
            currentSessionId
          ).catch((e) => console.warn('Non-blocking sessionStore turn completion warning:', e));
        }

        // Trigger non-blocking post-turn memory consolidation if sessionStore supports it
        if (sessionStore && typeof (sessionStore as any).triggerConsolidation === 'function') {
          (sessionStore as any).triggerConsolidation().catch((_e: any) => {
            // Automatic consolidation must never disrupt user turn lifecycle
          });
        }

        yield createAssistantMessage({ content: contentBlocks });
      }
    } catch (err: any) {
      if (params.signal?.aborted) {
        return;
      }
      yield createAssistantAPIErrorMessage({
        content: err?.message || 'Brain backend client error',
        apiError: 'internal_server_error' as any,
      });
    }
  };
}
