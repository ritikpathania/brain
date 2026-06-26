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

  const validateSequence = (sequence: number) => {
    const expected = expectedSequenceRef.current;
    if (sequence !== expected) {
      const warningMsg = `[Protocol Warning] Stream sequence mismatch: expected ${expected}, got ${sequence}`;
      if (onWarning) {
        onWarning(warningMsg);
      } else {
        console.warn(warningMsg);
      }
    }
    expectedSequenceRef.current = sequence + 1;
  };

  const startStream = (streamId: string) => {
    setDisplayedText('');
    setIsStreaming(true);
    setProgress(null);
    chunkQueueRef.current = [];
    tokenQueueRef.current = [];
    networkFinishedRef.current = false;
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

  const queueChunk = (content: string, sequence: number) => {
    validateSequence(sequence);
    chunkQueueRef.current.push(content);
  };

  const handleProgress = (progVal: number, msgVal: string, sequence: number) => {
    validateSequence(sequence);
    setProgress({ progress: progVal, message: msgVal });
  };

  const endStream = (sequence: number) => {
    validateSequence(sequence);
    networkFinishedRef.current = true;
  };

  const cancelStream = (sequence: number) => {
    validateSequence(sequence);
    chunkQueueRef.current = [];
    tokenQueueRef.current = [];
    networkFinishedRef.current = true;
    setIsStreaming(false);
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
  };
};

