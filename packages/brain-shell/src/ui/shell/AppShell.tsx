import * as React from 'react';
import { Box, Text, useTerminalSize } from '../../compat/index.js';
import { WelcomeFrame } from './WelcomeFrame.js';
import { Spinner, spinnerLabel } from './Spinner.js';
import { MessageRow } from '../transcript/MessageRow.js';
import { PromptInput } from '../composer/PromptInput.js';
import { SessionController } from '../../state/sessionController.js';
import { UdsBrainBackendClient } from '../../client/UdsBrainBackendClient.js';
import { useMainLoopModel } from '../../contracts/model.js';
import { useShellSnapshot } from './useShellSnapshot.js';
import { useBoundInput } from '../../keybindings/useBoundInput.js';
import { COMMANDS } from '../../commands/matcher.js';

/**
 * Top-level live shell: frozen transcript + streaming block + composer.
 * Static/live split via React.memo rows (not ink <Static>) so ctrl+o can
 * re-render past tool cards.
 */
export function AppShell(): React.ReactElement {
  const { columns } = useTerminalSize();
  const model = useMainLoopModel(); // hoisted — hooks never inside JSX
  // One controller per mount; UDS path resolves from BRAIN_SOCKET_PATH.
  const controller = React.useMemo(
    () => new SessionController(new UdsBrainBackendClient()),
    [],
  );
  const snapshot = useShellSnapshot(controller);
  const [expandTools, setExpandTools] = React.useState(false);

  useBoundInput({
    contexts: ['global'],
    onAction: (action) => {
      if (action === 'shell:exit') process.exit(0);
      if (action === 'shell:toggleTools') setExpandTools((v) => !v);
    },
  });

  const helpText = (): string =>
    ['Slash commands:', ...COMMANDS.map((c) => `/${c.name} — ${c.description}`)].join('\n');

  const runCommand = (rawValue: string): void => {
    const token = rawValue.trim().slice(1).toLowerCase(); // strip '/', tolerate trailing space
    if (token.length === 0) return;
    const exact = COMMANDS.find(
      (c) => c.name === token || (c.aliases ?? []).includes(token),
    );
    let chosen = exact;
    if (chosen === undefined) {
      const prefixHits = COMMANDS.filter((c) => c.name.startsWith(token));
      if (prefixHits.length === 1) chosen = prefixHits[0];
      else if (prefixHits.length > 1) {
        controller.notice(`Ambiguous command: /${token}`);
        return;
      } else {
        controller.notice(`Unknown command: /${token}`);
        return;
      }
    }
    if (chosen.name === 'help') controller.notice(helpText());
    else if (chosen.name === 'clear') controller.clear();
    else if (chosen.name === 'quit') process.exit(0);
  };

  const handleSubmit = (text: string): void => {
    if (text.trimStart().startsWith('/')) runCommand(text);
    else void controller.submit(text);
  };

  const lastThinking =
    snapshot.live.thinkingText.length > 0
      ? snapshot.live.thinkingText.trimEnd().split('\n').slice(-1)[0]!
      : '';

  return (
    <Box flexDirection="column" width={columns}>
      {snapshot.rows.length === 0 && !snapshot.busy ? <WelcomeFrame /> : null}
      {snapshot.rows.map((row) => (
        <MessageRow key={row.id} row={row} expanded={expandTools} />
      ))}
      {snapshot.busy ? (
        <Box marginTop={1} flexDirection="column">
          <Spinner label={spinnerLabel(snapshot.live)} />
          {lastThinking.length > 0 ? (
            <Text dimColor italic>
              {lastThinking}
            </Text>
          ) : null}
          {snapshot.live.responseText.length > 0 ? (
            <Text>{snapshot.live.responseText}</Text>
          ) : null}
        </Box>
      ) : null}
      {snapshot.connectionError !== undefined ? (
        <Text color="red">⚠ {snapshot.connectionError}</Text>
      ) : null}
      <Box marginTop={1}>
        <PromptInput
          disabled={false}
          busy={snapshot.busy}
          onSubmit={handleSubmit}
          onAbort={() => controller.abort()}
        />
      </Box>
      <Text dimColor>
        model: {model} · ! bash · / commands · ↑↓ history · esc stop · ctrl+o{' '}
        {expandTools ? 'collapse' : 'expand'} tools · ctrl+c exit
      </Text>
    </Box>
  );
}
