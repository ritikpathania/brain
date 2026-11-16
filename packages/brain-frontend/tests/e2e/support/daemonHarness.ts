import { existsSync, mkdtempSync, rmSync } from 'node:fs';
import { createServer } from 'node:net';
import { join } from 'node:path';

export function requireDaemonBinary(): string {
  const binary = process.env.BRAIN_DAEMON_BIN;
  if (binary === undefined || binary.length === 0) {
    throw new Error('BRAIN_DAEMON_BIN is required for owned frontend real-daemon E2E tests');
  }
  if (!existsSync(binary)) {
    throw new Error(`BRAIN_DAEMON_BIN does not exist: ${binary}`);
  }
  return binary;
}

/** Owns an isolated daemon process and all runtime paths for each E2E test. */
export class DaemonHarness {
  public readonly socketPath: string;

  private constructor(
    private readonly runtimeRoot: string,
    private process: ReturnType<typeof Bun.spawn>,
    private readonly spawnProcess: () => ReturnType<typeof Bun.spawn>,
  ) {
    this.socketPath = join(runtimeRoot, 'brain.sock');
  }

  public static async start(binary: string, chunkDelayMs = 1): Promise<DaemonHarness> {
    const runtimeRoot = mkdtempSync('/tmp/bfe-');
    const socketPath = join(runtimeRoot, 'brain.sock');
    const healthPort = await availableTcpPort();
    const spawnProcess = () => Bun.spawn([binary, 'daemon', 'run'], {
      env: {
        ...process.env,
        HOME: runtimeRoot,
        XDG_CONFIG_HOME: join(runtimeRoot, 'xdg-config'),
        XDG_STATE_HOME: join(runtimeRoot, 'xdg-state'),
        BRAIN_CONFIG_DIR: runtimeRoot,
        BRAIN_SOCKET_PATH: socketPath,
        BRAIN_PID_PATH: join(runtimeRoot, 'brain.pid'),
        BRAIN_DB_PATH: join(runtimeRoot, 'brain.db'),
        BRAIN_ANALYTICS_DB_PATH: join(runtimeRoot, 'analytics.db'),
        BRAIN_HEALTH_PORT: String(healthPort),
        BRAIN_MOCK_CHUNK_DELAY_MS: String(chunkDelayMs),
      },
      stdout: 'pipe',
      stderr: 'pipe',
    });
    const harness = new DaemonHarness(runtimeRoot, spawnProcess(), spawnProcess);
    try {
      await waitFor(() => existsSync(socketPath), 5_000, 'daemon did not create its UDS socket');
      return harness;
    } catch (error) {
      await harness.close();
      throw error;
    }
  }

  public async stop(): Promise<void> {
    if (this.process.exitCode === null) {
      this.process.kill('SIGTERM');
      await Promise.race([
        this.process.exited,
        new Promise<void>(resolve => setTimeout(resolve, 3_000)),
      ]);
    }
    if (this.process.exitCode === null) {
      this.process.kill('SIGKILL');
      await this.process.exited;
    }
  }

  public async restart(): Promise<void> {
    await this.stop();
    this.process = this.spawnProcess();
    await waitFor(() => existsSync(this.socketPath), 5_000, 'restarted daemon did not create its UDS socket');
  }

  public async close(): Promise<void> {
    await this.stop();
    rmSync(this.runtimeRoot, { recursive: true, force: true });
  }
}

async function availableTcpPort(): Promise<number> {
  const server = createServer();
  await new Promise<void>((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', () => {
      server.off('error', reject);
      resolve();
    });
  });
  const address = server.address();
  if (address === null || typeof address === 'string') {
    server.close();
    throw new Error('could not reserve an ephemeral TCP health port');
  }
  await new Promise<void>(resolve => server.close(() => resolve()));
  return address.port;
}

async function waitFor(predicate: () => boolean, timeoutMs: number, message: string): Promise<void> {
  const started = Date.now();
  while (!predicate()) {
    if (Date.now() - started >= timeoutMs) throw new Error(message);
    await new Promise(resolve => setTimeout(resolve, 25));
  }
}
