import React from 'react';
import { ThemedBox, ThemedText } from '../design-system';
import { WidgetContainer, WidgetHeader, WidgetBody } from './base/Widget';
import { WidgetState } from './base/InteractiveWidget';

interface TableProps {
  title: string;
  headers: string[];
  rows: string[][];
  isFocused?: boolean;
  state?: WidgetState;
  errorMessage?: string;
}

export const Table: React.FC<TableProps> = ({
  title,
  headers,
  rows,
  isFocused = false,
  state = 'idle',
  errorMessage,
}) => {
  // Compute column widths
  const colWidths = headers.map((header, colIdx) => {
    const maxRowLen = rows.reduce(
      (max, row) => Math.max(max, row[colIdx] ? row[colIdx].length : 0),
      0
    );
    return Math.max(header.length, maxRowLen) + 2; // add 2 spaces padding
  });

  return (
    <WidgetContainer isFocused={isFocused}>
      <WidgetHeader title={title} isFocused={isFocused} state={state} errorMessage={errorMessage} />
      <WidgetBody state={state} errorMessage={errorMessage}>
        {rows.length === 0 && state === 'idle' ? (
          <ThemedBox padding={1}>
            <ThemedText color="inactive" italic>No records found.</ThemedText>
          </ThemedBox>
        ) : (
          <ThemedBox flexDirection="column">
            {/* Header row */}
            <ThemedBox flexDirection="row" borderStyle="classic" borderColor="subtle" paddingBottom={0}>
              {headers.map((h, colIdx) => (
                <ThemedText key={colIdx} color="claude" bold width={colWidths[colIdx]}>
                  {h}
                </ThemedText>
              ))}
            </ThemedBox>
            {/* Data rows */}
            {rows.map((row, rowIdx) => (
              <ThemedBox key={rowIdx} flexDirection="row" marginTop={0}>
                {row.map((cell, colIdx) => (
                  <ThemedText key={colIdx} color="text" width={colWidths[colIdx]}>
                    {cell}
                  </ThemedText>
                ))}
              </ThemedBox>
            ))}
          </ThemedBox>
        )}
      </WidgetBody>
    </WidgetContainer>
  );
};
