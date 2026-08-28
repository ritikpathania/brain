/**
 * /model overlay body: interactive model selector (Inc 38).
 */
import * as React from 'react';
import { Box, Text } from '../../compat/index.js';
import type { BrainTokens } from '../../state/palettes.js';
import type { ModelItemVM } from './modelPickerLogic.js';
import { ModalFrame } from './ModalFrame.js';

export function ModelPickerView(props: {
  loading: boolean;
  error?: string;
  models: readonly ModelItemVM[];
  selectedIndex: number;
  tokens: BrainTokens;
}): React.ReactElement {
  void props.tokens;
  const sel = Math.min(props.selectedIndex, Math.max(0, props.models.length - 1));

  return (
    <ModalFrame
      title="Select Active Model"
      subtitle="Choose default generation model from registered providers"
      footerHints="↑↓ navigate · enter switch model · esc close"
      width={80}
    >
      {props.loading ? (
        <Text color="yellow">Discovering available models from gateway…</Text>
      ) : props.error ? (
        <Text color="red">{props.error}</Text>
      ) : props.models.length === 0 ? (
        <Text dimColor>No registered models found.</Text>
      ) : (
        <Box flexDirection="column">
          {props.models.map((m, idx) => {
            const isSelected = idx === sel;
            return (
              <Box key={m.id} flexDirection="row" justifyContent="space-between">
                <Text color={isSelected ? 'cyan' : undefined} bold={isSelected}>
                  {isSelected ? '❯ ' : '  '}
                  {m.isActive ? <Text color="green">● </Text> : null}
                  {m.name}
                  <Text dimColor>{` (${m.provider})`}</Text>
                  {m.isDefault ? <Text color="yellow"> [default]</Text> : null}
                  {m.supportsThinking ? <Text color="magenta"> [thinking]</Text> : null}
                  {m.supportsTools ? <Text color="blue"> [tools]</Text> : null}
                </Text>
                <Text dimColor>{m.contextLabel}</Text>
              </Box>
            );
          })}
        </Box>
      )}
    </ModalFrame>
  );
}
