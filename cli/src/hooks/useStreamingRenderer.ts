import { useState, useEffect, useRef } from 'react';

export interface ProgressState {
  progress: number;
  message: string;
}

export const useStreamingRenderer = (
  speedMs: number = 20,
  onWarning?: (message: string) => void
) => {
  const [displayedText, setDisplayedText] = useState('');
  const [isStreaming, setIsStreaming] = useState(false);
  const [progress, setProgress] = useState<ProgressState | null>(null);

  const chunkQueueRef = useRef<string[]>([]);
  const tokenQueueRef = useRef<string[]>([]);
  const networkFinishedRef = useRef(false);
  const expectedSequenceRef = useRef(1);
  const timerRef = useRef<any>(null);

  const activeStreamIdRef = useRef<string | null>(null);
  const lastSequenceRef = useRef<number>(0);
  const terminatedStreamsRef = useRef<Set<string>>(new Set());

  const validateSequence = (sequence: number, streamId?: string) => {
    const sId = streamId || activeStreamIdRef.current || 'unknown';

    // 1. Reconnect resurrection prevention / terminated check
    if (terminatedStreamsRef.current.has(sId)) {
      const warningMsg = `[Protocol Warning] Received packet for already terminated stream "${sId}"`;
      if (onWarning) {
        onWarning(warningMsg);
      } else {
        console.warn(warningMsg);
      }
      return;
    }

    // 2. Stream mismatch check
    if (activeStreamIdRef.current && activeStreamIdRef.current !== sId) {
      const warningMsg = `[Protocol Warning] Received packet for stream "${sId}" but active stream is "${activeStreamIdRef.current}"`;
      if (onWarning) {
        onWarning(warningMsg);
      } else {
        console.warn(warningMsg);
      }
      return;
    }

    // 3. Monotonic sequence regression check
    if (sequence <= lastSequenceRef.current) {
      const warningMsg = `[Protocol Warning] Stream sequence regressed: expected greater than ${lastSequenceRef.current}, got ${sequence}`;
      if (onWarning) {
        onWarning(warningMsg);
      } else {
        console.warn(warningMsg);
      }
    }

    // 4. Expected sequence mismatch check
    const expected = expectedSequenceRef.current;
    if (sequence !== expected) {
      const warningMsg = `[Protocol Warning] Stream sequence mismatch: expected ${expected}, got ${sequence}`;
      if (onWarning) {
        onWarning(warningMsg);
      } else {
        console.warn(warningMsg);
      }
    }

    lastSequenceRef.current = sequence;
    expectedSequenceRef.current = sequence + 1;
  };

  const startStream = (streamId: string) => {
    // Check if there is an active stream that was not terminated
    if (activeStreamIdRef.current && activeStreamIdRef.current !== streamId) {
      const warningMsg = `[Protocol Warning] Stream "${activeStreamIdRef.current}" was not terminated before starting stream "${streamId}"`;
      if (onWarning) {
        onWarning(warningMsg);
      } else {
        console.warn(warningMsg);
      }
    }

    setDisplayedText('');
    setIsStreaming(true);
    setProgress(null);
    chunkQueueRef.current = [];
    tokenQueueRef.current = [];
    networkFinishedRef.current = false;
    
    activeStreamIdRef.current = streamId;
    lastSequenceRef.current = 0;
    expectedSequenceRef.current = 1;

    if (timerRef.current) {
      clearInterval(timerRef.current);
      timerRef.current = null;
    }

    timerRef.current = setInterval(runTick, speedMs);
  };

  const runTick = () => {
    if (tokenQueueRef.current.length > 0) {
      const nextToken = tokenQueueRef.current.shift()!;
      setDisplayedText((prev) => prev + nextToken);
    } else if (chunkQueueRef.current.length > 0) {
      const nextChunk = chunkQueueRef.current.shift()!;
      const tokens = nextChunk.split(/(\s+)/).filter(Boolean);
      tokenQueueRef.current.push(...tokens);
      if (tokenQueueRef.current.length > 0) {
        const nextToken = tokenQueueRef.current.shift()!;
        setDisplayedText((prev) => prev + nextToken);
      }
    } else if (networkFinishedRef.current) {
      setIsStreaming(false);
      if (timerRef.current) {
        clearInterval(timerRef.current);
        timerRef.current = null;
      }
    }
  };

  const queueChunk = (content: string, sequence: number, streamId?: string) => {
    validateSequence(sequence, streamId);
    chunkQueueRef.current.push(content);
  };

  const handleProgress = (progVal: number, msgVal: string, sequence: number, streamId?: string) => {
    validateSequence(sequence, streamId);
    setProgress({ progress: progVal, message: msgVal });
  };

  const endStream = (sequence: number, streamId?: string) => {
    const sId = streamId || activeStreamIdRef.current || 'unknown';
    if (terminatedStreamsRef.current.has(sId)) {
      // endStream is idempotent
      return;
    }
    validateSequence(sequence, sId);
    terminatedStreamsRef.current.add(sId);
    if (activeStreamIdRef.current === sId) {
      activeStreamIdRef.current = null;
    }
    networkFinishedRef.current = true;
  };

  const cancelStream = (sequence: number, streamId?: string) => {
    const sId = streamId || activeStreamIdRef.current || 'unknown';
    if (terminatedStreamsRef.current.has(sId)) {
      // cancel is idempotent
      return;
    }
    if (sequence !== 0) {
      validateSequence(sequence, sId);
    }
    terminatedStreamsRef.current.add(sId);
    if (activeStreamIdRef.current === sId) {
      activeStreamIdRef.current = null;
    }
    chunkQueueRef.current = [];
    tokenQueueRef.current = [];
    networkFinishedRef.current = true;
    setIsStreaming(false);
    if (timerRef.current) {
      clearInterval(timerRef.current);
      timerRef.current = null;
    }
  };

  const resetStreamState = () => {
    activeStreamIdRef.current = null;
    lastSequenceRef.current = 0;
    expectedSequenceRef.current = 1;
    terminatedStreamsRef.current.clear();
    setIsStreaming(false);
    setDisplayedText('');
    chunkQueueRef.current = [];
    tokenQueueRef.current = [];
    networkFinishedRef.current = false;
    if (timerRef.current) {
      clearInterval(timerRef.current);
      timerRef.current = null;
    }
  };

  useEffect(() => {
    return () => {
      if (timerRef.current) {
        clearInterval(timerRef.current);
      }
    };
  }, []);

  return {
    displayedText,
    isStreaming,
    progress,
    startStream,
    queueChunk,
    handleProgress,
    endStream,
    cancelStream,
    resetStreamState,
  };
};

