/**
 * Inc 15: transport-level daemon liveness probe. Opens a bare connection
 * to the UDS path — no protocol bytes, no invented health action — and
 * reports whether anything is accepting. Resolves (never rejects) and
 * destroys the socket either way.
 */
import * as net from 'net';

export function probeDaemonSocket(socketPath: string, timeoutMs = 1500): Promise<boolean> {
  return new Promise((resolve) => {
    let settled = false;
    const socket = net.createConnection(socketPath);
    const timer = setTimeout(() => finish(false), timeoutMs);
    timer.unref?.();

    function finish(ok: boolean): void {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      socket.destroy();
      resolve(ok);
    }

    socket.once('connect', () => finish(true));
    socket.once('error', () => finish(false));
  });
}
