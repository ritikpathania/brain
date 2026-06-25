import React from 'react';
import { ThemedBox, ThemedText } from '../design-system';
import { WidgetContainer, WidgetHeader, WidgetBody } from './base/Widget';
import { WidgetState } from './base/InteractiveWidget';

export interface DiffLine {
  type: 'added' | 'removed' | 'unchanged';
  content: string;
}

interface DiffViewerProps {
  title: string;
  lines: DiffLine[];
  isFocused?: boolean;
  state?: WidgetState;
  errorMessage?: string;
}

export const DiffViewer: React.FC<DiffViewerProps> = ({
  title,
  lines,
  isFocused = false,
  state = 'idle',
  errorMessage,
}) => {
  return (
    <WidgetContainer isFocused={isFocused}>
      <WidgetHeader title={title} isFocused={isFocused} state={state} errorMessage={errorMessage} />
      <WidgetBody state={state} errorMessage={errorMessage}>
        {lines.length === 0 && state === 'idle' ? (
          <ThemedBox padding={1}>
            <ThemedText color="inactive" italic>No differences found.</ThemedText>
          </ThemedBox>
        ) : (
          <ThemedBox flexDirection="column">
            {lines.map((line, idx) => {
              let color = 'text';
              let bg = undefined;
              let prefix = '  ';

              if (line.type === 'added') {
                color = 'success';
                prefix = '+ ';
              } else if (line.type === 'removed') {
                color = 'error';
                prefix = '- ';
              }

              return (
                <ThemedBox key={idx} flexDirection="row" backgroundColor={bg} paddingX={1}>
                  <ThemedText color={color} bold marginRight={1}>
                    {prefix}
                  </ThemedText>
                  <ThemedText color={color}>
                    {line.content}
                  </ThemedText>
                </ThemedBox>
              );
            })}
          </ThemedBox>
        )}
      </WidgetBody>
    </WidgetContainer>
  );
};
