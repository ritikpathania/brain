import React, { useState, useEffect } from 'react';
import { ThemedBox } from './ThemedBox';
import { ThemedText } from './ThemedText';
import { ColorToken } from '../tokens';

interface SpinnerProps {
  color?: ColorToken;
  shimmerColor?: ColorToken;
  label?: string;
}

const defaultVerbs = [
  'Thinking…',
  'Reading file…',
  'Searching knowledge graph…',
  'Processing tokens…',
  'Generating context…',
];

export const Spinner: React.FC<SpinnerProps> = ({
  color = 'claude',
  shimmerColor = 'claudeShimmer',
  label,
}) => {
  const [frame, setFrame] = useState(0);
  const [verbIndex, setVerbIndex] = useState(0);

  // Frame tick: 80ms interval
  useEffect(() => {
    if (process.env.NODE_ENV === 'test') return;
    const timer = setInterval(() => {
      setFrame((prev) => (prev + 1) % 10);
    }, 80);
    return () => clearInterval(timer);
  }, []);

  // Verb rotation: every 1.6s (20 frames)
  useEffect(() => {
    if (process.env.NODE_ENV === 'test') return;
    const verbTimer = setInterval(() => {
      setVerbIndex((prev) => (prev + 1) % defaultVerbs.length);
    }, 1600);
    return () => clearInterval(verbTimer);
  }, []);

  const spinnerFrames = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
  const char = spinnerFrames[frame % spinnerFrames.length];
  const activeColor = frame % 2 === 0 ? color : shimmerColor;
  const displayLabel = label || defaultVerbs[verbIndex];

  return (
    <ThemedBox flexDirection="row" alignItems="center">
      <ThemedText color={activeColor} bold>
        {char}
      </ThemedText>
      <ThemedText color="inactive" marginLeft={1}>
        {displayLabel}
      </ThemedText>
    </ThemedBox>
  );
};
