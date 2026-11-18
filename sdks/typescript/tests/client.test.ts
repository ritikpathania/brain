import { describe, expect, test } from "bun:test";
import { BrainClient } from "../src/client";
import { BrainError } from "../src/errors";
import { Transport, TransportStream, Codec, JsonLineCodec } from "../src/internal/transport";

class MockTransport implements Transport {
    public sentPayloads: Uint8Array[] = [];
    public responseQueue: Uint8Array[] = [];
    public streamMock: TransportStream | null = null;

    async send(payload: Uint8Array): Promise<Uint8Array> {
        this.sentPayloads.push(payload);
        const resp = this.responseQueue.shift();
        if (!resp) {
            throw new Error("No queued response in MockTransport");
        }
        return resp;
    }

    stream(payload: Uint8Array): TransportStream {
        this.sentPayloads.push(payload);
        if (this.streamMock) {
            return this.streamMock;
        }
        throw new Error("No stream mock configured");
    }
}

describe("BrainClient", () => {
    test("sends request and parses response via JsonLineCodec", async () => {
        const transport = new MockTransport();
        const encoder = new TextEncoder();
        const client = new BrainClient(transport);

        transport.responseQueue.push(
            encoder.encode(JSON.stringify({ status: "success", body: { ok: true } }) + "\n")
        );

        const res = await client.send({ action: "v1/status" });
        expect(res).toEqual({ status: "success", body: { ok: true } });
        expect(transport.sentPayloads.length).toBe(1);

        const sentDecoded = JSON.parse(new TextDecoder().decode(transport.sentPayloads[0]));
        expect(sentDecoded.action).toBe("v1/status");
    });

    test("handles abort signal cancellation", async () => {
        const transport = new MockTransport();
        const client = new BrainClient(transport);
        const controller = new AbortController();
        controller.abort();

        expect(client.send({ action: "v1/status" }, { signal: controller.signal })).rejects.toThrow(
            BrainError
        );
    });

    test("handles empty response frame error", async () => {
        const transport = new MockTransport();
        const client = new BrainClient(transport);

        transport.responseQueue.push(new Uint8Array(0));

        expect(client.send({ action: "v1/status" })).rejects.toThrow(
            "Empty or incomplete response frame"
        );
    });

    test("iterates over async stream events cleanly", async () => {
        const transport = new MockTransport();
        const encoder = new TextEncoder();

        const mockStream: TransportStream = {
            async *[Symbol.asyncIterator]() {
                yield encoder.encode(JSON.stringify({ type: "stream_start", stream_id: "s1" }) + "\n");
                yield encoder.encode(JSON.stringify({ type: "stream_chunk", stream_id: "s1", chunk: "Hello " }) + "\n");
                yield encoder.encode(JSON.stringify({ type: "stream_chunk", stream_id: "s1", chunk: "World" }) + "\n");
                yield encoder.encode(JSON.stringify({ type: "stream_end", stream_id: "s1" }) + "\n");
            },
            cancel() {}
        };

        transport.streamMock = mockStream;
        const client = new BrainClient(transport);

        const events: any[] = [];
        for await (const event of client.stream({ action: "v1/chat/stream" })) {
            events.push(event);
        }

        expect(events.length).toBe(4);
        expect(events[0].type).toBe("stream_start");
        expect(events[1].chunk).toBe("Hello ");
        expect(events[2].chunk).toBe("World");
        expect(events[3].type).toBe("stream_end");
    });
});
