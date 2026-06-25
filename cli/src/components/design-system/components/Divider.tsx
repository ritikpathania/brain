import React from 'react';
import { ThemedBox } from './ThemedBox';
import { ThemedText } from './ThemedText';
import { ColorToken } from '../tokens';

interface DividerProps {
  title?: string;
  color?: ColorToken | string;
}

export const Divider: React.FC<DividerProps> = ({ title, color = 'subtle' }) => {
  if (!title) {
    return (
      <ThemedBox
        borderStyle="single"
        borderTop={true}
        borderBottom={false}
        borderLeft={false}
        borderRight={false}
        borderColor={color}
        width="100%"
        height={1}
      />
    );
  }

  return (
    <ThemedBox flexDirection="row" alignItems="center" width="100%" height={1}>
      <ThemedBox
        borderStyle="single"
        borderTop={true}
        borderBottom={false}
        borderLeft={false}
        borderRight={false}
        borderColor={color}
        flexGrow={1}
      />
      <ThemedText color={color} bold>
        {` ${title} `}
      </ThemedText>
      <ThemedBox
        borderStyle="single"
        borderTop={true}
        borderBottom={false}
        borderLeft={false}
        borderRight={false}
        borderColor={color}
        flexGrow={1}
      />
    </ThemedBox>
  );
};
