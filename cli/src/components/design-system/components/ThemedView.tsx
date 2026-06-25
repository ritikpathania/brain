import React from 'react';
import { ThemedBox, ThemedBoxProps } from './ThemedBox';
import { ThemedText } from './ThemedText';
import { ColorToken } from '../tokens';
import { useTheme } from '../hooks';

export interface PanelProps extends ThemedBoxProps {
  title?: string;
  titleColor?: ColorToken | string;
  borderColor?: ColorToken | string;
  borderStyle?: 'single' | 'double' | 'round' | 'classic' | 'none';
  backgroundColor?: ColorToken | string;
  padding?: number;
  children?: React.ReactNode;
}

export const Panel: React.FC<PanelProps> = ({
  title,
  titleColor,
  borderColor = 'promptBorder',
  borderStyle,
  backgroundColor,
  padding = 1,
  children,
  ...boxProps
}) => {
  const { border } = useTheme();

  // Resolve border style from theme if not overridden
  const resolvedBorderStyle = borderStyle === 'none' ? undefined : (borderStyle || border('style'));

  return (
    <ThemedBox
      borderStyle={resolvedBorderStyle}
      borderColor={borderColor}
      backgroundColor={backgroundColor}
      padding={padding}
      flexDirection="column"
      {...boxProps}
    >
      {title && (
        <ThemedBox marginTop={-1} marginBottom={1}>
          <ThemedText color={titleColor || borderColor} bold>
            {` ${title} `}
          </ThemedText>
        </ThemedBox>
      )}
      {children}
    </ThemedBox>
  );
};

export const Card: React.FC<PanelProps> = ({
  backgroundColor = 'userMessageBackground',
  borderColor = 'promptBorder',
  padding = 1,
  children,
  ...panelProps
}) => {
  return (
    <Panel
      backgroundColor={backgroundColor}
      borderColor={borderColor}
      padding={padding}
      {...panelProps}
    >
      {children}
    </Panel>
  );
};

export const InfoPanel: React.FC<PanelProps> = ({
  borderColor = 'suggestion',
  titleColor = 'suggestion',
  children,
  ...panelProps
}) => (
  <Panel borderColor={borderColor} titleColor={titleColor} {...panelProps}>
    {children}
  </Panel>
);

export const ErrorPanel: React.FC<PanelProps> = ({
  borderColor = 'error',
  titleColor = 'error',
  children,
  ...panelProps
}) => (
  <Panel borderColor={borderColor} titleColor={titleColor} {...panelProps}>
    {children}
  </Panel>
);

export const WarningPanel: React.FC<PanelProps> = ({
  borderColor = 'warning',
  titleColor = 'warning',
  children,
  ...panelProps
}) => (
  <Panel borderColor={borderColor} titleColor={titleColor} {...panelProps}>
    {children}
  </Panel>
);
