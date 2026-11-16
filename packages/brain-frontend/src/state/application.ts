import type { BrainSession } from '../brain/sessions.js';
import type { ConnectionState } from '../brain/recovery.js';

/**
 * Headless projection boundary. A later presentation package may consume this
 * immutable state but cannot become a second source of session truth.
 */
export type FrontendApplicationState = Readonly<{
  connection: ConnectionState;
  session?: BrainSession;
}>;

export function initialApplicationState(): FrontendApplicationState {
  return Object.freeze({ connection: 'checking' });
}
