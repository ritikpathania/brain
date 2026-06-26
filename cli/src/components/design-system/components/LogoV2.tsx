import React from 'react';
import { ThemedBox } from './ThemedBox';
import { ThemedText } from './ThemedText';

export const LogoV2: React.FC = () => {
  const asciiLines = [
    ' ____   ____      _      ___ _   _ ',
    '| __ ) |  _ \\    / \\    |_ _| \\ | |',
    '|  _ \\ | |_) |  / _ \\    | ||  \\| |',
    '| |_) | |  _ < / ___ \\   | || |\\  |',
    '|____/  |_| \\_\\_/   \\_\\ |___|_| \\_|',
  ];

  return (
    <ThemedBox flexDirection="column" alignItems="center" marginY={1}>
      {asciiLines.map((line, index) => (
        <ThemedText key={index} color="claude" bold>
          {line}
        </ThemedText>
      ))}
      <ThemedText color="inactive" dimColor>
        Memory Companion CLI · v0.1.0
      </ThemedText>
    </ThemedBox>
  );
};
