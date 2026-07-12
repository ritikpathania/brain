import { Transport, Codec, JsonLineCodec, UdsTransport } from './internal/transport';
import { BrainError } from './errors';

/**
 * Ergonomic, transport-neutral client interface to interact with the relational memory engine.
 */
export class BrainClient {
    /**
     * Helper factory to instantiate a client communicating over a local Unix Domain Socket (UDS).
     */
    public static connectUds(socketPath: string): BrainClient {
        return new BrainClient(new UdsTransport(socketPath));
    }

    private readonly codec: Codec;

    constructor(private readonly transport: Transport, codec?: Codec) {
        this.codec = codec || new JsonLineCodec();
    }

    /**
     * Sends a request to the engine and returns the response.
     */
    public async send<TResponse = any>(
        requestPayload: any,
        options?: { signal?: AbortSignal }
    ): Promise<TResponse> {
        if (options?.signal?.aborted) {
            throw new BrainError('Operation aborted');
        }

        const encoded = this.codec.encode(requestPayload);

        const executePromise = this.transport.send(encoded);

        let abortListener: (() => void) | null = null;
        const resultPromise = new Promise<Uint8Array>((resolve, reject) => {
            if (options?.signal) {
                abortListener = () => {
                    reject(new BrainError('Operation aborted'));
                };
                options.signal.addEventListener('abort', abortListener);
            }
            executePromise.then(resolve, reject);
        });

        try {
            const rawResponse = await resultPromise;
            const decoder = new (this.codec.constructor as any)() as Codec;
            const frames = decoder.feed(rawResponse);
            if (frames.length === 0) {
                throw new BrainError('Empty or incomplete response frame received');
            }
            return frames[0] as TResponse;
        } finally {
            if (options?.signal && abortListener) {
                options.signal.removeEventListener('abort', abortListener);
            }
        }
    }

    /**
     * Dispatches a streaming request and yields typed event progress chunks.
     */
    public async *stream<TEvent = any>(
        requestPayload: any,
        options?: { signal?: AbortSignal }
    ): AsyncIterable<TEvent> {
        if (options?.signal?.aborted) {
            throw new BrainError('Operation aborted');
        }

        const encoded = this.codec.encode(requestPayload);

        const decoder = new (this.codec.constructor as any)() as Codec;
        const byteStream = this.transport.stream(encoded);

        let abortListener: (() => void) | null = null;
        let isAborted = false;

        if (options?.signal) {
            abortListener = () => {
                isAborted = true;
            };
            options.signal.addEventListener('abort', abortListener);
        }

        try {
            for await (const chunk of byteStream) {
                if (isAborted) {
                    throw new BrainError('Operation aborted');
                }
                const frames = decoder.feed(chunk);
                for (const frame of frames) {
                    yield frame as TEvent;
                }
            }
        } finally {
            if (options?.signal && abortListener) {
                options.signal.removeEventListener('abort', abortListener);
            }
        }
    }
}
