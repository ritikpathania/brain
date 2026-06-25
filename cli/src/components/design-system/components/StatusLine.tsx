import React from 'react';
import { ThemedBox } from './ThemedBox';
import { ThemedText } from './ThemedText';
import { ColorToken } from '../tokens';

interface StatusLineProps {
  mode?: 'Plan' | 'Auto' | 'Fast';
  modelName?: string;
  tokens?: { input: number; output: number };
  cost?: number;
  rateLimitPercent?: number; // 0 - 100
}

export const StatusLine: React.FC<StatusLineProps> = ({
  mode = 'Auto',
  modelName = 'Gemini 3.5 Flash',
  tokens = { input: 1240, output: 382 },
  cost = 0.0024,
  rateLimitPercent = 15,
}) => {
  // Determine mode color
  let modeColor: ColorToken = 'autoAccept';
  if (mode === 'Plan') modeColor = 'planMode';
  if (mode === 'Fast') modeColor = 'fastMode';

  // Cost color warning threshold
  const costColor: ColorToken = cost > 1.0 ? 'warning' : 'inactive';

  // Rate limit bar drawing: mini 5-character progress bar
  const drawMiniProgressBar = (pct: number) => {
    const filledChars = Math.round((pct / 100) * 5);
    const emptyChars = 5 - filledChars;
    return (
      <ThemedBox flexDirection="row">
        <ThemedText color="rate_limit_fill">
          {'█'.repeat(filledChars)}
        </ThemedText>
        <ThemedText color="rate_limit_empty">
          {'░'.repeat(emptyChars)}
        </ThemedText>
      </ThemedBox>
    );
  };

  return (
    <ThemedBox
      flexDirection="row"
      justifyContent="space-between"
      paddingX={1}
      width="100%"
      height={1}
    >
      {/* Left segments: Mode & Model */}
      <ThemedBox flexDirection="row" gap={2}>
        <ThemedBox flexDirection="row">
          <ThemedText color="text">[</ThemedText>
          <ThemedText color={modeColor} bold>
            {mode}
          </ThemedText>
          <ThemedText color="text">]</ThemedText>
        </ThemedBox>

        <ThemedBox flexDirection="row" flexShrink={1}>
          <ThemedText color="claude" bold>
            {modelName}
          </ThemedText>
        </ThemedBox>
      </ThemedBox>

      {/* Center segments: Token usage & Cost */}
      <ThemedBox flexDirection="row" gap={2} flexShrink={1}>
        <ThemedText color="inactive">
          In: {tokens.input} · Out: {tokens.output}
        </ThemedText>
        <ThemedText color={costColor}>
          ${cost.toFixed(4)}
        </ThemedText>
      </ThemedBox>

      {/* Right segments: Rate Limit & Key Hints */}
      <ThemedBox flexDirection="row" gap={2}>
        <ThemedBox flexDirection="row" gap={1}>
          <ThemedText color="inactive">Rate:</ThemedText>
          {drawMiniProgressBar(rateLimitPercent)}
        </ThemedBox>
        <ThemedText color="subtle" italic>
          'exit' to quit
        </ThemedText>
      </ThemedBox>
    </ThemedBox>
  );
};
