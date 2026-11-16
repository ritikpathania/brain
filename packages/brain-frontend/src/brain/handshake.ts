import {
  isRecord,
  parseJsonObject,
  requiredString,
  requiredStringArray,
  type JsonRecord,
} from './protocol/json.js';

export const BRAIN_FRONTEND_PROTOCOL = 'brain.frontend';
export const BRAIN_FRONTEND_PROTOCOL_VERSION = 1;
export const BRAIN_FRONTEND_REQUIRED_CAPABILITIES = Object.freeze([
  'sessions.v1',
  'generation.stream.v1',
  'generation.cancel.v1',
] as const);

export type FrontendHandshakeRequest = Readonly<{
  protocol: typeof BRAIN_FRONTEND_PROTOCOL;
  minimumProtocolVersion: number;
  maximumProtocolVersion: number;
  frontendName: 'brain-frontend';
  frontendVersion: string;
  frontendBuildId: string | null;
  requiredCapabilities: readonly string[];
  optionalCapabilities: readonly string[];
}>;

export type NegotiatedCapabilities = Readonly<{
  selectedProtocolVersion: number;
  backendName: string;
  backendVersion: string;
  daemonState: string;
  supportedCapabilities: readonly string[];
  acceptedOptionalCapabilities: readonly string[];
}>;

export class FrontendHandshakeError extends Error {
  public constructor(
    public readonly code: string,
    message: string,
  ) {
    super(`${code}: ${message}`);
    this.name = 'FrontendHandshakeError';
  }
}

export function createHandshakeRequest(options: Readonly<{
  version: string;
  buildId?: string | null;
}>): FrontendHandshakeRequest {
  if (options.version.length === 0) {
    throw new FrontendHandshakeError('invalid_frontend_identity', 'frontend version is required');
  }
  return Object.freeze({
    protocol: BRAIN_FRONTEND_PROTOCOL,
    minimumProtocolVersion: BRAIN_FRONTEND_PROTOCOL_VERSION,
    maximumProtocolVersion: BRAIN_FRONTEND_PROTOCOL_VERSION,
    frontendName: 'brain-frontend',
    frontendVersion: options.version,
    frontendBuildId: options.buildId ?? null,
    requiredCapabilities: BRAIN_FRONTEND_REQUIRED_CAPABILITIES,
    optionalCapabilities: Object.freeze([]),
  });
}

export function decodeHandshakeResponse(frame: unknown): NegotiatedCapabilities {
  if (!isRecord(frame)) {
    throw malformed('response must be an object');
  }
  if (frame.type === 'Error' || frame.status === 'error') {
    throw decodeFailure(frame);
  }
  if (frame.type !== 'Response' || frame.status !== 'success') {
    throw malformed('expected a successful versioned response');
  }

  const body = parseJsonObject(frame.body, malformed);
  const selectedProtocolVersion = body.selectedProtocolVersion;
  if (!Number.isSafeInteger(selectedProtocolVersion)) {
    throw malformed('missing integer field \'selectedProtocolVersion\'');
  }
  if (selectedProtocolVersion !== BRAIN_FRONTEND_PROTOCOL_VERSION) {
    throw new FrontendHandshakeError(
      'unsupported_protocol_version',
      `daemon selected unsupported frontend protocol ${String(selectedProtocolVersion)}`,
    );
  }

  const supportedCapabilities = requiredStringArray(body, 'supportedCapabilities', malformed);
  for (const capability of BRAIN_FRONTEND_REQUIRED_CAPABILITIES) {
    if (!supportedCapabilities.includes(capability)) {
      throw new FrontendHandshakeError(
        'missing_required_capability',
        `daemon omitted required capability '${capability}'`,
      );
    }
  }

  return Object.freeze({
    selectedProtocolVersion,
    backendName: requiredString(body, 'backendName', malformed),
    backendVersion: requiredString(body, 'backendVersion', malformed),
    daemonState: requiredString(body, 'daemonState', malformed),
    supportedCapabilities,
    acceptedOptionalCapabilities: requiredStringArray(body, 'acceptedOptionalCapabilities', malformed),
  });
}

function decodeFailure(frame: JsonRecord): FrontendHandshakeError {
  const body = parseJsonObject(frame.body, malformed);
  return new FrontendHandshakeError(
    requiredString(body, 'code', malformed),
    requiredString(body, 'message', malformed),
  );
}

function malformed(message: string): FrontendHandshakeError {
  return new FrontendHandshakeError('malformed_handshake_response', message);
}
