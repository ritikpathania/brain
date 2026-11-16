import { describe, expect, test } from 'bun:test';

import {
  FrontendHandshakeError,
  createHandshakeRequest,
  decodeHandshakeResponse,
} from '../../src/brain/handshake.js';

const validResponse = {
  version: '1.0',
  type: 'Response',
  id: 'handshake-1',
  status: 'success',
  body: JSON.stringify({
    selectedProtocolVersion: 1,
    backendName: 'brain-daemon',
    backendVersion: '1.1.0',
    daemonState: 'ready',
    supportedCapabilities: [
      'generation.cancel.v1',
      'generation.stream.v1',
      'sessions.v1',
    ],
    acceptedOptionalCapabilities: [],
  }),
};

describe('owned frontend handshake', () => {
  test('uses the Brain-owned frontend identity and required capability contract', () => {
    expect(createHandshakeRequest({ version: '0.1.0', buildId: 'test-build' })).toEqual({
      protocol: 'brain.frontend',
      minimumProtocolVersion: 1,
      maximumProtocolVersion: 1,
      frontendName: 'brain-frontend',
      frontendVersion: '0.1.0',
      frontendBuildId: 'test-build',
      requiredCapabilities: ['sessions.v1', 'generation.stream.v1', 'generation.cancel.v1'],
      optionalCapabilities: [],
    });
  });

  test('decodes and freezes a compatible daemon capability snapshot', () => {
    const snapshot = decodeHandshakeResponse(validResponse);

    expect(snapshot.backendName).toBe('brain-daemon');
    expect(snapshot.selectedProtocolVersion).toBe(1);
    expect(Object.isFrozen(snapshot)).toBe(true);
    expect(Object.isFrozen(snapshot.supportedCapabilities)).toBe(true);
  });

  test('reports a typed incompatibility rather than accepting an unsupported protocol', () => {
    expect(() => decodeHandshakeResponse({
      version: '1.0',
      type: 'Error',
      id: 'handshake-1',
      status: 'error',
      body: JSON.stringify({
        code: 'unsupported_protocol_version',
        message: 'frontend protocol range 2..2 is unsupported',
        supportedMinimumProtocolVersion: 1,
        supportedMaximumProtocolVersion: 1,
        missingRequiredCapabilities: [],
      }),
    })).toThrow(FrontendHandshakeError);

    try {
      decodeHandshakeResponse({ type: 'Response', status: 'success', body: '{}' });
      throw new Error('expected malformed handshake to throw');
    } catch (error) {
      expect(error).toBeInstanceOf(FrontendHandshakeError);
      expect((error as FrontendHandshakeError).code).toBe('malformed_handshake_response');
    }
  });
});
