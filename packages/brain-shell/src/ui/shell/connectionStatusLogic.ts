/** Pure projection: status-bar text for a non-connected shell. Null hides. */
import type { ConnectionState } from '../../state/connectionMonitor.js';

export function connectionStatusText(state: ConnectionState | undefined): string | null {
  if (!state || state.status === 'connected') return null;
  return `reconnecting (attempt ${state.attempt})`;
}
