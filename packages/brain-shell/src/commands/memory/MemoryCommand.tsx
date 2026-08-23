import React, { useState, useEffect, useCallback } from 'react';
import { Box, Text, useInput } from '../../compat/ink.js';
import { useTerminalSize } from '../../compat/hooks.js';
import type { BrainMemoryService } from '../../adapter/BrainMemoryService.js';
import type { MemoryProvenanceView } from '../../adapter/BrainViewModels.js';
import { BrainModal } from '../../components/BrainModal.js';
import { BrainSearchField } from '../../components/BrainSearchField.js';

export interface MemoryCommandProps {
  memoryService: BrainMemoryService;
  sessionId?: string;
  initialQuery?: string;
  onDone: (result?: string, options?: { display?: 'system' | 'user' }) => void;
}

export const MemoryCommand: React.FC<MemoryCommandProps> = ({
  memoryService,
  sessionId,
  initialQuery = '',
  onDone,
}) => {
  const [memories, setMemories] = useState<MemoryProvenanceView[]>([]);
  const [selectedIndex, setSelectedIndex] = useState<number>(0);
  const [searchQuery, setSearchQuery] = useState<string>(initialQuery);
  const [isExpanded, setIsExpanded] = useState<boolean>(false);
  const [isLoading, setIsLoading] = useState<boolean>(true);
  const [isOffline, setIsOffline] = useState<boolean>(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const { columns } = useTerminalSize();

  const fetchMemories = useCallback(
    async (query: string) => {
      setIsLoading(true);
      setErrorMessage(null);

      try {
        const available = await memoryService.isAvailable();
        if (!available) {
          setIsOffline(true);
          setIsLoading(false);
          setMemories([]);
          return;
        }

        setIsOffline(false);
        const results = await memoryService.searchBackend(query, 20, sessionId);
        setMemories(results);
        setSelectedIndex((prev) => {
          if (results.length === 0) return 0;
          return prev >= results.length ? results.length - 1 : prev;
        });
      } catch (err: any) {
        const msg = err?.message || String(err);
        if (
          msg.includes('unavailable') ||
          msg.includes('offline') ||
          msg.includes('ECONNREFUSED') ||
          msg.includes('ENOENT')
        ) {
          setIsOffline(true);
          setMemories([]);
        } else {
          setErrorMessage(`Error: ${msg}`);
        }
      } finally {
        setIsLoading(false);
      }
    },
    [memoryService, sessionId]
  );

  useEffect(() => {
    void fetchMemories(searchQuery);
  }, [fetchMemories]);

  useInput((input, key) => {
    if (key.escape) {
      onDone('Closed memory exploration view', { display: 'system' });
      return;
    }

    if (key.upArrow) {
      setSelectedIndex((prev) => (prev > 0 ? prev - 1 : Math.max(0, memories.length - 1)));
      return;
    }

    if (key.downArrow) {
      setSelectedIndex((prev) => (prev < memories.length - 1 ? prev + 1 : 0));
      return;
    }

    if (key.return || input === ' ') {
      if (memories.length > 0) {
        setIsExpanded((prev) => !prev);
      }
    }
  });

  const selectedMemory = memories[selectedIndex] || memories[0];

  return (
    <BrainModal
      title="Relational Knowledge & Memory"
      subtitle="Inspect concepts, confidence scores, excerpts, and graph relations"
      footerHints="↑/↓ Navigate · Enter/Space Expand details · Esc to close"
      onDismiss={() => onDone('Closed memory exploration view', { display: 'system' })}
      width={Math.min(80, columns)}
    >
      <Box flexDirection="column" gap={1}>
        <BrainSearchField
          value={searchQuery}
          onChange={(q) => {
            setSearchQuery(q);
            void fetchMemories(q);
          }}
          placeholder="Search knowledge graph…"
          width={Math.min(76, columns - 4)}
        />

        {isOffline ? (
          <Box flexDirection="column" marginY={1}>
            <Text color="red">Brain daemon is offline or unreachable.</Text>
            <Text dimColor>Start it with `brain daemon start` or `make dev`</Text>
          </Box>
        ) : isLoading ? (
          <Text color="yellow">Searching knowledge graph…</Text>
        ) : errorMessage ? (
          <Text color="red">{errorMessage}</Text>
        ) : memories.length === 0 ? (
          <Box marginY={1}>
            <Text dimColor>
              {searchQuery
                ? `No concepts matching "${searchQuery}".`
                : 'No concepts recorded in the Brain knowledge graph yet.'}
            </Text>
          </Box>
        ) : (
          <Box flexDirection="column" marginTop={1}>
            {memories.slice(0, 6).map((m, idx) => {
              const isSelected = idx === selectedIndex;
              const scorePct = Math.round(m.score);

              return (
                <Box key={m.nodeId} flexDirection="column">
                  <Box flexDirection="row" justifyContent="space-between">
                    <Text color={isSelected ? 'cyan' : undefined} bold={isSelected}>
                      {isSelected ? '❯ ' : '  '}
                      {m.label}
                    </Text>
                    <Text dimColor>
                      <Text color="cyan">{scorePct}%</Text> · [{m.source}]
                    </Text>
                  </Box>

                  {isSelected && isExpanded && (
                    <Box
                      marginLeft={2}
                      marginTop={1}
                      marginBottom={1}
                      flexDirection="column"
                      paddingLeft={1}
                    >
                      {m.excerpt && (
                        <Box marginBottom={m.relations && m.relations.length > 0 ? 1 : 0}>
                          <Text dimColor>{m.excerpt}</Text>
                        </Box>
                      )}

                      {m.relations && m.relations.length > 0 ? (
                        <Box flexDirection="column">
                          <Text bold color="cyan">
                            Connected Relations:
                          </Text>
                          {m.relations.map((r, rIdx) => (
                            <Text key={rIdx} dimColor>
                              {'  ⎿ '}{r.relation} → {r.targetLabel || r.targetId}
                            </Text>
                          ))}
                        </Box>
                      ) : (
                        <Text dimColor>(No outgoing relations)</Text>
                      )}
                    </Box>
                  )}
                </Box>
              );
            })}
          </Box>
        )}
      </Box>
    </BrainModal>
  );
};

export default MemoryCommand;
