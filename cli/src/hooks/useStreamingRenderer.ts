import { useState, useEffect, useRef } from 'react';

export const useStreamingRenderer = (speedMs: number = 20) => {
  const [displayedText, setDisplayedText] = useState('');
  const [isStreaming, setIsStreaming] = useState(false);
  const queueRef = useRef<string[]>([]);
  const timerRef = useRef<any>(null);

  const startStream = (text: string) => {
    setDisplayedText('');
    setIsStreaming(true);
    if (timerRef.current) {
      clearInterval(timerRef.current);
    }

    // Tokenize text into words/spaces
    const tokens = text.split(/(\s+)/).filter(Boolean);
    queueRef.current = tokens;

    let idx = 0;
    timerRef.current = setInterval(() => {
      if (idx < queueRef.current.length) {
        setDisplayedText((prev) => prev + queueRef.current[idx]);
        idx++;
      } else {
        setIsStreaming(false);
        if (timerRef.current) {
          clearInterval(timerRef.current);
          timerRef.current = null;
        }
      }
    }, speedMs);
  };

  useEffect(() => {
    return () => {
      if (timerRef.current) {
        clearInterval(timerRef.current);
      }
    };
  }, []);

  return { displayedText, isStreaming, startStream };
};
