/**
 * Brain-owned session identity and working-directory state.
 * Replaces vendor bootstrap/state.js + utils/cwd.js + types/ids.js consumers.
 */

const BRAND: unique symbol = Symbol('SessionId');
export type SessionId = string & { readonly [BRAND]: true };

export function asSessionId(value: string): SessionId {
  if (!value) throw new Error('SessionId must be non-empty');
  return value as SessionId;
}

let current = asSessionId(
  process.env.BRAIN_SESSION_ID ??
    `ses_${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`,
);
const originalCwd = process.cwd();

export function getSessionId(): SessionId {
  return current;
}

export function setSessionId(id: SessionId): void {
  current = id;
}

export function switchSession(id: SessionId): void {
  current = id;
}

export function getOriginalCwd(): string {
  return originalCwd;
}

export function getCwd(): string {
  return process.cwd();
}

/** Deprecated compat flags — Brain config owns real feature flags. */
export function getKairosActive(): boolean {
  return false;
}

/** Deprecated compat flags — Brain config owns real feature flags. */
export function getUserMsgOptIn(): boolean {
  return false;
}
