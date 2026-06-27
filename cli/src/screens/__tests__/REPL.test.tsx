import { mock } from 'bun:test';
mock.module('ink', () => {
  const original = require('ink');
  return {
    ...original,
    useStdout: () => ({
      stdout: {
        columns: 120,
        rows: 60,
        on: () => {},
        off: () => {},
      },
    }),
  };
});

import React from 'react';
import { REPL } from '../REPL';
import { render as inkRender, cleanup } from 'ink-testing-library';
import { expect, test, afterEach } from 'bun:test';
import { FocusManager } from '../../services/FocusManager';
import { EventEmitter } from 'events';

process.env.TERMINAL_COLUMNS = '120';
process.env.TERMINAL_ROWS = '60';

if (!(EventEmitter.prototype as any).ref) {
  (EventEmitter.prototype as any).ref = function() { return this; };
}
if (!(EventEmitter.prototype as any).unref) {
  (EventEmitter.prototype as any).unref = function() { return this; };
}

afterEach(() => {
  cleanup();
  FocusManager.reset();
});

// Wrapper render to patch mock stdin for Bun test runner compatibility
const render = (tree: React.ReactNode) => {
  const result = inkRender(tree);
  if (result.stdin) {
    const buffer: string[] = [];
    (result.stdin as any).read = () => {
      return buffer.shift() ?? null;
    };
    
    result.stdin.write = (data: any) => {
      if (typeof data === 'string') {
        buffer.push(data);
        result.stdin.emit('readable');
      }
      return true;
    };
  }
  return result;
};

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

class MockSocketClient {
  public connected = false;
  private logCallbacks: ((msg: string) => void)[] = [];
  private msgCallbacks: ((msg: any) => void)[] = [];
  public sentCommands: { action: string; payload: string }[] = [];

  connect() {
    this.connected = true;
    this.triggerLog('Connected to mock server');
  }

  onLog(cb: (msg: string) => void) {
    this.logCallbacks.push(cb);
    return () => {
      this.logCallbacks = this.logCallbacks.filter((c) => c !== cb);
    };
  }

  onMessage(cb: (msg: any) => void) {
    this.msgCallbacks.push(cb);
    return () => {
      this.msgCallbacks = this.msgCallbacks.filter((c) => c !== cb);
    };
  }

  send(action: string, payload: string) {
    this.sentCommands.push({ action, payload });
  }

  triggerLog(msg: string) {
    for (const cb of this.logCallbacks) {
      cb(msg);
    }
  }

  triggerMessage(status: string, message: string) {
    for (const cb of this.msgCallbacks) {
      cb({ status, message });
    }
  }

  triggerRawMessage(msg: any) {
    for (const cb of this.msgCallbacks) {
      cb(msg);
    }
  }
}

test('REPL renders header banner, standby status, and prompt', () => {
  const client = new MockSocketClient() as any;
  const { lastFrame } = render(<REPL client={client} />);

  const frame = lastFrame();
  expect(frame).toContain('Relational Memory Engine');
  expect(frame).toContain('Conversation View');
  expect(frame).toContain('Execution Timeline');
  expect(frame).toContain('Active Tools');
  expect(frame).toContain('exit\' to quit');
  expect(frame).toContain('Memory Engine>');
  
  expect(frame).toMatchSnapshot();
});

test('REPL displays logs when client receives connection logs', async () => {
  const client = new MockSocketClient();
  const { lastFrame } = render(<REPL client={client as any} />);
  await sleep(50);

  client.triggerLog('Connected to mock server');
  await sleep(100);

  const frame = lastFrame();
  expect(frame).toContain('[CLI Log] Connected to mock server');
});

test('REPL handles submitting commands and shows spinner status', async () => {
  const client = new MockSocketClient();
  const { stdin, lastFrame } = render(<REPL client={client as any} />);
  await sleep(100);

  // Type a command and submit it
  stdin.write('ingest sqlite setup');
  await sleep(100);
  stdin.write('\r');
  await sleep(150);

  const frame = lastFrame();
  expect(frame).toContain('ingest sqlite setup');
  expect(frame).toContain('Plan'); // Running execution status shows 'Plan' in footer

  expect(client.sentCommands.length).toBe(1);
  expect(client.sentCommands[0].action).toBe('ingest');
  expect(client.sentCommands[0].payload).toBe('sqlite setup');

  // Trigger response
  client.triggerMessage('ok', 'Ingested node successfully');
  await sleep(350);

  const finalFrame = lastFrame();
  expect(finalFrame).toContain('Ingested node successfully');
});

test('REPL handles empty stream correctly', async () => {
  const client = new MockSocketClient();
  const { lastFrame } = render(<REPL client={client as any} />);
  await sleep(50);

  // Trigger stream start and end
  client.triggerRawMessage({ type: 'stream_start', streamId: 'stream-1' });
  client.triggerRawMessage({ type: 'stream_end', streamId: 'stream-1', sequence: 1 });
  await sleep(100);

  const frame = lastFrame();
  expect(frame).not.toContain('[Protocol Warning]');
});

test('REPL handles progress updates and displays progress in status line', async () => {
  const client = new MockSocketClient();
  const { lastFrame } = render(<REPL client={client as any} />);
  await sleep(50);

  // Trigger start, progress with stage metadata, and end
  client.triggerRawMessage({ type: 'stream_start', streamId: 'stream-2' });
  client.triggerRawMessage({
    type: 'stream_progress',
    streamId: 'stream-2',
    sequence: 1,
    progress: 0.42,
    message: 'Testing progress message',
    metadata: { stage: 'Planning', status: 'started' },
  });
  await sleep(50);

  // Check progress stage displays in the timeline area
  let frame = lastFrame();
  expect(frame).toContain('Planning');

  client.triggerRawMessage({ type: 'stream_end', streamId: 'stream-2', sequence: 2 });
  await sleep(100);
});

test('REPL streams a single chunk of text and drains typewriter queue', async () => {
  const client = new MockSocketClient();
  const { lastFrame } = render(<REPL client={client as any} />);
  await sleep(50);

  client.triggerRawMessage({ type: 'stream_start', streamId: 'stream-3' });
  client.triggerRawMessage({
    type: 'stream_chunk',
    streamId: 'stream-3',
    sequence: 1,
    content: 'HelloStreamWorld',
  });
  client.triggerRawMessage({ type: 'stream_end', streamId: 'stream-3', sequence: 2 });
  
  // Wait for typewriter to drain
  await sleep(300);

  const frame = lastFrame();
  expect(frame).toContain('HelloStreamWorld');
});

test('REPL streams multiple chunks and displays them in order', async () => {
  const client = new MockSocketClient();
  const { lastFrame } = render(<REPL client={client as any} />);
  await sleep(50);

  client.triggerRawMessage({ type: 'stream_start', streamId: 'stream-4' });
  client.triggerRawMessage({
    type: 'stream_chunk',
    streamId: 'stream-4',
    sequence: 1,
    content: 'FirstChunk ',
  });
  client.triggerRawMessage({
    type: 'stream_chunk',
    streamId: 'stream-4',
    sequence: 2,
    content: 'SecondChunk',
  });
  client.triggerRawMessage({ type: 'stream_end', streamId: 'stream-4', sequence: 3 });

  await sleep(400);

  const frame = lastFrame();
  expect(frame).toContain('FirstChunk SecondChunk');
});

test('REPL logs a warning on sequence mismatch without crashing', async () => {
  const client = new MockSocketClient();
  const { lastFrame } = render(<REPL client={client as any} />);
  await sleep(50);

  client.triggerRawMessage({ type: 'stream_start', streamId: 'stream-5' });
  // Send sequence 2 instead of expected 1
  client.triggerRawMessage({
    type: 'stream_chunk',
    streamId: 'stream-5',
    sequence: 2,
    content: 'MismatchText',
  });
  client.triggerRawMessage({ type: 'stream_end', streamId: 'stream-5', sequence: 3 });

  await sleep(300);

  const frame = lastFrame();
  expect(frame).toContain('sequence mismatch');
  expect(frame).toContain('expected 1');
  expect(frame).toContain('got 2');
  expect(frame).toContain('MismatchText');
});

test('REPL is forward compatible with unknown event types', async () => {
  const client = new MockSocketClient();
  const { lastFrame } = render(<REPL client={client as any} />);
  await sleep(50);

  client.triggerRawMessage({ type: 'stream_start', streamId: 'stream-6' });
  // Send an unrecognized event type with streamId
  client.triggerRawMessage({
    type: 'stream_metric',
    streamId: 'stream-6',
    sequence: 1,
    value: 123.45,
  });
  client.triggerRawMessage({
    type: 'stream_chunk',
    streamId: 'stream-6',
    sequence: 2,
    content: 'AfterMetricText',
  });
  client.triggerRawMessage({ type: 'stream_end', streamId: 'stream-6', sequence: 3 });

  await sleep(300);

  const frame = lastFrame();
  expect(frame).toContain('Ignored unknown stream event');
  expect(frame).toContain('stream_metric');
  expect(frame).toContain('AfterMetricText');
});

test('REPL handles chunk followed by non-streaming Error response', async () => {
  const client = new MockSocketClient();
  const { lastFrame } = render(<REPL client={client as any} />);
  await sleep(50);

  client.triggerRawMessage({ type: 'stream_start', streamId: 'stream-7' });
  client.triggerRawMessage({
    type: 'stream_chunk',
    streamId: 'stream-7',
    sequence: 1,
    content: 'PartialResult',
  });
  // Wait for typewriter to render the chunk
  await sleep(150);
  // Send a versioned Error response mid-stream
  client.triggerRawMessage({
    version: '1.0',
    type: 'Error',
    id: 123,
    status: 'error',
    body: 'Fatal daemon error occurred',
  });

  await sleep(350);

  const frame = lastFrame();
  expect(frame).toContain('PartialResult');
  expect(frame).toContain('Fatal daemon error occurred');
});

test('REPL handles Tab key to switch between focus states of panels', async () => {
  const client = new MockSocketClient();
  const { stdin, lastFrame } = render(<REPL client={client as any} />);
  await sleep(100);

  // Initial state: ChatView is focused
  let frame = lastFrame();
  expect(frame).toContain('Conversation View (Focused)');

  // Press Tab -> focus sessions browser
  stdin.write('\t');
  await sleep(150);
  frame = lastFrame();
  expect(frame).toContain('Sessions (Focused)');

  // Press Tab -> focus timeline
  stdin.write('\t');
  await sleep(150);
  frame = lastFrame();
  expect(frame).toContain('Execution Timeline (Focused)');

  // Press Tab -> focus active tools
  stdin.write('\t');
  await sleep(150);
  frame = lastFrame();
  expect(frame).toContain('Active Tools (Focused)');

  // Press Tab -> cycle back to chat
  stdin.write('\t');
  await sleep(150);
  frame = lastFrame();
  expect(frame).toContain('Conversation View (Focused)');
});

test('REPL commits partial text to logs when interrupted by a new stream_start event', async () => {
  const client = new MockSocketClient();
  const { lastFrame } = render(<REPL client={client as any} />);
  await sleep(50);

  // Start first stream
  client.triggerRawMessage({ type: 'stream_start', streamId: 'stream-8' });
  client.triggerRawMessage({
    type: 'stream_chunk',
    streamId: 'stream-8',
    sequence: 1,
    content: 'InterruptedPartialContent',
  });
  // Wait for typewriter to render part of it
  await sleep(150);

  // Start second stream before first finishes, triggering interruption
  client.triggerRawMessage({ type: 'stream_start', streamId: 'stream-9' });
  client.triggerRawMessage({
    type: 'stream_chunk',
    streamId: 'stream-9',
    sequence: 1,
    content: 'NewContentAfterInterruption',
  });
  await sleep(350);

  const frame = lastFrame();
  expect(frame).toContain('InterruptedPartialContent');
  expect(frame).toContain('NewContentAfterInterruption');
  expect(frame).toContain('was not terminated');
});

test('REPL logs a warning on sequence regression', async () => {
  const client = new MockSocketClient();
  const { lastFrame } = render(<REPL client={client as any} />);
  await sleep(50);

  client.triggerRawMessage({ type: 'stream_start', streamId: 'stream-10' });
  client.triggerRawMessage({
    type: 'stream_chunk',
    streamId: 'stream-10',
    sequence: 1,
    content: 'FirstChunk',
  });
  client.triggerRawMessage({
    type: 'stream_chunk',
    streamId: 'stream-10',
    sequence: 2,
    content: 'SecondChunk',
  });
  // Regress sequence number
  client.triggerRawMessage({
    type: 'stream_chunk',
    streamId: 'stream-10',
    sequence: 1,
    content: 'RegressedChunk',
  });
  client.triggerRawMessage({ type: 'stream_end', streamId: 'stream-10', sequence: 3 });

  await sleep(350);
  const frame = lastFrame();
  expect(frame).toContain('sequence regressed');
});

test('REPL logs a warning on already terminated stream resurrection', async () => {
  const client = new MockSocketClient();
  const { lastFrame } = render(<REPL client={client as any} />);
  await sleep(50);

  client.triggerRawMessage({ type: 'stream_start', streamId: 'stream-11' });
  client.triggerRawMessage({ type: 'stream_end', streamId: 'stream-11', sequence: 1 });
  // Resurrect already terminated stream
  client.triggerRawMessage({
    type: 'stream_chunk',
    streamId: 'stream-11',
    sequence: 2,
    content: 'ResurrectedChunk',
  });

  await sleep(350);
  const frame = lastFrame();
  expect(frame).toContain('already terminated stream');
});

test('REPL logs a warning when a stream starts before the previous one terminates', async () => {
  const client = new MockSocketClient();
  const { lastFrame } = render(<REPL client={client as any} />);
  await sleep(50);

  client.triggerRawMessage({ type: 'stream_start', streamId: 'stream-12' });
  // Start another stream without terminating the first
  client.triggerRawMessage({ type: 'stream_start', streamId: 'stream-13' });

  await sleep(350);
  const frame = lastFrame();
  expect(frame).toContain('was not terminated');
  expect(frame).toContain('stream-12');
  expect(frame).toContain('stream-13');
});
