/** /memory overlay body: searchable knowledge-graph browser. Pure view. */
import * as React from 'react';
import { Box, Text } from '../../compat/index.js';
import type { BrainTokens } from '../../state/palettes.js';
import type { RetrievedMemory } from '../../client/BrainBackendClient.js';
import { ModalFrame } from './ModalFrame.js';
import { scorePercent } from './memoryOverlayLogic.js';

export function MemoryOverlayView(props: {
  query: string;
  state: 'loading' | 'offline' | 'ready';
  rows: readonly RetrievedMemory[];
  selectedIndex: number;
  expandedId: string | null;
  tokens: BrainTokens;
}): React.ReactElement {
  void props.tokens; // reserved: selection colors arrive with semantic tokens
  return (
    <ModalFrame
      title="Relational Knowledge & Memory"
      subtitle="Inspect concepts, confidence scores, excerpts, and graph relations"
      footerHints="↑↓ navigate · enter expand · type to filter · esc close"
      width={80}
    >
      <Text>› {props.query}▏</Text>
      {props.state === 'offline' ? (
        <Box flexDirection="column">
          <Text color="red">Brain daemon is offline or unreachable.</Text>
          <Text dimColor>Start it with `brain daemon start` or `make dev`</Text>
        </Box>
      ) : props.state === 'loading' ? (
        <Text color="yellow">Searching knowledge graph…</Text>
      ) : props.rows.length === 0 ? (
        <Text dimColor>No concepts recorded in the Brain knowledge graph yet.</Text>
      ) : (
        <Box flexDirection="column">
          {props.rows.slice(0, 6).map((m, idx) => {
            const isSelected = idx === props.selectedIndex;
            const isExpanded = isSelected && props.expandedId === m.node_id;
            return (
              <Box key={m.node_id} flexDirection="column">
                <Box flexDirection="row" justifyContent="space-between">
                  <Text color={isSelected ? 'cyan' : undefined} bold={isSelected}>
                    {isSelected ? '❯ ' : '  '}{m.label}
                  </Text>
                  <Text dimColor><Text color="cyan">{scorePercent(m.score)}%</Text> · [{m.channel}]</Text>
                </Box>
                {isExpanded ? (
                  <Box marginLeft={2} flexDirection="column">
                    {m.excerpt ? <Text dimColor>{m.excerpt}</Text> : null}
                    {m.relations && m.relations.length > 0 ? (
                      <Box flexDirection="column">
                        <Text bold color="cyan">Connected Relations:</Text>
                        {m.relations.map((r, rIdx) => (
                          <Text key={rIdx} dimColor>
                            {'  ⎿ '}{r.relation} → {r.target_label ?? r.target_id}
                          </Text>
                        ))}
                      </Box>
                    ) : (
                      <Text dimColor>(No outgoing relations)</Text>
                    )}
                  </Box>
                ) : null}
              </Box>
            );
          })}
        </Box>
      )}
    </ModalFrame>
  );
}
