import React from 'react';
import { Box, Text } from '../../vendor/claude/ink.js';
import { Byline } from '../../vendor/claude/components/design-system/Byline.js';
import { isVimModeEnabled } from '../../vendor/claude/components/PromptInput/utils.js';
import type { VimMode, PromptInputMode } from '../../vendor/claude/types/textInputTypes.js';
import type { ToolPermissionContext } from '../../vendor/claude/Tool.js';

type Props = {
  exitMessage: {
    show: boolean;
    key?: string;
  };
  vimMode: VimMode | undefined;
  mode: PromptInputMode;
  toolPermissionContext: ToolPermissionContext;
  suppressHint: boolean;
  isLoading: boolean;
  showMemoryTypeSelector?: boolean;
  tasksSelected: boolean;
  teamsSelected: boolean;
  tmuxSelected: boolean;
  teammateFooterIndex?: number;
  isPasting?: boolean;
  isSearching: boolean;
  historyQuery: string;
  setHistoryQuery: (query: string) => void;
  historyFailedMatch: boolean;
  onOpenTasksDialog?: (taskId?: string) => void;
};

export function PromptInputFooterLeftSide({
  exitMessage,
  vimMode,
  mode,
  toolPermissionContext,
  suppressHint,
  isLoading,
  tasksSelected,
  teamsSelected,
  tmuxSelected,
  teammateFooterIndex,
  isPasting,
  isSearching,
  historyQuery,
  setHistoryQuery,
  historyFailedMatch,
  onOpenTasksDialog,
}: Props): React.ReactNode {
  if (exitMessage.show) {
    return (
      <Text dimColor key="exit-message">
        Press {exitMessage.key} again to exit
      </Text>
    );
  }
  if (isPasting) {
    return (
      <Text dimColor key="pasting-message">
        Pasting text…
      </Text>
    );
  }

  const showVim = isVimModeEnabled() && vimMode === 'INSERT' && !isSearching;

  const modePart = (
    <Text color="inactive" key="mode">
      ⏸ manual mode on
    </Text>
  );

  const parts: React.ReactNode[] = [];
  if (!suppressHint) {
    if (!showVim) {
      parts.push(
        <Text dimColor key="shortcuts-hint">
          ? for shortcuts
        </Text>
      );
    }
    parts.push(
      <Text dimColor key="agents-hint">
        ← for agents
      </Text>
    );
  }

  return (
    <Box justifyContent="flex-start" gap={1}>
      {showVim ? (
        <Text dimColor key="vim-insert">
          -- INSERT --
        </Text>
      ) : null}
      <Box height={1} overflow="hidden" flexShrink={0}>
        <Box flexShrink={0}>
          {modePart}
          {parts.length > 0 && <Text dimColor> · </Text>}
        </Box>
        {parts.length > 0 && (
          <Box flexShrink={0}>
            <Byline>{parts}</Byline>
          </Box>
        )}
      </Box>
    </Box>
  );
}
