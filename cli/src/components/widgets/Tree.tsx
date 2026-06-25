import React from 'react';
import { ThemedBox, ThemedText } from '../design-system';
import { WidgetContainer, WidgetHeader, WidgetBody } from './base/Widget';
import { WidgetState } from './base/InteractiveWidget';
import { ColorToken } from '../tokens';

export interface TreeNode {
  label: string;
  color?: ColorToken;
  children?: TreeNode[];
}

interface TreeProps {
  title: string;
  nodes: TreeNode[];
  isFocused?: boolean;
  state?: WidgetState;
  errorMessage?: string;
}

export const Tree: React.FC<TreeProps> = ({
  title,
  nodes,
  isFocused = false,
  state = 'idle',
  errorMessage,
}) => {
  return (
    <WidgetContainer isFocused={isFocused}>
      <WidgetHeader title={title} isFocused={isFocused} state={state} errorMessage={errorMessage} />
      <WidgetBody state={state} errorMessage={errorMessage}>
        {nodes.length === 0 && state === 'idle' ? (
          <ThemedBox padding={1}>
            <ThemedText color="inactive" italic>Empty tree.</ThemedText>
          </ThemedBox>
        ) : (
          <ThemedBox flexDirection="column">
            {nodes.map((node, idx) => renderTreeNode(node, '', idx === nodes.length - 1))}
          </ThemedBox>
        )}
      </WidgetBody>
    </WidgetContainer>
  );
};

function renderTreeNode(node: TreeNode, prefix: string, isLast: boolean): React.ReactNode {
  const currentPrefix = prefix + (isLast ? '└── ' : '├── ');
  const nextPrefix = prefix + (isLast ? '    ' : '│   ');

  return (
    <ThemedBox key={node.label} flexDirection="column">
      {/* Node line */}
      <ThemedBox flexDirection="row">
        {prefix.length > 0 && (
          <ThemedText color="inactive" bold>
            {currentPrefix}
          </ThemedText>
        )}
        <ThemedText color={node.color || 'text'} bold>
          {node.label}
        </ThemedText>
      </ThemedBox>
      {/* Children */}
      {node.children &&
        node.children.map((child, idx) =>
          renderTreeNode(child, nextPrefix, idx === node.children!.length - 1)
        )}
    </ThemedBox>
  );
}
