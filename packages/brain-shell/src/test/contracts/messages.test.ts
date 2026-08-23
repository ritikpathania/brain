import { describe, expect, test } from 'bun:test';
import {
  createAssistantAPIErrorMessage,
  createAssistantMessage,
  createUserMessage,
  extractTag,
  getMessagesAfterCompactBoundary,
} from '../../contracts/messages.js';

describe('contracts/messages', () => {
  test('createUserMessage wraps content in the envelope shape', () => {
    const m = createUserMessage('hello');
    expect(m.type).toBe('user');
    expect(m.message.content).toBe('hello');
    expect(typeof m.uuid).toBe('string');
  });

  test('createAssistantMessage wraps string content as a text block', () => {
    const m = createAssistantMessage({ content: 'hi there' });
    expect(m.type).toBe('assistant');
    expect(m.message.content[0]).toEqual({ type: 'text', text: 'hi there' });
  });

  test('createAssistantMessage passes block arrays through untouched', () => {
    const blocks = [
      { type: 'thinking', thinking: 'hmm' },
      { type: 'text', text: 'answer' },
    ] as const;
    const m = createAssistantMessage({ content: [...blocks] });
    expect((m.message.content as unknown[])[0]).toEqual(blocks[0]);
    expect((m.message.content as unknown[])[1]).toEqual(blocks[1]);
  });

  test("createAssistantMessage maps empty string to '(no content)'", () => {
    const m = createAssistantMessage({ content: '' });
    expect(m.message.content).toEqual([{ type: 'text', text: '(no content)' }]);
  });

  test('createAssistantMessage carries usage and isVirtual', () => {
    const m = createAssistantMessage({
      content: 'x',
      usage: { input_tokens: 3, output_tokens: 4 },
      isVirtual: true,
    });
    expect(m.usage).toEqual({ input_tokens: 3, output_tokens: 4 });
    expect(m.isVirtual).toBe(true);
  });

  test('createAssistantAPIErrorMessage marks the turn as failed without rewriting content', () => {
    const m = createAssistantAPIErrorMessage({
      content: 'daemon unreachable',
      apiError: 'internal_server_error',
    });
    expect(m.isError).toBe(true);
    expect(m.isApiErrorMessage).toBe(true);
    expect(m.apiError).toBe('internal_server_error');
    expect(m.message.content).toEqual([{ type: 'text', text: 'daemon unreachable' }]);
  });

  test('extractTag finds tagged content', () => {
    expect(extractTag('<think>abc</think>', 'think')).toBe('abc');
    expect(extractTag('no tags', 'think')).toBeNull();
  });

  test('getMessagesAfterCompactBoundary drops messages up to boundary', () => {
    const msgs = [
      createUserMessage('a'),
      { type: 'system', subtype: 'compact_boundary', uuid: 'b1', timestamp: '' } as never,
      createUserMessage('after'),
    ];
    expect(getMessagesAfterCompactBoundary(msgs)).toHaveLength(1);
  });
});
