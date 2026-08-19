import * as React from 'react';
import { memo, type ReactNode } from 'react';
import { useTerminalSize } from '../../vendor/claude/hooks/useTerminalSize.js';
import { stringWidth } from '../../vendor/claude/ink/stringWidth.js';
import { Box, Text } from '../../vendor/claude/ink.js';
import { truncatePathMiddle, truncateToWidth } from '../../vendor/claude/utils/format.js';
import type { Theme } from '../../vendor/claude/utils/theme.js';

export type SuggestionItem = {
  id: string;
  displayText: string;
  tag?: string;
  description?: string;
  metadata?: unknown;
  color?: keyof Theme;
  kind?: string;
  sourceTag?: string;
  query?: string;
};

export const OVERLAY_MAX_ITEMS = 5;

function isUnifiedSuggestion(itemId: string): boolean {
  return (
    itemId.startsWith('file-') ||
    itemId.startsWith('mcp-resource-') ||
    itemId.startsWith('mcp-template') ||
    itemId.startsWith('agent-')
  );
}

function getIcon(itemId: string): string {
  if (itemId.startsWith('file-')) return '+';
  if (itemId.startsWith('mcp-resource-') || itemId.startsWith('mcp-template')) return '◇';
  if (itemId.startsWith('agent-')) return '*';
  return '+';
}

function getKindInfo(item: SuggestionItem) {
  const kindLabel =
    item.kind === undefined || item.kind === 'action'
      ? ''
      : item.kind === 'info'
      ? 'config'
      : item.kind;
  const kindLaneText =
    item.kind === undefined ? '' : kindLabel + ' '.repeat(Math.max(0, 7 - stringWidth(kindLabel)));
  const sourceText = item.sourceTag ? `[${item.sourceTag}] ` : '';
  return { kindLaneText, kindLabel, sourceText };
}

function calcItemLines(item: SuggestionItem, columns: number, maxColWidth: number): number {
  if (isUnifiedSuggestion(item.id) || !item.description) return 1;
  const effectiveColWidth = Math.max(maxColWidth, 30);
  const nameColWidth = Math.min(effectiveColWidth, Math.floor(columns * 0.4));
  const tagWidth = item.tag ? stringWidth(`[${item.tag}] `) : 0;
  const { kindLaneText, sourceText } = getKindInfo(item);
  const available = Math.max(
    0,
    columns - nameColWidth - tagWidth - stringWidth(kindLaneText) - stringWidth(sourceText) - 4,
  );
  if (available <= 0) return 1;
  const cleanDesc = item.description.replace(/\s+/g, ' ').trim();
  return stringWidth(cleanDesc) > available ? 2 : 1;
}

function wrapLine(text: string, width: number): [string, string] {
  if (width <= 0 || stringWidth(text) <= width) return [text, ''];
  const line1 = truncateToWidth(text, width);
  const remainder = text.slice(line1.length);
  if (remainder.startsWith(' ')) return [line1, remainder.trimStart()];
  const lastSpace = line1.lastIndexOf(' ');
  if (lastSpace > 0) return [line1.slice(0, lastSpace), text.slice(lastSpace + 1)];
  return [line1, remainder];
}

function findQueryMatches(text: string, query: string, contiguousOnly = false): [number, number][] {
  const lowerText = text.toLowerCase();
  const lowerQuery = query.toLowerCase();
  if (lowerText.length !== text.length) return [];

  const exactIdx = lowerText.indexOf(lowerQuery);
  if (exactIdx !== -1) {
    return [[exactIdx, exactIdx + lowerQuery.length]];
  }
  if (contiguousOnly) return [];

  const intervals: [number, number][] = [];
  let searchStart = 0;
  for (const char of lowerQuery) {
    const charIdx = lowerText.indexOf(char, searchStart);
    if (charIdx === -1) return [];
    const charEnd = charIdx + char.length;
    const lastInterval = intervals[intervals.length - 1];
    if (lastInterval && lastInterval[1] === charIdx) {
      lastInterval[1] = charEnd;
    } else {
      intervals.push([charIdx, charEnd]);
    }
    searchStart = charEnd;
  }
  return intervals;
}

function HighlightQuery({
  text,
  query,
  color,
  isSelected,
  bold,
  contiguousOnly = false,
}: {
  text: string;
  query?: string;
  color?: keyof Theme;
  isSelected: boolean;
  bold?: boolean;
  contiguousOnly?: boolean;
}): ReactNode {
  if (!query) {
    return (
      <Text color={color} dimColor={!isSelected} bold={bold}>
        {text}
      </Text>
    );
  }

  const effectiveQuery = query.startsWith('/') ? query.slice(1) : query;
  const matches = effectiveQuery ? findQueryMatches(text, effectiveQuery, contiguousOnly) : [];

  if (matches.length === 0) {
    return (
      <Text color={color} dimColor={!isSelected} bold={bold}>
        {text}
      </Text>
    );
  }

  const nodes: ReactNode[] = [];
  let lastEnd = 0;
  for (const [start, end] of matches) {
    if (start > lastEnd) {
      nodes.push(
        <Text key={`unmatched-${lastEnd}`} color={color} dimColor={!isSelected} bold={bold}>
          {text.slice(lastEnd, start)}
        </Text>
      );
    }
    nodes.push(
      <Text key={`matched-${start}`} color={color} dimColor={false} bold={true}>
        {text.slice(start, end)}
      </Text>
    );
    lastEnd = end;
  }
  if (lastEnd < text.length) {
    nodes.push(
      <Text key={`unmatched-${lastEnd}`} color={color} dimColor={!isSelected} bold={bold}>
        {text.slice(lastEnd)}
      </Text>
    );
  }

  return <>{nodes}</>;
}

const SuggestionItemRow = memo(function SuggestionItemRow({
  item,
  maxColumnWidth,
  isSelected,
  allowWrap = true,
}: {
  item: SuggestionItem;
  maxColumnWidth?: number;
  isSelected: boolean;
  allowWrap?: boolean;
}): ReactNode {
  const columns = useTerminalSize().columns;
  if (isUnifiedSuggestion(item.id)) {
    const icon = getIcon(item.id);
    const textColor: keyof Theme | undefined = isSelected ? 'suggestion' : undefined;
    const dimColor = !isSelected;
    const isFile = item.id.startsWith('file-');
    const isMcpResource = item.id.startsWith('mcp-resource-');
    const isMcpTemplate =
      item.id.startsWith('mcp-template-value::') || item.id.startsWith('mcp-template::');
    const separatorWidth = item.description ? 3 : 0;

    let displayText: string;
    if (isFile || isMcpTemplate) {
      const descReserve = item.description ? Math.min(20, stringWidth(item.description)) : 0;
      const maxPathLength = columns - 2 - 4 - separatorWidth - descReserve;
      displayText = truncatePathMiddle(item.displayText, maxPathLength);
    } else if (isMcpResource) {
      displayText = truncateToWidth(item.displayText, 30);
    } else {
      displayText = item.displayText;
    }

    const availableWidth = columns - 2 - stringWidth(displayText) - separatorWidth - 4;
    let lineContent: string;
    if (item.description) {
      const maxDescLength = Math.max(0, availableWidth);
      const truncatedDesc = truncateToWidth(item.description.replace(/\s+/g, ' '), maxDescLength);
      lineContent = `${icon} ${displayText} – ${truncatedDesc}`;
    } else {
      lineContent = `${icon} ${displayText}`;
    }

    return (
      <Text color={textColor} dimColor={dimColor} wrap="truncate">
        {lineContent}
      </Text>
    );
  }

  const nameColWidth = Math.floor(columns * 0.4);
  const effectiveColWidth = Math.max(maxColumnWidth ?? 0, 30);
  const displayColWidth = Math.min(effectiveColWidth, nameColWidth);
  const textColor: keyof Theme | undefined = item.color || (isSelected ? 'suggestion' : undefined);
  const shouldDim = !isSelected;

  let displayText = item.displayText;
  if (stringWidth(displayText) > displayColWidth - 2) {
    displayText = truncateToWidth(displayText, displayColWidth - 2);
  }
  const paddingLength = Math.max(0, displayColWidth - stringWidth(displayText));
  const paddingText = ' '.repeat(paddingLength);

  const tagText = item.tag ? `[${item.tag}] ` : '';
  const tagWidth = stringWidth(tagText);
  const { kindLaneText, kindLabel, sourceText } = getKindInfo(item);
  const kindColor: keyof Theme | undefined =
    kindLabel === 'skill' ? 'skill' : kindLabel === 'agent' ? 'background' : undefined;
  const laneWidth = stringWidth(kindLaneText) + stringWidth(sourceText);
  const descColWidth = Math.max(0, columns - displayColWidth - tagWidth - laneWidth - 4);

  const cleanDesc = item.description ? item.description.replace(/\s+/g, ' ').trim() : '';
  const [descLine1, descLine2] = allowWrap
    ? wrapLine(cleanDesc, descColWidth)
    : [truncateToWidth(cleanDesc, descColWidth), ''];

  const line1 = (
    <Text wrap="truncate">
      <HighlightQuery
        text={displayText}
        query={item.query}
        color={textColor}
        isSelected={isSelected}
      />
      {paddingText ? (
        <Text color={textColor} dimColor={shouldDim}>
          {paddingText}
        </Text>
      ) : null}
      {kindLaneText ? (
        <Text color={kindColor} dimColor={kindColor === undefined}>
          {kindLaneText}
        </Text>
      ) : null}
      {tagText ? <Text dimColor>{tagText}</Text> : null}
      {sourceText ? <Text dimColor>{sourceText}</Text> : null}
      <HighlightQuery
        text={descLine1}
        query={item.query}
        color={isSelected ? 'suggestion' : undefined}
        isSelected={isSelected}
        contiguousOnly={true}
      />
    </Text>
  );

  if (!descLine2) {
    return line1;
  }

  const indentWidth = displayColWidth + tagWidth + laneWidth;
  const line2Desc = truncateToWidth(descLine2, Math.max(0, columns - indentWidth - 4));
  const line2 = (
    <Text wrap="truncate">
      {' '.repeat(indentWidth)}
      <HighlightQuery
        text={line2Desc}
        query={item.query}
        color={isSelected ? 'suggestion' : undefined}
        isSelected={isSelected}
        contiguousOnly={true}
      />
    </Text>
  );

  return (
    <Box flexDirection="column">
      {line1}
      {line2}
    </Box>
  );
});

type Props = {
  suggestions: SuggestionItem[];
  selectedSuggestion: number;
  maxColumnWidth?: number;
  emptyMessage?: string;
  overlay?: boolean;
};

export function PromptInputFooterSuggestions({
  suggestions,
  selectedSuggestion,
  maxColumnWidth: maxColumnWidthProp,
  emptyMessage,
  overlay,
}: Props): ReactNode {
  const { rows, columns } = useTerminalSize();
  const d = overlay ? OVERLAY_MAX_ITEMS : Math.max(1, Math.min(Math.max(6, Math.floor(rows / 2)), rows - 3));

  if (suggestions.length === 0) {
    if (!emptyMessage) return null;
    const pad = overlay ? 0 : Math.max(0, d - 1);
    return (
      <Box
        flexDirection="column"
        justifyContent={overlay ? undefined : 'flex-end'}
      >
        <Text>{emptyMessage}</Text>
        {Array.from({ length: pad }, (_, p) => (
          <Text key={`pad-${p}`}> </Text>
        ))}
      </Box>
    );
  }

  const maxColWidth = Math.max(maxColumnWidthProp ?? 0, 30);
  const allowWrap = d >= 2;
  const itemHeights = suggestions.map((item) => (allowWrap ? calcItemLines(item, columns, maxColWidth) : 1));

  const sel = Math.max(0, Math.min(selectedSuggestion, suggestions.length - 1));
  let start = sel;
  let end = sel + 1;
  let totalH = itemHeights[sel] ?? 1;
  let v = 0;
  const half = Math.floor(d / 2);

  while (start > 0 && totalH < d && v + (itemHeights[start - 1] ?? 1) <= half) {
    start--;
    v += itemHeights[start] ?? 1;
  }
  totalH += v;

  while (end < suggestions.length && totalH + (itemHeights[end] ?? 1) <= d) {
    totalH += itemHeights[end] ?? 1;
    end++;
  }

  while (start > 0 && totalH + (itemHeights[start - 1] ?? 1) <= d) {
    start--;
    totalH += itemHeights[start] ?? 1;
  }

  const visibleItems = suggestions.slice(start, end);
  const padCount = overlay ? 0 : Math.max(0, d - totalH);

  return (
    <Box
      flexDirection="column"
      justifyContent={overlay ? undefined : 'flex-end'}
    >
      {visibleItems.map((item) => (
        <SuggestionItemRow
          key={item.id}
          item={item}
          maxColumnWidth={maxColWidth}
          isSelected={item.id === suggestions[selectedSuggestion]?.id}
          allowWrap={allowWrap}
        />
      ))}
      {Array.from({ length: padCount }, (_, p) => (
        <Text key={`pad-${p}`}> </Text>
      ))}
    </Box>
  );
}

export default memo(PromptInputFooterSuggestions);
