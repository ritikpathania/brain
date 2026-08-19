import * as React from 'react';
import { useEffect, useState } from 'react';
import { Box, Text, color, useTheme } from '../../vendor/claude/ink.js';
import { useTerminalSize } from '../../vendor/claude/hooks/useTerminalSize.js';
import { stringWidth } from '../../vendor/claude/ink/stringWidth.js';
import { getLayoutMode, calculateLayoutDimensions, calculateOptimalLeftWidth, formatWelcomeMessage, truncatePath, getRecentActivitySync, getRecentReleaseNotesSync, getLogoDisplayData } from '../../vendor/claude/utils/logoV2Utils.js';
import { truncate } from '../../vendor/claude/utils/format.js';
import { getDisplayPath } from '../../vendor/claude/utils/file.js';
import { Clawd } from '../../vendor/claude/components/LogoV2/Clawd.js';
import { FeedColumn } from '../../vendor/claude/components/LogoV2/FeedColumn.js';
import { createRecentActivityFeed, createWhatsNewFeed, createProjectOnboardingFeed, createGuestPassesFeed } from '../../vendor/claude/components/LogoV2/feedConfigs.js';
import { getGlobalConfig, saveGlobalConfig } from 'src/utils/config.js';
import { resolveThemeSetting } from 'src/utils/systemTheme.js';
import { getInitialSettings } from 'src/utils/settings/settings.js';
import { isDebugMode, isDebugToStdErr, getDebugLogPath } from 'src/utils/debug.js';
import { getSteps, shouldShowProjectOnboarding, incrementProjectOnboardingSeenCount } from '../../vendor/claude/projectOnboardingState.js';
import { CondensedLogo } from '../../vendor/claude/components/LogoV2/CondensedLogo.js';
import { checkForReleaseNotesSync } from '../../vendor/claude/utils/releaseNotes.js';
import { getDumpPromptsPath } from 'src/services/api/dumpPrompts.js';
import { isEnvTruthy } from 'src/utils/envUtils.js';
import { getStartupPerfLogPath, isDetailedProfilingEnabled } from 'src/utils/startupProfiler.js';
import { EmergencyTip } from '../../vendor/claude/components/LogoV2/EmergencyTip.js';
import { VoiceModeNotice } from '../../vendor/claude/components/LogoV2/VoiceModeNotice.js';
import { Opus1mMergeNotice } from '../../vendor/claude/components/LogoV2/Opus1mMergeNotice.js';
import { feature } from 'bun:bundle';
import { SandboxManager } from 'src/utils/sandbox/sandbox-adapter.js';
import { useShowGuestPassesUpsell, incrementGuestPassesSeenCount } from '../../vendor/claude/components/LogoV2/GuestPassesUpsell.js';
import { useShowOverageCreditUpsell, incrementOverageCreditUpsellSeenCount, createOverageCreditFeed } from '../../vendor/claude/components/LogoV2/OverageCreditUpsell.js';
import { useAppState } from '../../vendor/claude/state/AppState.js';
import { useMainLoopModel } from '../../vendor/claude/hooks/useMainLoopModel.js';
import { renderModelSetting } from '../../vendor/claude/utils/model/model.js';
import { getEffortSuffix } from '../../vendor/claude/utils/effort.js';

/* eslint-disable @typescript-eslint/no-require-imports */
const ChannelsNoticeModule = feature('KAIROS') || feature('KAIROS_CHANNELS') ? require('../../vendor/claude/components/LogoV2/ChannelsNotice.js') as typeof import('../../vendor/claude/components/LogoV2/ChannelsNotice.js') : null;
/* eslint-enable @typescript-eslint/no-require-imports */

const LEFT_PANEL_MAX_WIDTH = 50;

export function LogoV2(): React.ReactNode {
  'use no memo';

  const [themeName] = useTheme();

  const activities = getRecentActivitySync();
  const username = getGlobalConfig().oauthAccount?.displayName ?? '';
  const { columns } = useTerminalSize();
  const showOnboarding = shouldShowProjectOnboarding();
  const showSandboxStatus = SandboxManager.isSandboxingEnabled();
  const showGuestPassesUpsell = useShowGuestPassesUpsell();
  const showOverageCreditUpsell = useShowOverageCreditUpsell();
  const agent = useAppState((s: any) => s.agent);
  const effortValue = useAppState((s: any) => s.effortValue);
  const config = getGlobalConfig();

  let changelog: any[];
  try {
    changelog = getRecentReleaseNotesSync(3);
  } catch {
    changelog = [];
  }

  const [announcement] = useState(() => {
    const announcements = getInitialSettings().companyAnnouncements;
    if (!announcements || announcements.length === 0) return;
    return config.numStartups === 1
      ? announcements[0]
      : announcements[Math.floor(Math.random() * announcements.length)];
  });

  const { hasReleaseNotes } = checkForReleaseNotesSync(config.lastReleaseNotesSeen);

  useEffect(() => {
    const currentConfig = getGlobalConfig();
    const currentVersion = (globalThis as any).MACRO?.VERSION;
    if (currentConfig.lastReleaseNotesSeen === currentVersion) return;
    saveGlobalConfig((curr: any) => ({
      ...curr,
      lastReleaseNotesSeen: currentVersion,
    }));
    if (showOnboarding) {
      incrementProjectOnboardingSeenCount();
    }
  }, [config, showOnboarding]);

  const [isCondensedMode] = useState(
    () => !hasReleaseNotes && !showOnboarding && !isEnvTruthy(process.env.CLAUDE_CODE_FORCE_FULL_LOGO)
  );

  useEffect(() => {
    if (showGuestPassesUpsell && !showOnboarding && !isCondensedMode) {
      incrementGuestPassesSeenCount();
    }
  }, [showGuestPassesUpsell, showOnboarding, isCondensedMode]);

  useEffect(() => {
    if (showOverageCreditUpsell && !showOnboarding && !showGuestPassesUpsell && !isCondensedMode) {
      incrementOverageCreditUpsellSeenCount();
    }
  }, [showOverageCreditUpsell, showOnboarding, showGuestPassesUpsell, isCondensedMode]);

  const model = useMainLoopModel();
  const fullModelDisplayName = renderModelSetting(model);
  const { version, cwd, billingType, agentName: agentNameFromSettings } = getLogoDisplayData();
  const agentName = agent ?? agentNameFromSettings;
  const effortSuffix = getEffortSuffix(model, effortValue);
  const modelDisplayName = truncate(fullModelDisplayName + effortSuffix, LEFT_PANEL_MAX_WIDTH - 20);

  const debugWarning =
    isDebugMode() && isDebugToStdErr() ? (
      <Box marginTop={1}>
        <Text color="warning">
          Warning: Debug mode is enabled and logging to stderr, this will cause rendering artifacts.
        </Text>
      </Box>
    ) : null;

  if (isCondensedMode) {
    return (
      <>
        <CondensedLogo />
        <EmergencyTip />
        <VoiceModeNotice />
        <Opus1mMergeNotice />
        {ChannelsNoticeModule && <ChannelsNoticeModule.ChannelsNotice />}
        {showSandboxStatus && (
          <Box marginTop={1} flexDirection="column">
            <Text color="warning">Your bash commands will be sandboxed. Disable with /sandbox.</Text>
          </Box>
        )}
        {debugWarning}
      </>
    );
  }

  const layoutMode = getLayoutMode(columns);
  const userTheme = resolveThemeSetting(themeName);
  const borderTitle = ` ${color('claude', userTheme)('Claude Code')} ${color('inactive', userTheme)(`v${version}`)} `;
  const compactBorderTitle = color('claude', userTheme)(' Claude Code ');

  if (layoutMode === 'compact') {
    let welcomeMessage = formatWelcomeMessage(username);
    if (stringWidth(welcomeMessage) > columns - 4) {
      welcomeMessage = formatWelcomeMessage(null);
    }
    const cwdAvailableWidth = agentName
      ? columns - 4 - 1 - stringWidth(agentName) - 3
      : columns - 4;
    const truncatedCwd = truncatePath(cwd, Math.max(cwdAvailableWidth, 10));

    return (
      <>
        <Box
          flexDirection="column"
          borderStyle="round"
          borderColor="claude"
          borderText={{
            content: compactBorderTitle,
            position: 'top',
            align: 'start',
            offset: 1,
          }}
          paddingX={1}
          paddingY={1}
          alignItems="center"
          width={columns}
        >
          <Text bold={true}>{welcomeMessage}</Text>
          <Box marginY={1}>
            <Clawd />
          </Box>
          <Text dimColor={true}>{modelDisplayName}</Text>
          <Text dimColor={true}>{billingType}</Text>
          <Text dimColor={true}>{agentName ? `@${agentName} · ${truncatedCwd}` : truncatedCwd}</Text>
        </Box>
        <VoiceModeNotice />
        <Opus1mMergeNotice />
        {ChannelsNoticeModule && <ChannelsNoticeModule.ChannelsNotice />}
        {showSandboxStatus && (
          <Box marginTop={1} flexDirection="column">
            <Text color="warning">Your bash commands will be sandboxed. Disable with /sandbox.</Text>
          </Box>
        )}
        {debugWarning}
      </>
    );
  }

  const welcomeMessage = formatWelcomeMessage(username);
  const modelLine =
    !process.env.IS_DEMO && config.oauthAccount?.organizationName
      ? `${modelDisplayName} · ${billingType} · ${config.oauthAccount.organizationName}`
      : `${modelDisplayName} · ${billingType}`;
  const cwdAvailableWidth = agentName
    ? LEFT_PANEL_MAX_WIDTH - 1 - stringWidth(agentName) - 3
    : LEFT_PANEL_MAX_WIDTH;
  const truncatedCwd = truncatePath(cwd, Math.max(cwdAvailableWidth, 10));
  const cwdLine = agentName ? `@${agentName} · ${truncatedCwd}` : truncatedCwd;
  const optimalLeftWidth = calculateOptimalLeftWidth(welcomeMessage, cwdLine, modelLine);
  const { leftWidth, rightWidth } = calculateLayoutDimensions(
    columns,
    layoutMode,
    optimalLeftWidth,
  );

  const feeds = showOnboarding
    ? [createProjectOnboardingFeed(getSteps()), createWhatsNewFeed(changelog)]
    : showGuestPassesUpsell
    ? [createRecentActivityFeed(activities), createGuestPassesFeed()]
    : showOverageCreditUpsell
    ? [createRecentActivityFeed(activities), createOverageCreditFeed()]
    : [createWhatsNewFeed(changelog)];

  return (
    <>
      <Box
        flexDirection="column"
        borderStyle="round"
        borderColor="claude"
        borderText={{
          content: borderTitle,
          position: 'top',
          align: 'start',
          offset: 3,
        }}
      >
        <Box flexDirection={layoutMode === 'horizontal' ? 'row' : 'column'} paddingX={1} gap={1}>
          <Box
            flexDirection="column"
            width={leftWidth}
            justifyContent="space-between"
            alignItems="center"
            minHeight={9}
          >
            <Box marginTop={1}>
              <Text bold={true}>{welcomeMessage}</Text>
            </Box>
            <Clawd />
            <Box flexDirection="column" alignItems="center">
              <Text dimColor={true}>{modelLine}</Text>
              <Text dimColor={true}>{cwdLine}</Text>
            </Box>
          </Box>
          {layoutMode === 'horizontal' && (
            <Box
              height="100%"
              borderStyle="single"
              borderColor="claude"
              borderDimColor={true}
              borderTop={false}
              borderBottom={false}
              borderLeft={false}
            />
          )}
          {layoutMode === 'horizontal' && <FeedColumn feeds={feeds} maxWidth={rightWidth} />}
        </Box>
      </Box>
      <VoiceModeNotice />
      <Opus1mMergeNotice />
      {ChannelsNoticeModule && <ChannelsNoticeModule.ChannelsNotice />}
      {isDebugMode() && (
        <Box paddingLeft={2} flexDirection="column">
          <Text color="warning">Debug mode enabled</Text>
          <Text dimColor={true}>
            Logging to: {isDebugToStdErr() ? 'stderr' : getDebugLogPath()}
          </Text>
        </Box>
      )}
      <EmergencyTip />
      {process.env.CLAUDE_CODE_TMUX_SESSION && (
        <Box paddingLeft={2} flexDirection="column">
          <Text dimColor={true}>tmux session: {process.env.CLAUDE_CODE_TMUX_SESSION}</Text>
          <Text dimColor={true}>
            {process.env.CLAUDE_CODE_TMUX_PREFIX_CONFLICTS
              ? `Detach: ${process.env.CLAUDE_CODE_TMUX_PREFIX} ${process.env.CLAUDE_CODE_TMUX_PREFIX} d (press prefix twice - Claude uses ${process.env.CLAUDE_CODE_TMUX_PREFIX})`
              : `Detach: ${process.env.CLAUDE_CODE_TMUX_PREFIX} d`}
          </Text>
        </Box>
      )}
      {announcement && (
        <Box paddingLeft={2} flexDirection="column">
          {!process.env.IS_DEMO && config.oauthAccount?.organizationName && (
            <Text dimColor={true}>Message from {config.oauthAccount.organizationName}:</Text>
          )}
          <Text>{announcement}</Text>
        </Box>
      )}
      {showSandboxStatus && (
        <Box paddingLeft={2} flexDirection="column">
          <Text color="warning">Your bash commands will be sandboxed. Disable with /sandbox.</Text>
        </Box>
      )}
    </>
  );
}