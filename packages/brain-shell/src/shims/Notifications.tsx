import { feature } from 'bun:bundle';
import * as React from 'react';
import { type ReactNode, useEffect, useMemo, useState } from 'react';
import { type Notification, useNotifications } from '../../vendor/claude/context/notifications.js';
import { logEvent } from '../../vendor/claude/services/analytics/index.js';
import { useAppState } from '../../vendor/claude/state/AppState.js';
import { useVoiceState } from '../../vendor/claude/context/voice.js';
import type { VerificationStatus } from '../../vendor/claude/hooks/useApiKeyVerification.js';
import { useIdeConnectionStatus } from '../../vendor/claude/hooks/useIdeConnectionStatus.js';
import type { IDESelection } from '../../vendor/claude/hooks/useIdeSelection.js';
import { useMainLoopModel } from '../../vendor/claude/hooks/useMainLoopModel.js';
import { useVoiceEnabled } from '../../vendor/claude/hooks/useVoiceEnabled.js';
import { Box, Text } from '../../vendor/claude/ink.js';
import { useClaudeAiLimits } from '../../vendor/claude/services/claudeAiLimitsHook.js';
import { calculateTokenWarningState } from '../../vendor/claude/services/compact/autoCompact.js';
import type { MCPServerConnection } from '../../vendor/claude/services/mcp/types.js';
import type { Message } from '../../vendor/claude/types/message.js';
import { getApiKeyHelperElapsedMs, getConfiguredApiKeyHelper, getSubscriptionType } from '../../vendor/claude/utils/auth.js';
import type { AutoUpdaterResult } from '../../vendor/claude/utils/autoUpdater.js';
import { getExternalEditor } from '../../vendor/claude/utils/editor.js';
import { isEnvTruthy } from '../../vendor/claude/utils/envUtils.js';
import { formatDuration } from '../../vendor/claude/utils/format.js';
import { setEnvHookNotifier } from '../../vendor/claude/utils/hooks/fileChangedWatcher.js';
import { toIDEDisplayName } from '../../vendor/claude/utils/ide.js';
import { getMessagesAfterCompactBoundary } from '../../vendor/claude/utils/messages.js';
import { tokenCountFromLastAPIResponse } from '../../vendor/claude/utils/tokens.js';
import { AutoUpdaterWrapper } from '../../vendor/claude/components/AutoUpdaterWrapper.js';
import { ConfigurableShortcutHint } from '../../vendor/claude/components/ConfigurableShortcutHint.js';
import { IdeStatusIndicator } from '../../vendor/claude/components/IdeStatusIndicator.js';
import { MemoryUsageIndicator } from '../../vendor/claude/components/MemoryUsageIndicator.js';
import { SentryErrorBoundary } from '../../vendor/claude/components/SentryErrorBoundary.js';
import { TokenWarning } from '../../vendor/claude/components/TokenWarning.js';
import { SandboxPromptFooterHint } from '../../vendor/claude/components/PromptInput/SandboxPromptFooterHint.js';

export const FOOTER_TEMPORARY_STATUS_TIMEOUT = 5000;

type Props = {
  apiKeyStatus: VerificationStatus;
  autoUpdaterResult: AutoUpdaterResult | null;
  isAutoUpdating: boolean;
  debug: boolean;
  verbose: boolean;
  messages: Message[];
  onAutoUpdaterResult: (result: AutoUpdaterResult) => void;
  onChangeIsUpdating: (isUpdating: boolean) => void;
  ideSelection: IDESelection | undefined;
  mcpClients?: MCPServerConnection[];
  isInputWrapped?: boolean;
  isNarrow?: boolean;
};

export function Notifications({
  apiKeyStatus,
  autoUpdaterResult,
  debug,
  isAutoUpdating,
  verbose,
  messages,
  onAutoUpdaterResult,
  onChangeIsUpdating,
  ideSelection,
  mcpClients,
  isInputWrapped = false,
  isNarrow = false,
}: Props): ReactNode {
  const tokenUsage = useMemo(() => {
    const messagesForTokenCount = getMessagesAfterCompactBoundary(messages);
    return tokenCountFromLastAPIResponse(messagesForTokenCount);
  }, [messages]);

  const mainLoopModel = useMainLoopModel();
  const isShowingCompactMessage = calculateTokenWarningState(
    tokenUsage,
    mainLoopModel,
  ).isAboveWarningThreshold;
  const { status: ideStatus } = useIdeConnectionStatus(mcpClients);
  const notifications = useAppState(s => s.notifications);
  const { addNotification, removeNotification } = useNotifications();
  const claudeAiLimits = useClaudeAiLimits();

  useEffect(() => {
    setEnvHookNotifier((text, isError) => {
      addNotification({
        key: 'env-hook',
        text,
        color: isError ? 'error' : undefined,
        priority: isError ? 'medium' : 'low',
        timeoutMs: isError ? 8000 : 5000,
      });
    });
    return () => setEnvHookNotifier(null);
  }, [addNotification]);

  const shouldShowIdeSelection =
    ideStatus === 'connected' &&
    (ideSelection?.filePath ||
      (ideSelection?.text && ideSelection.lineCount > 0));

  const shouldShowAutoUpdater =
    !shouldShowIdeSelection ||
    isAutoUpdating ||
    autoUpdaterResult?.status !== 'success';

  const isUsingOverage = claudeAiLimits.isUsingOverage;
  const subscriptionType = getSubscriptionType();
  const isTeamOrEnterprise =
    subscriptionType === 'team' || subscriptionType === 'enterprise';

  const editor = getExternalEditor();
  const shouldShowExternalEditorHint =
    isInputWrapped &&
    !isShowingCompactMessage &&
    apiKeyStatus !== 'invalid' &&
    apiKeyStatus !== 'missing' &&
    editor !== undefined;

  useEffect(() => {
    if (shouldShowExternalEditorHint && editor) {
      logEvent('tengu_external_editor_hint_shown', {});
      addNotification({
        key: 'external-editor-hint',
        jsx: (
          <Text dimColor>
            <ConfigurableShortcutHint
              action="chat:externalEditor"
              context="Chat"
              fallback="ctrl+g"
              description={`edit in ${toIDEDisplayName(editor)}`}
            />
          </Text>
        ),
        priority: 'immediate',
        timeoutMs: 5000,
      });
    } else {
      removeNotification('external-editor-hint');
    }
  }, [
    shouldShowExternalEditorHint,
    editor,
    addNotification,
    removeNotification,
  ]);

  return (
    <SentryErrorBoundary>
      <Box
        flexDirection="column"
        alignItems={isNarrow ? 'flex-start' : 'flex-end'}
        flexShrink={0}
        overflowX="hidden"
      >
        <NotificationContent
          ideSelection={ideSelection}
          mcpClients={mcpClients}
          notifications={notifications}
          isInOverageMode={isUsingOverage ?? false}
          isTeamOrEnterprise={isTeamOrEnterprise}
          apiKeyStatus={apiKeyStatus}
          debug={debug}
          verbose={verbose}
          tokenUsage={tokenUsage}
          mainLoopModel={mainLoopModel}
          shouldShowAutoUpdater={shouldShowAutoUpdater}
          autoUpdaterResult={autoUpdaterResult}
          isAutoUpdating={isAutoUpdating}
          isShowingCompactMessage={isShowingCompactMessage}
          onAutoUpdaterResult={onAutoUpdaterResult}
          onChangeIsUpdating={onChangeIsUpdating}
        />
      </Box>
    </SentryErrorBoundary>
  );
}

function NotificationContent({
  ideSelection,
  mcpClients,
  notifications,
  isInOverageMode,
  isTeamOrEnterprise,
  apiKeyStatus,
  debug,
  verbose,
  tokenUsage,
  mainLoopModel,
  shouldShowAutoUpdater,
  autoUpdaterResult,
  isAutoUpdating,
  isShowingCompactMessage,
  onAutoUpdaterResult,
  onChangeIsUpdating,
}: {
  ideSelection: IDESelection | undefined;
  mcpClients?: MCPServerConnection[];
  notifications: {
    current: Notification | null;
    queue: Notification[];
  };
  isInOverageMode: boolean;
  isTeamOrEnterprise: boolean;
  apiKeyStatus: VerificationStatus;
  debug: boolean;
  verbose: boolean;
  tokenUsage: number;
  mainLoopModel: string;
  shouldShowAutoUpdater: boolean;
  autoUpdaterResult: AutoUpdaterResult | null;
  isAutoUpdating: boolean;
  isShowingCompactMessage: boolean;
  onAutoUpdaterResult: (result: AutoUpdaterResult) => void;
  onChangeIsUpdating: (isUpdating: boolean) => void;
}): ReactNode {
  const [apiKeyHelperSlow, setApiKeyHelperSlow] = useState<string | null>(null);
  useEffect(() => {
    if (!getConfiguredApiKeyHelper()) return;
    const interval = setInterval(
      (setSlow: React.Dispatch<React.SetStateAction<string | null>>) => {
        const ms = getApiKeyHelperElapsedMs();
        const next = ms >= 10_000 ? formatDuration(ms) : null;
        setSlow(prev => (next === prev ? prev : next));
      },
      1000,
      setApiKeyHelperSlow,
    );
    return () => clearInterval(interval);
  }, []);

  return (
    <>
      <IdeStatusIndicator ideSelection={ideSelection} mcpClients={mcpClients} />
      {isInOverageMode && !isTeamOrEnterprise && (
        <Box>
          <Text dimColor wrap="truncate">
            Now using extra usage
          </Text>
        </Box>
      )}
      {apiKeyHelperSlow && (
        <Box>
          <Text color="warning" wrap="truncate">
            apiKeyHelper is taking a while{' '}
          </Text>
          <Text dimColor wrap="truncate">
            ({apiKeyHelperSlow})
          </Text>
        </Box>
      )}
      {(apiKeyStatus === 'invalid' || apiKeyStatus === 'missing') && (
        <Box>
          <Text color="error" wrap="truncate">
            {isEnvTruthy(process.env.CLAUDE_CODE_REMOTE)
              ? 'Authentication error · Try again'
              : 'Not logged in · Run /login'}
          </Text>
        </Box>
      )}
      {notifications.current &&
        ('jsx' in notifications.current ? (
          <Text wrap="truncate" key={notifications.current.key}>
            {notifications.current.jsx}
          </Text>
        ) : (
          <Text
            color={notifications.current.color}
            dimColor={!notifications.current.color}
            wrap="truncate"
          >
            {notifications.current.text}
          </Text>
        ))}
      {debug && (
        <Box>
          <Text color="warning" wrap="truncate">
            Debug mode
          </Text>
        </Box>
      )}
      {apiKeyStatus !== 'invalid' && apiKeyStatus !== 'missing' && verbose && (
        <Box>
          <Text dimColor wrap="truncate">
            {tokenUsage} tokens
          </Text>
        </Box>
      )}
      <TokenWarning tokenUsage={tokenUsage} model={mainLoopModel} />
      {shouldShowAutoUpdater && (
        <AutoUpdaterWrapper
          verbose={verbose}
          onAutoUpdaterResult={onAutoUpdaterResult}
          autoUpdaterResult={autoUpdaterResult}
          isUpdating={isAutoUpdating}
          onChangeIsUpdating={onChangeIsUpdating}
          showSuccessMessage={!isShowingCompactMessage}
        />
      )}
      <MemoryUsageIndicator />
      <SandboxPromptFooterHint />
    </>
  );
}
