import React from 'react';
import { Box, BoxProps } from 'ink';
import { useTheme } from '../hooks';
import { ColorToken } from '../tokens';

export interface ThemedBoxProps extends Omit<BoxProps, 
  | 'borderColor' 
  | 'borderTopColor' 
  | 'borderBottomColor' 
  | 'borderLeftColor' 
  | 'borderRightColor'
  | 'backgroundColor'
> {
  borderColor?: ColorToken | string;
  borderTopColor?: ColorToken | string;
  borderBottomColor?: ColorToken | string;
  borderLeftColor?: ColorToken | string;
  borderRightColor?: ColorToken | string;
  backgroundColor?: ColorToken | string;
  children?: React.ReactNode;
}

export const ThemedBox: React.FC<ThemedBoxProps> = ({
  borderColor,
  borderTopColor,
  borderBottomColor,
  borderLeftColor,
  borderRightColor,
  backgroundColor,
  children,
  ...boxProps
}) => {
  const { color } = useTheme();

  return (
    <Box
      borderColor={borderColor ? color(borderColor) : undefined}
      borderTopColor={borderTopColor ? color(borderTopColor) : undefined}
      borderBottomColor={borderBottomColor ? color(borderBottomColor) : undefined}
      borderLeftColor={borderLeftColor ? color(borderLeftColor) : undefined}
      borderRightColor={borderRightColor ? color(borderRightColor) : undefined}
      backgroundColor={backgroundColor ? color(backgroundColor) : undefined}
      {...boxProps}
    >
      {children}
    </Box>
  );
};
