import React from 'react';
import { ThemedBox, ThemedText } from '../design-system';
import { WidgetContainer, WidgetHeader, WidgetBody } from './base/Widget';
import { WidgetState } from './base/InteractiveWidget';
import { Progress } from '../design-system/components/HeadlessComponents';

export interface ProgressTask {
  id: string;
  name: string;
  percent: number;
  status: 'running' | 'completed' | 'failed' | 'pending';
}

interface ProgressTasksProps {
  title: string;
  tasks: ProgressTask[];
  isFocused?: boolean;
  state?: WidgetState;
  errorMessage?: string;
}

export const ProgressTasks: React.FC<ProgressTasksProps> = ({
  title,
  tasks,
  isFocused = false,
  state = 'idle',
  errorMessage,
}) => {
  return (
    <WidgetContainer isFocused={isFocused}>
      <WidgetHeader title={title} isFocused={isFocused} state={state} errorMessage={errorMessage} />
      <WidgetBody state={state} errorMessage={errorMessage}>
        {tasks.length === 0 && state === 'idle' ? (
          <ThemedBox padding={1}>
            <ThemedText color="inactive" italic>No active tasks.</ThemedText>
          </ThemedBox>
        ) : (
          <ThemedBox flexDirection="column">
            {tasks.map((task) => {
              let statusText = '';
              let statusColor = 'text';

              if (task.status === 'completed') {
                statusText = '✓ OK';
                statusColor = 'success';
              } else if (task.status === 'failed') {
                statusText = '✗ FAILED';
                statusColor = 'error';
              } else if (task.status === 'pending') {
                statusText = '○ PENDING';
                statusColor = 'inactive';
              } else {
                statusText = '▶ RUNNING';
                statusColor = 'claude';
              }

              return (
                <ThemedBox key={task.id} flexDirection="column" marginY={1}>
                  <ThemedBox flexDirection="row" justifyContent="space-between">
                    <ThemedText color="text" bold>
                      {task.name}
                    </ThemedText>
                    <ThemedText color={statusColor as any} bold>
                      {statusText}
                    </ThemedText>
                  </ThemedBox>
                  <ThemedBox marginTop={0}>
                    <Progress percent={task.percent} width={25} />
                  </ThemedBox>
                </ThemedBox>
              );
            })}
          </ThemedBox>
        )}
      </WidgetBody>
    </WidgetContainer>
  );
};
