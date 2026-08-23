import * as React from 'react';
import { Box, Text } from '../../compat/index.js';
import type { BrainTokens } from '../../state/palettes.js';
import { paletteWindow, PALETTE_MAX_ITEMS } from './paletteLogic.js';

export interface PaletteItemVM {
  name: string;
  description: string;
}

/**
 * Rounded-border suggestion panel rendered ABOVE the composer while a slash
 * query is active. Pure view: explicit tokens, direct invocation in tests.
 */
export function PaletteView(props: {
  items: PaletteItemVM[];
  selectedIndex: number;
  maxColumns: number;
  tokens: BrainTokens;
}): React.ReactElement | null {
  const { items, selectedIndex, maxColumns, tokens } = props;
  if (items.length === 0) return null;
  const sel = Math.min(Math.max(0, selectedIndex), items.length - 1);
  const { start, end } = paletteWindow(items.length, sel);
  return (
    <Box flexDirection="column" borderStyle="round" paddingX={1}>
      {items.slice(start, end).map((item, i) => {
        const idx = start + i;
        const isSelected = idx === sel;
        const label = `${isSelected ? '❯' : ' '} /${item.name} — ${item.description}`;
        const shown =
          label.length > maxColumns
            ? `${label.slice(0, Math.max(1, maxColumns - 1))}…`
            : label;
        return (
          <Text key={item.name} inverse={isSelected}>
            {shown}
          </Text>
        );
      })}
    </Box>
  );
}

export { PALETTE_MAX_ITEMS };
