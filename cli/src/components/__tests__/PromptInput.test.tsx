import React from 'react';
import { PromptInput } from '../PromptInput';
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

// Wrapper render to patch mock stdin for Bun & Ink 4 compatibility
const render = (tree: React.ReactNode) => {
  const result = inkRender(tree);
  if (result.stdin) {
    if (!result.stdin.ref) {
      result.stdin.ref = () => result.stdin;
    }
    if (!result.stdin.unref) {
      result.stdin.unref = () => result.stdin;
    }
    
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

test('PromptInput renders default placeholder and prefix', () => {
  const handleSubmit = mock(() => {});
  const { lastFrame } = render(<PromptInput onSubmit={handleSubmit} />);
  
  const frame = lastFrame();
  expect(frame).toContain('Memory Engine>');
  expect(frame).toContain('Type a memory command');
  expect(frame).toContain('█');
  
  expect(frame).toMatchSnapshot();
});

test('PromptInput renders custom placeholder and prefix', () => {
  const handleSubmit = mock(() => {});
  const { lastFrame } = render(
    <PromptInput 
      onSubmit={handleSubmit} 
      placeholder="custom placeholder" 
      prefix="Prefix> " 
    />
  );
  
  const frame = lastFrame();
  expect(frame).toContain('Prefix>');
  expect(frame).toContain('custom placeholder');
});

test('PromptInput handles typing and submission', async () => {
  let submittedValue = '';
  const handleSubmit = (val: string) => {
    submittedValue = val;
  };
  const { stdin, lastFrame } = render(<PromptInput onSubmit={handleSubmit} />);

  await sleep(100);
  // Simulate typing
  stdin.write('query database');
  await sleep(100);
  
  let frame = lastFrame();
  expect(frame).toContain('query database');
  expect(frame).not.toContain('Type a memory command');

  // Submit with Enter key
  stdin.write('\r');
  await sleep(100);

  expect(submittedValue).toBe('query database');
  expect(lastFrame()).toContain('Type a memory command');
});
