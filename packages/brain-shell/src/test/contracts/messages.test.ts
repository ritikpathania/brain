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

  test('createAssistantMessage produces assistant envelope', () => {
    const m = createAssistantMessage('hi there');
    expect(m.type).toBe('assistant');
    expect(m.message.content[0]).toEqual({ type: 'text', text: 'hi there' });
  });

  test('createAssistantAPIErrorMessage marks isError', () => {
    const m = createAssistantAPIErrorMessage('daemon unreachable');
    expect(m.isError).toBe(true);
    expect(JSON.stringify(m.message.content)).toContain('daemon unreachable');
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
