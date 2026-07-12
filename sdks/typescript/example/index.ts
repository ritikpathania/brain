import { BrainClient, IngestionEnvelope } from '../src';
import { MockTransport, JsonLineCodec } from '../src/internal/transport';

async function main() {
    console.log('Starting SDK and mock transport validation...');

    const mockTransport = new MockTransport();
    const client = new BrainClient(mockTransport);
    const codec = new JsonLineCodec();

    // 1. Mock send handler
    mockTransport.setSendHandler(async (data: Uint8Array) => {
        const decoder = new JsonLineCodec();
        const frames = decoder.feed(data);
        const request = frames[0] as any;
        console.log('Mock Transport received request payload model version:', request.event_model_version);

        const responsePayload = { status: 'success', processed: true };
        return codec.encode(responsePayload);
    });

    const envelope: IngestionEnvelope = {
        event_model_version: '1.0',
        identity: {
            adapter_id: 'vscode',
            client_id: 'client-1',
            conversation_id: null,
            event_id: 'event-123',
            parent_event_id: null,
            session_id: 'session-123',
            timestamp: new Date().toISOString(),
            workspace_id: 'workspace-123'
        },
        event: {
            event_type: 'text',
            content: 'Hello relational memory engine',
            metadata: {}
        }
    };

    console.log('Sending mock envelope...');
    const result = await client.send(envelope);
    console.log('Received response:', result);
    if (!result.processed) {
        throw new Error('Test failed: response process state invalid');
    }

    // 2. Mock streaming handler
    console.log('Testing streaming response...');
    mockTransport.setStreamHandler((_data: Uint8Array) => {
        const events = [
            { step: 1, message: 'Parsing DTO' },
            { step: 2, message: 'Consolidating memories' },
            { step: 3, message: 'Done' }
        ];
        return {
            async *[Symbol.asyncIterator]() {
                for (const ev of events) {
                    yield codec.encode(ev);
                }
            }
        };
    });

    const streamResults = [];
    for await (const chunk of client.stream({ query: 'test' })) {
        console.log('  Stream chunk:', chunk);
        streamResults.push(chunk);
    }

    if (streamResults.length !== 3) {
        throw new Error(`Test failed: expected 3 stream chunks, got ${streamResults.length}`);
    }

    console.log('All SDK validation checks: PASSED!');
}

main().catch(err => {
    console.error('Test execution failed:', err);
    process.exit(1);
});
