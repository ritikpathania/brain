import { isRecord, requiredString } from './protocol/json.js';
import { UdsResponseError } from './transport/udsClient.js';

export type SessionMessage = Readonly<{
  role: 'user' | 'assistant' | 'thinking' | 'tool' | 'system';
  content: string;
}>;

export type BrainSession = Readonly<{
  sessionId: string;
  title: string;
  systemPrompt?: string;
  messages: readonly SessionMessage[];
}>;

export type BrainSessionSummary = Readonly<{
  sessionId: string;
  title: string;
  messageCount: number;
  pinned: boolean;
  archived: boolean;
}>;

export type SessionTransport = Readonly<{
  request(action: string, body: unknown): Promise<unknown>;
}>;

export class SessionNotFoundError extends Error {
  public constructor(public readonly sessionId: string) {
    super(`session_not_found: Brain did not find session '${sessionId}'`);
    this.name = 'SessionNotFoundError';
  }
}

export class SessionClient {
  public constructor(private readonly transport: SessionTransport) {}

  public async create(input: Readonly<{ title?: string; systemPrompt?: string }> = {}): Promise<Readonly<{ sessionId: string }>> {
    const response = await this.transport.request('v1/session/create', {
      ...(input.title === undefined ? {} : { title: input.title }),
      ...(input.systemPrompt === undefined ? {} : { system_prompt: input.systemPrompt }),
    });
    if (!isRecord(response)) throw new Error('malformed_session_response: create response must be an object');
    return Object.freeze({
      sessionId: requiredString(response, 'session_id', message => new Error(`malformed_session_response: ${message}`)),
    });
  }

  public async list(): Promise<readonly BrainSessionSummary[]> {
    const response = await this.transport.request('v1/session/list', {});
    if (!isRecord(response) || !Array.isArray(response.sessions)) {
      throw new Error('malformed_session_response: list response must contain sessions');
    }
    return Object.freeze(response.sessions.map(decodeSessionSummary));
  }

  public async load(sessionId: string): Promise<BrainSession> {
    try {
      const response = await this.transport.request('v1/session/load', { session_id: sessionId });
      if (!isRecord(response)) throw new Error('malformed_session_response: load response must be an object');
      return decodeSession(response.session);
    } catch (error) {
      if (error instanceof UdsResponseError && error.code === 'session_not_found') {
        throw new SessionNotFoundError(sessionId);
      }
      throw error;
    }
  }
}

function decodeSessionSummary(value: unknown): BrainSessionSummary {
  if (!isRecord(value)) throw new Error('malformed_session_response: session summary must be an object');
  // daemon::server::protocol::SessionSummaryDto serializes these as camelCase.
  const messageCount = value.messageCount;
  if (typeof messageCount !== 'number' || !Number.isSafeInteger(messageCount) || messageCount < 0) {
    throw new Error('malformed_session_response: messageCount must be a non-negative integer');
  }
  if (typeof value.pinned !== 'boolean' || typeof value.archived !== 'boolean') {
    throw new Error('malformed_session_response: pinned and archived must be booleans');
  }
  return Object.freeze({
    sessionId: requiredString(value, 'sessionId', reason => new Error(`malformed_session_response: ${reason}`)),
    title: requiredString(value, 'title', reason => new Error(`malformed_session_response: ${reason}`)),
    messageCount,
    pinned: value.pinned,
    archived: value.archived,
  });
}

function decodeSession(value: unknown): BrainSession {
  if (!isRecord(value)) throw new Error('malformed_session_response: session must be an object');
  const messages = value.messages;
  if (!Array.isArray(messages)) throw new Error('malformed_session_response: session messages must be an array');
  const decodedMessages = messages.map(message => {
    if (!isRecord(message)) throw new Error('malformed_session_response: message must be an object');
    const role = requiredString(message, 'role', reason => new Error(`malformed_session_response: ${reason}`));
    if (!['user', 'assistant', 'thinking', 'tool', 'system'].includes(role)) {
      throw new Error(`malformed_session_response: unsupported message role '${role}'`);
    }
    return Object.freeze({
      role: role as SessionMessage['role'],
      content: requiredString(message, 'content', reason => new Error(`malformed_session_response: ${reason}`)),
    });
  });
  const systemPrompt = value.systemPrompt;
  if (systemPrompt !== undefined && typeof systemPrompt !== 'string') {
    throw new Error('malformed_session_response: systemPrompt must be a string');
  }
  return Object.freeze({
    sessionId: requiredString(value, 'sessionId', reason => new Error(`malformed_session_response: ${reason}`)),
    title: requiredString(value, 'title', reason => new Error(`malformed_session_response: ${reason}`)),
    ...(systemPrompt === undefined ? {} : { systemPrompt }),
    messages: Object.freeze(decodedMessages),
  });
}
