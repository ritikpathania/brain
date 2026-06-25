import net from 'net';
import path from 'path';

// Define structures matching daemon IPC contract
export interface ServerResponse {
  status: string;
  message: string;
}

export type MessageHandler = (msg: ServerResponse) => void;
export type LogHandler = (log: string) => void;

export class SocketClient {
  private socketPath: string;
  private client: net.Socket | null = null;
  private messageListeners: MessageHandler[] = [];
  private logListeners: LogHandler[] = [];
  private buffer: string = '';
  private isConnecting: boolean = false;

  constructor(socketPath?: string) {
    const homeDir = process.env.HOME || '/tmp';
    this.socketPath = socketPath || process.env.BRAIN_SOCKET_PATH || path.join(homeDir, '.brain', 'daemon.sock');
  }

  /**
   * Connect to the Unix Domain Socket server.
   */
  public connect(onConnected?: () => void): void {
    if (this.client || this.isConnecting) return;
    this.isConnecting = true;
    this.log(`Connecting to daemon socket at ${this.socketPath}...`);

    const client = net.createConnection({ path: this.socketPath });

    client.on('connect', () => {
      this.client = client;
      this.isConnecting = false;
      this.log('Successfully connected to Relational Memory daemon!');
      if (onConnected) onConnected();
    });

    client.on('data', (data) => {
      this.buffer += data.toString();
      let newlineIdx;
      while ((newlineIdx = this.buffer.indexOf('\n')) !== -1) {
        const line = this.buffer.slice(0, newlineIdx).trim();
        this.buffer = this.buffer.slice(newlineIdx + 1);
        if (line) {
          try {
            const parsed: ServerResponse = JSON.parse(line);
            this.emitMessage(parsed);
          } catch (e) {
            this.log(`[Error Parsing JSON]: ${line} | Detail: ${e}`);
          }
        }
      }
    });

    client.on('error', (err: any) => {
      if (err.code === 'ENOENT') {
        this.log(`Daemon socket not found at ${this.socketPath}. Is the daemon running?`);
      } else {
        this.log(`Connection error: ${err.message}`);
      }
      this.cleanup();
      // Retry connection after 2 seconds
      setTimeout(() => this.connect(onConnected), 2000);
    });

    client.on('end', () => {
      this.log('Daemon closed the connection.');
      this.cleanup();
      // Retry connection after 2 seconds
      setTimeout(() => this.connect(onConnected), 2000);
    });
  }

  private cleanup(): void {
    this.isConnecting = false;
    if (this.client) {
      this.client.destroy();
      this.client = null;
    }
  }

  /**
   * Send a JSON string payload to the daemon socket.
   */
  public send(action: string, payload: string): void {
    if (!this.client) {
      this.log(`[Pending Connection] Cannot send action: '${action}'. Waiting to connect...`);
      return;
    }
    const message = JSON.stringify({ action, payload }) + '\n';
    this.client.write(message);
  }

  /**
   * Register a callback for incoming messages from the daemon.
   */
  public onMessage(handler: MessageHandler): () => void {
    this.messageListeners.push(handler);
    return () => {
      this.messageListeners = this.messageListeners.filter(h => h !== handler);
    };
  }

  /**
   * Register a callback for CLI client log output.
   */
  public onLog(handler: LogHandler): () => void {
    this.logListeners.push(handler);
    return () => {
      this.logListeners = this.logListeners.filter(h => h !== handler);
    };
  }

  private emitMessage(msg: ServerResponse): void {
    for (const listener of this.messageListeners) {
      listener(msg);
    }
  }

  private log(message: string): void {
    for (const listener of this.logListeners) {
      listener(message);
    }
  }
}
