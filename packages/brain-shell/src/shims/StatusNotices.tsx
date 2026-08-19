import * as React from 'react';
import { Box } from '../../vendor/claude/ink.js';
import type { AgentDefinitionsResult } from '../../vendor/claude/tools/AgentTool/loadAgentsDir.js';
import { getGlobalConfig } from '../../vendor/claude/utils/config.js';
import {
  getActiveNotices,
  type StatusNoticeContext,
} from '../../vendor/claude/utils/statusNoticeDefinitions.js';

type Props = {
  agentDefinitions?: AgentDefinitionsResult;
};

export function StatusNotices({ agentDefinitions }: Props): React.ReactNode {
  const config = getGlobalConfig();
  const memoryFiles: any[] = [];
  const context: StatusNoticeContext = {
    config,
    agentDefinitions,
    memoryFiles: memoryFiles as any,
  };
  const activeNotices = getActiveNotices(context);
  if (activeNotices.length === 0) {
    return <Box />;
  }
  return (
    <Box flexDirection="column" paddingLeft={1}>
      {activeNotices.map((notice) => (
        <React.Fragment key={notice.id}>
          {notice.render(context)}
        </React.Fragment>
      ))}
    </Box>
  );
}
