import { describe, expect, test } from 'bun:test';
import {
  asSessionId,
  getCwd,
  getOriginalCwd,
  getSessionId,
  setSessionId,
  switchSession,
} from '../../contracts/session.js';

describe('contracts/session', () => {
  test('session id is stable and switchable', () => {
    const first = getSessionId();
    expect(first).toBe(asSessionId(first));
    switchSession(asSessionId('session-beta'));
    expect(getSessionId()).toBe('session-beta');
    setSessionId(asSessionId(first));
  });

  test('asSessionId rejects empty values', () => {
    expect(() => asSessionId('')).toThrow();
  });

  test('cwd getters return absolute paths', () => {
    expect(getCwd().startsWith('/')).toBe(true);
    expect(getOriginalCwd().startsWith('/')).toBe(true);
  });
});
