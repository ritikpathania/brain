import React from 'react';
import { REPL } from '../REPL';
import { render as inkRender, cleanup } from 'ink-testing-library';
import { expect, test, mock, afterEach } from 'bun:test';
import { EventEmitter } from 'events';
import { FocusManager } from '../../services/FocusManager';

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
}

test('REPL renders header banner, standby status, and prompt', () => {
  const client = new MockSocketClient() as any;
  const { lastFrame } = render(<REPL client={client} />);

  const frame = lastFrame();
  expect(frame).toContain('Memory Companion CLI');
  expect(frame).toContain('Daemon Status:');
  expect(frame).toContain('✗ Unreachable');
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
  expect(frame).toContain('> ingest sqlite setup');
  expect(frame).toContain('● Processing');
  
  expect(client.sentCommands.length).toBe(1);
  expect(client.sentCommands[0].action).toBe('ingest');
  expect(client.sentCommands[0].payload).toBe('sqlite setup');

  // Trigger response
  client.triggerMessage('ok', 'Ingested node successfully');
  await sleep(350);

  const finalFrame = lastFrame();
  expect(finalFrame).toContain('[Daemon Response] Status: OK');
  expect(finalFrame).toContain('Ingested node successfully');
  expect(finalFrame).toContain('✗ Unreachable');
});
