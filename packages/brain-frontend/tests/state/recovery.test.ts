import { describe, expect, test } from 'bun:test';

import {
  BrainConnectionRecovery,
  BRAIN_NEVER_AUTOMATICALLY_RETRIED,
  RecoveryError,
} from '../../src/brain/recovery.js';
import { FrontendHandshakeError } from '../../src/brain/handshake.js';
import { SessionNotFoundError } from '../../src/brain/sessions.js';

describe('connection recovery', () => {
  test('re-handshakes and reloads Brain authority without replaying a generation', async () => {
    const calls: string[] = [];
    const recovery = new BrainConnectionRecovery({
      handshake: async () => {
        calls.push('handshake');
      },
      reloadSession: async sessionId => {
        calls.push(`reload:${sessionId}`);
        return { sessionId };
      },
    });

    recovery.markDisconnected(new Error('socket EOF'));
    await expect(recovery.recover('session-a')).resolves.toEqual({ kind: 'recovered', sessionId: 'session-a' });

    expect(calls).toEqual(['handshake', 'reload:session-a']);
    expect(recovery.snapshot().state).toBe('ready');
    expect(recovery.snapshot().epoch).toBe(1);
    expect(BRAIN_NEVER_AUTOMATICALLY_RETRIED).toContain('generation.submit');
    expect(BRAIN_NEVER_AUTOMATICALLY_RETRIED).toContain('permission.resolve');
  });

  test('stops recovery as incompatible rather than retrying a rejected protocol handshake', async () => {
    let attempts = 0;
    const recovery = new BrainConnectionRecovery({
      handshake: async () => {
        attempts += 1;
        throw new FrontendHandshakeError('unsupported_protocol_version', 'v1 is unavailable');
      },
      reloadSession: async sessionId => ({ sessionId }),
      maxAttempts: 3,
    });

    recovery.markDisconnected(new Error('socket EOF'));
    await expect(recovery.recover('session-a'))
      .rejects.toMatchObject({ code: 'incompatible_after_reconnect' } satisfies Partial<RecoveryError>);
    expect(attempts).toBe(1);
    expect(recovery.snapshot().state).toBe('incompatible');
  });

  test('reports an authoritative missing session without creating or retrying a replacement', async () => {
    let reloadAttempts = 0;
    const recovery = new BrainConnectionRecovery({
      handshake: async () => {},
      reloadSession: async sessionId => {
        reloadAttempts += 1;
        throw new SessionNotFoundError(sessionId);
      },
      maxAttempts: 3,
    });

    recovery.markDisconnected(new Error('socket EOF'));
    await expect(recovery.recover('session-a'))
      .rejects.toMatchObject({ code: 'session_missing_after_reconnect' } satisfies Partial<RecoveryError>);
    expect(reloadAttempts).toBe(1);
    expect(recovery.snapshot().state).toBe('failed');
  });
});
