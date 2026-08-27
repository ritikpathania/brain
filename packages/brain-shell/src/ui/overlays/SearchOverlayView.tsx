/**
 * /search overlay body: interactive cross-session transcript explorer (Inc 34).
 */
import * as React from 'react';
import { Box, Text } from '../../compat/index.js';
import type { BrainTokens } from '../../state/palettes.js';
import type { SearchItemVM } from './searchOverlayLogic.js';
import { SEARCH_MAX_DISPLAY_ITEMS } from './searchOverlayLogic.js';
import { ModalFrame } from './ModalFrame.js';

export function SearchOverlayView(props: {
  query: string;
  state: 'idle' | 'loading' | 'ready';
  matches: readonly SearchItemVM[];
  selectedIndex: number;
  tokens: BrainTokens;
  currentSessionId?: string;
}): React.ReactElement {
  void props.tokens;
  const sel = Math.min(props.selectedIndex, Math.max(0, props.matches.length - 1));

  return (
    <ModalFrame
      title="Search Transcripts"
      subtitle="Search conversation history across all recorded sessions"
      footerHints="↑↓ navigate · enter resume session · esc close · type to search"
      width={80}
    >
      <Text>› {props.query}▏</Text>
      {props.state === 'loading' ? (
        <Text color="yellow">Searching transcripts across sessions…</Text>
      ) : props.state === 'idle' && props.query.trim().length === 0 ? (
        <Text dimColor>Type a query to search across historical conversation logs.</Text>
      ) : props.matches.length === 0 ? (
        <Text dimColor>No transcript matches found for “{props.query}”.</Text>
      ) : (
        <Box flexDirection="column">
          {props.matches.slice(0, SEARCH_MAX_DISPLAY_ITEMS).map((m, idx) => {
            const isSelected = idx === sel;
            const isCurrent = Boolean(props.currentSessionId && m.sessionId === props.currentSessionId);
            const prefix = m.sessionId ? `[${m.sessionId.slice(0, 8)}] ` : '';
            const roleTag = m.role ? `(${m.role}) ` : '';
            const snippetText = m.snippet.length > 70 ? `${m.snippet.slice(0, 67)}...` : m.snippet;

            return (
              <Box key={m.id || `${m.sessionId}-${idx}`} flexDirection="column">
                <Box flexDirection="row" justifyContent="space-between">
                  <Text color={isSelected ? 'cyan' : undefined} bold={isSelected}>
                    {isSelected ? '❯ ' : '  '}
                    {isCurrent ? <Text dimColor>● </Text> : null}
                    <Text dimColor>{prefix}</Text>
                    <Text color={isSelected ? 'cyan' : 'yellow'}>{roleTag}</Text>
                    {snippetText}
                  </Text>
                  {m.score !== undefined ? (
                    <Text dimColor><Text color="cyan">{Math.round(m.score * 100)}%</Text></Text>
                  ) : null}
                </Box>
              </Box>
            );
          })}
        </Box>
      )}
    </ModalFrame>
  );
}
