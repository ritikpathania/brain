import * as net from 'net';
import { TransportError } from '../errors';

/**
 * Transport-neutral interface for shipping byte payloads.
 */
export interface Transport {
    send(data: Uint8Array): Promise<Uint8Array>;
    stream(data: Uint8Array): AsyncIterable<Uint8Array>;
}

/**
 * Generic serialization and framing codec abstraction.
 */
export interface Codec {
    encode(value: unknown): Uint8Array;
    feed(chunk: Uint8Array): unknown[];
}

/**
 * Codec implementation using newline delimited JSON lines.
 */
export class JsonLineCodec implements Codec {
    private buffer = '';

    public encode(value: unknown): Uint8Array {
        return new TextEncoder().encode(JSON.stringify(value) + '\n');
    }

    public feed(chunk: Uint8Array): unknown[] {
        const text = new TextDecoder('utf-8').decode(chunk, { stream: true });
        this.buffer += text;
        const frames = this.buffer.split('\n');
        this.buffer = frames.pop() || '';
        
        const results: unknown[] = [];
        for (const frame of frames) {
            if (frame.trim()) {
                results.push(JSON.parse(frame));
            }
        }
        return results;
    }
}

/**
 * Unix Domain Socket byte-level transport.
 */
export class UdsTransport implements Transport {
    constructor(private readonly socketPath: string) {}

    public async send(data: Uint8Array): Promise<Uint8Array> {
        return new Promise((resolve, reject) => {
            let socket: net.Socket;
            try {
                socket = net.connect(this.socketPath);
            } catch (err: any) {
                return reject(new TransportError(`Failed to connect to socket: ${err.message}`, err));
            }

            let responseBuffer = Buffer.alloc(0);

            socket.on('connect', () => {
                socket.write(data);
            });

            socket.on('data', (chunk) => {
                responseBuffer = Buffer.concat([responseBuffer, chunk]);
            });

            socket.on('end', () => {
                socket.destroy();
                resolve(new Uint8Array(responseBuffer));
            });

            socket.on('error', (err) => {
                socket.destroy();
                reject(new TransportError(`UDS connection error: ${err.message}`, err));
            });
        });
    }

    public stream(data: Uint8Array): AsyncIterable<Uint8Array> {
        const socketPath = this.socketPath;
        return {
            [Symbol.asyncIterator]() {
                let socket: net.Socket | null = null;
                const queue: Uint8Array[] = [];
                let error: Error | null = null;
                let done = false;
                let pendingResolve: ((value: IteratorResult<Uint8Array>) => void) | null = null;

                const push = (chunk: Uint8Array) => {
                    if (pendingResolve) {
                        const resolve = pendingResolve;
                        pendingResolve = null;
                        resolve({ value: chunk, done: false });
                    } else {
                        queue.push(chunk);
                    }
                };

                const finish = () => {
                    done = true;
                    if (pendingResolve) {
                        const resolve = pendingResolve;
                        pendingResolve = null;
                        resolve({ value: undefined as any, done: true });
                    }
                };

                const fail = (err: Error) => {
                    error = new TransportError(`UDS stream error: ${err.message}`, err);
                    if (pendingResolve) {
                        const resolve = pendingResolve;
                        pendingResolve = null;
                        resolve(Promise.reject(error));
                    }
                };

                try {
                    socket = net.connect(socketPath);
                    socket.on('connect', () => {
                        socket!.write(data);
                    });
                    socket.on('data', (chunk) => {
                        push(new Uint8Array(chunk));
                    });
                    socket.on('end', () => {
                        socket!.destroy();
                        finish();
                    });
                    socket.on('error', (err) => {
                        socket!.destroy();
                        fail(err);
                    });
                } catch (err: any) {
                    fail(err);
                }

                return {
                    async next(): Promise<IteratorResult<Uint8Array>> {
                        if (error) {
                            throw error;
                        }
                        if (queue.length > 0) {
                            return { value: queue.shift()!, done: false };
                        }
                        if (done) {
                            return { value: undefined as any, done: true };
                        }
                        return new Promise((resolve) => {
                            pendingResolve = resolve;
                        });
                    },
                    async return(): Promise<IteratorResult<Uint8Array>> {
                        if (socket) {
                            socket.destroy();
                        }
                        done = true;
                        return { value: undefined as any, done: true };
                    }
                };
            }
        };
    }
}

/**
 * Programmable Mock transport for testing and simulation.
 */
export class MockTransport implements Transport {
    private sendHandler?: (data: Uint8Array) => Promise<Uint8Array>;
    private streamHandler?: (data: Uint8Array) => AsyncIterable<Uint8Array>;

    constructor() {}

    public setSendHandler(handler: (data: Uint8Array) => Promise<Uint8Array>): void {
        this.sendHandler = handler;
    }

    public setStreamHandler(handler: (data: Uint8Array) => AsyncIterable<Uint8Array>): void {
        this.streamHandler = handler;
    }

    public async send(data: Uint8Array): Promise<Uint8Array> {
        if (this.sendHandler) {
            return this.sendHandler(data);
        }
        return data; // Echo default
    }

    public stream(data: Uint8Array): AsyncIterable<Uint8Array> {
        if (this.streamHandler) {
            return this.streamHandler(data);
        }

        const half = Math.ceil(data.length / 2);
        const chunk1 = data.slice(0, half);
        const chunk2 = data.slice(half);

        return {
            async *[Symbol.asyncIterator]() {
                yield chunk1;
                yield chunk2;
            }
        };
    }
}
