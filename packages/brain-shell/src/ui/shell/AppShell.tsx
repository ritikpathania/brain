import * as React from 'react';
import * as path from 'path';
import { Box, Text, useTerminalSize } from '../../compat/index.js';
import { WelcomeFrame } from './WelcomeFrame.js';
import { StatusBarView } from './StatusBar.js';
import { connectionStatusText } from './connectionStatusLogic.js';
import { Spinner, spinnerLabel } from './Spinner.js';
import { MessageRow } from '../transcript/MessageRow.js';
import { PromptInput } from '../composer/PromptInput.js';
import { SessionController } from '../../state/sessionController.js';
import { makeUserExit, planQuit } from './exitLogic.js';
import { UdsBrainBackendClient } from '../../client/UdsBrainBackendClient.js';
import { useShellSnapshot } from './useShellSnapshot.js';
import { useBoundInput } from '../../keybindings/useBoundInput.js';
import { getCommand, getCommands } from '../../commands/registry.js';
import '../../commands/builtin.js';
import { DoctorProbe, type EngineDiagnosticReport } from '../../adapter/doctorProbe.js';
import { DoctorOverlayView } from '../overlays/DoctorOverlayView.js';
import { ContextOverlayView } from '../overlays/ContextOverlayView.js';
import { MemoryOverlayView } from '../overlays/MemoryOverlayView.js';
import { ModelPickerView } from '../overlays/ModelPickerView.js';
import { toModelItemVM, modelListDecision } from '../overlays/modelPickerLogic.js';
import { clampSelection } from '../overlays/memoryOverlayLogic.js';
import type { RetrievedMemory } from '../../client/BrainBackendClient.js';
import { useTheme } from '../../compat/index.js';
import { overlayListDecision } from '../overlays/overlayLogic.js';
import { THEME_CHOICES, ThemePickerView } from '../overlays/ThemePicker.js';
import {
  applyQueryEdit,
  resumeChoices,
  resumeListDecision,
  resolveDirectResumeTarget,
} from '../overlays/resumePickerLogic.js';
import type {
  BrainModelDescriptor,
  BrainSessionSummary,
  TranscriptSearchMatch,
} from '../../client/BrainBackendClient.js';
import { ResumePickerView } from '../overlays/ResumePicker.js';
import { SearchOverlayView } from '../overlays/SearchOverlayView.js';
import {
  toSearchVM,
  applySearchQueryEdit,
  clampSearchSelection,
} from '../overlays/searchOverlayLogic.js';
import { dialogDecision } from '../overlays/permissionDialogLogic.js';
import { PermissionDialogView } from '../overlays/PermissionDialog.js';
import { writeThemeSetting } from '../../state/themeStore.js';
import type { ThemeSetting } from '../../contracts/theme.js';

/**
 * Top-level live shell: frozen transcript + streaming block + composer.
 * Static/live split via React.memo rows (not ink <Static>) so ctrl+o can
 * re-render past tool cards.
 */
export function AppShell(): React.ReactElement {
  const { columns } = useTerminalSize();
  // One controller per mount; UDS path resolves from BRAIN_SOCKET_PATH.
  const controller = React.useMemo(
    () => new SessionController(new UdsBrainBackendClient()),
    [],
  );
  // Inc 15: unmount must cancel the reconnect loop's timers.
  React.useEffect(() => () => controller.dispose(), [controller]);
  const snapshot = useShellSnapshot(controller);
  const [expandTools, setExpandTools] = React.useState(false);
  const { setting: themeSetting, tokens, setSetting } = useTheme();
  const workspaceLabel = path.basename(process.cwd()).slice(0, 24);
  const [themeOpen, setThemeOpen] = React.useState(false);
  const [themeSelected, setThemeSelected] = React.useState(0);
  const [themeOriginal, setThemeOriginal] = React.useState<ThemeSetting>('auto');
  const [resumeOpen, setResumeOpen] = React.useState(false);
  const [resumeSummaries, setResumeSummaries] = React.useState<BrainSessionSummary[]>([]);
  const [resumeSelected, setResumeSelected] = React.useState(0);
  const [resumeQuery, setResumeQuery] = React.useState('');
  // Command-surface overlays (Inc 21): /doctor and /memory mount points.
  const [doctorOpen, setDoctorOpen] = React.useState(false);
  const [doctorLoading, setDoctorLoading] = React.useState(true); // avoids a one-frame "failed" flash pre-effect
  const [doctorReport, setDoctorReport] = React.useState<EngineDiagnosticReport | null>(null);
  const [memoryOpen, setMemoryOpen] = React.useState(false);
  const [memoryQuery, setMemoryQuery] = React.useState('');
  const [memoryRows, setMemoryRows] = React.useState<RetrievedMemory[]>([]);
  const [memoryState, setMemoryState] = React.useState<'loading' | 'offline' | 'ready'>('loading');
  const [memorySelected, setMemorySelected] = React.useState(0);
  const [memoryExpandedId, setMemoryExpandedId] = React.useState<string | null>(null);
  const memoryToken = React.useRef(0);
  // Interactive search overlay (Inc 34)
  const [searchOpen, setSearchOpen] = React.useState(false);
  const [searchQuery, setSearchQuery] = React.useState('');
  const [searchMatches, setSearchMatches] = React.useState<TranscriptSearchMatch[]>([]);
  const [searchState, setSearchState] = React.useState<'idle' | 'loading' | 'ready'>('idle');
  const [searchSelected, setSearchSelected] = React.useState(0);
  const searchToken = React.useRef(0);
  // Context provenance overlay (Inc 36)
  const [contextOpen, setContextOpen] = React.useState(false);
  // Interactive model picker overlay (Inc 38)
  const [modelOpen, setModelOpen] = React.useState(false);
  const [modelLoading, setModelLoading] = React.useState(false);
  const [modelError, setModelError] = React.useState<string | undefined>(undefined);
  const [modelList, setModelList] = React.useState<BrainModelDescriptor[]>([]);
  const [modelSelected, setModelSelected] = React.useState(0);
  const doctorProbe = React.useMemo(() => new DoctorProbe(), []);
  const [resumeIncludeArchived, setResumeIncludeArchived] = React.useState(false);
  const resumeItems = React.useMemo(
    () =>
      resumeChoices(resumeSummaries, Date.now(), resumeQuery, {
        includeArchived: resumeIncludeArchived,
      }),
    [resumeSummaries, resumeQuery, resumeIncludeArchived],
  );
  const modelItems = React.useMemo(
    () => toModelItemVM(modelList, snapshot.activeModel?.id),
    [modelList, snapshot.activeModel?.id],
  );
  const permission = snapshot.permission;
  const [permSelected, setPermSelected] = React.useState(0);
  React.useEffect(() => {
    setPermSelected(0);
  }, [permission?.callId]);
  React.useEffect(() => {
    setResumeSelected((i) => Math.min(i, Math.max(0, resumeItems.length - 1)));
  }, [resumeItems.length]);
  React.useEffect(() => {
    setModelSelected((i) => Math.min(i, Math.max(0, modelItems.length - 1)));
  }, [modelItems.length]);

  const requestExit = React.useMemo(() => makeUserExit(controller), [controller]);
  useBoundInput({
    contexts: ['global'],
    onAction: (action) => {
      if (action === 'shell:exit') requestExit();
      if (action === 'shell:toggleTools') setExpandTools((v) => !v);
    },
  });

  // Theme picker overlay: navigating previews live (setSetting), esc rolls
  // back to the setting captured at open, enter persists via the store.
  useBoundInput({
    contexts: ['overlay'],
    isActive: themeOpen,
    onAction: (action) => {
      const d = overlayListDecision(action, themeSelected, THEME_CHOICES.length);
      if (d.type === 'move') {
        setThemeSelected(d.index);
        setSetting(THEME_CHOICES[d.index]!.setting); // live preview
      } else if (d.type === 'commit') {
        setThemeOpen(false);
        try {
          writeThemeSetting(THEME_CHOICES[d.index]!.setting);
        } catch {
          controller.notice('Could not save the theme setting.');
        }
      } else if (d.type === 'cancel') {
        setSetting(themeOriginal); // rollback preview
        setThemeOpen(false);
      }
    },
  });

  // Resume picker overlay: same list grammar as /theme, but committing
  // hands the chosen session to the controller for adoption + replay.
  useBoundInput({
    contexts: ['overlay'],
    isActive: resumeOpen,
    onAction: (action, input) => {
      const d = resumeListDecision(action, resumeSelected, resumeItems.length);
      if (d.type === 'move') {
        setResumeSelected(d.index);
      } else if (d.type === 'commit') {
        setResumeOpen(false);
        setResumeIncludeArchived(false);
        const chosen = resumeItems[d.index];
        if (chosen) void controller.resumeSession(chosen.id);
      } else if (d.type === 'cancel') {
        setResumeOpen(false);
        setResumeIncludeArchived(false);
      } else if (action === 'overlay:insert') {
        setResumeQuery((q) => applyQueryEdit(q, action, input));
      } else if (action === 'overlay:backspace') {
        setResumeQuery((q) => applyQueryEdit(q, action, input));
      }
    },
  });

  // /doctor overlay: read-only report — not a navigable list, so the
  // list-decision table doesn't apply; enter/esc dismiss directly.
  useBoundInput({
    contexts: ['overlay'],
    isActive: doctorOpen,
    onAction: (action) => {
      if (action === 'overlay:commit' || action === 'overlay:cancel') {
        setDoctorOpen(false);
        controller.notice('Completed system diagnostics');
      }
    },
  });

  React.useEffect(() => {
    if (!doctorOpen) return;
    let alive = true;
    setDoctorLoading(true);
    setDoctorReport(null);
    void doctorProbe
      .runDiagnostics()
      .then((rep) => {
        if (alive) {
          setDoctorReport(rep);
          setDoctorLoading(false);
        }
      })
      .catch(() => {
        if (alive) setDoctorLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [doctorOpen, doctorProbe]);

  // /context overlay: read-only report; enter/esc dismiss directly.
  useBoundInput({
    contexts: ['overlay'],
    isActive: contextOpen,
    onAction: (action) => {
      if (action === 'overlay:commit' || action === 'overlay:cancel') {
        setContextOpen(false);
        controller.notice('Closed context provenance view');
      }
    },
  });

  // /memory overlay: type-to-filter over the knowledge graph.
  useBoundInput({
    contexts: ['overlay'],
    isActive: memoryOpen,
    onAction: (action, input) => {
      const d = overlayListDecision(action, memorySelected, Math.min(memoryRows.length, 6));
      if (d.type === 'move') {
        setMemoryExpandedId(null);
        setMemorySelected(d.index);
      } else if (d.type === 'commit') {
        const chosen = memoryRows[d.index];
        setMemoryExpandedId((cur) =>
          cur === chosen?.node_id ? null : chosen?.node_id ?? null,
        );
      } else if (d.type === 'cancel') {
        setMemoryOpen(false);
        controller.notice('Closed memory exploration view');
      } else if (action === 'overlay:insert' || action === 'overlay:backspace') {
        setMemoryQuery((q) => applyQueryEdit(q, action, input));
      }
    },
  });

  // Debounced /memory search: keystrokes paint instantly, RPCs fire ≥200ms
  // after the last one; the monotonic token drops stale responses.
  React.useEffect(() => {
    if (!memoryOpen) return;
    const ticket = ++memoryToken.current;
    setMemoryState('loading');
    const timer = setTimeout(() => {
      void controller.searchMemories(memoryQuery, 20).then((r) => {
        if (memoryToken.current !== ticket) return; // stale response dropped
        if (!r.ok) {
          setMemoryState('offline');
          setMemoryRows([]);
        } else {
          setMemoryState('ready');
          setMemoryRows(r.memories);
        }
        setMemorySelected((i) => clampSelection(i, r.ok ? r.memories.length : 0));
      });
    }, 200);
    return () => clearTimeout(timer);
  }, [memoryOpen, memoryQuery, controller]);

  // Permission dialog: dismissal never grants — esc and n both deny.
  useBoundInput({
    contexts: ['dialog'],
    isActive: permission !== undefined,
    onAction: (action) => {
      if (!permission) return;
      const d = dialogDecision(action, permSelected);
      if (d.type === 'move') {
        setPermSelected(d.index);
      } else if (d.type === 'allow') {
        controller.resolvePermission(permission.callId, true);
      } else if (d.type === 'always') {
        controller.resolvePermissionAlways(permission.callId);
      } else if (d.type === 'deny') {
        controller.resolvePermission(permission.callId, false);
      }
    },
  });

  // /search overlay: type-to-search across conversation transcripts.
  useBoundInput({
    contexts: ['overlay'],
    isActive: searchOpen,
    onAction: (action, input) => {
      const d = overlayListDecision(action, searchSelected, Math.min(searchMatches.length, 6));
      if (d.type === 'move') {
        setSearchSelected(d.index);
      } else if (d.type === 'commit') {
        setSearchOpen(false);
        const chosen = searchMatches[d.index];
        let sid = chosen?.sessionId || (chosen?.kind === 'session' ? chosen?.id : undefined) || chosen?.id;
        if (sid && sid.startsWith('session:')) {
          sid = sid.slice('session:'.length);
        }
        if (sid) {
          void controller.resumeSession(sid);
        }
      } else if (d.type === 'cancel') {
        setSearchOpen(false);
        controller.notice('Closed transcript search view');
      } else if (action === 'overlay:insert' || action === 'overlay:backspace') {
        setSearchQuery((q) => applySearchQueryEdit(q, action, input));
      }
    },
  });

  // /model overlay: keyboard navigation across registered models
  useBoundInput({
    contexts: ['overlay'],
    isActive: modelOpen,
    onAction: (action) => {
      const d = modelListDecision(action, modelSelected, modelItems.length);
      setModelSelected(d.nextSelected);
      if (d.committed) {
        setModelOpen(false);
        const chosen = modelItems[d.nextSelected];
        if (chosen) {
          void controller.switchModel(chosen.id);
        }
      } else if (d.cancelled) {
        setModelOpen(false);
        controller.notice('Closed model selection view');
      }
    },
  });

  // Debounced /search transcript search: keystrokes paint instantly, RPCs fire ≥150ms
  // after the last one; the monotonic token drops stale responses.
  React.useEffect(() => {
    if (!searchOpen) return;
    const trimmed = searchQuery.trim();
    if (!trimmed) {
      setSearchState('idle');
      setSearchMatches([]);
      setSearchSelected(0);
      return;
    }
    const ticket = ++searchToken.current;
    setSearchState('loading');
    const timer = setTimeout(() => {
      void controller.searchTranscripts(trimmed, 20).then((matches) => {
        if (searchToken.current !== ticket) return; // stale response dropped
        setSearchState('ready');
        setSearchMatches(matches);
        setSearchSelected((i) => clampSearchSelection(i, matches.length));
      });
    }, 150);
    return () => clearTimeout(timer);
  }, [searchOpen, searchQuery, controller]);

  const runCommand = (rawValue: string): void => {
    const words = rawValue.trim().slice(1).split(/\s+/); // strip '/', split args
    const token = (words[0] ?? '').toLowerCase();
    const args = words.slice(1);
    if (token.length === 0) return;
    let chosen = getCommand(token);
    if (chosen === undefined) {
      const prefixHits = getCommands().filter((c) => c.name.startsWith(token));
      if (prefixHits.length === 1) chosen = prefixHits[0];
    }
    if (chosen === undefined) {
      controller.notice(`Unknown command: /${token}`);
      return;
    }
    const res = chosen.run({ args });
    switch (res.type) {
      case 'text':
        controller.notice(res.value);
        break;
      case 'action':
        if (res.action === 'quit') {
          const plan = planQuit(snapshot.busy);
          if (plan.action === 'interrupt') {
            controller.interruptTurn();
            setTimeout(() => {
              controller.notice(plan.notice ?? 'Session interrupted. Press /quit again to exit.');
            }, 50);
          } else {
            process.exit(0);
          }
        } else if (res.action === 'clear') controller.clear();
        else if (res.action === 'theme') {
          setThemeOriginal(themeSetting);
          setThemeSelected(Math.max(0, THEME_CHOICES.findIndex((c) => c.setting === themeSetting)));
          setThemeOpen(true);
        } else if (res.action === 'resume' || res.action === 'resume-all') {
          if (snapshot.busy) {
            controller.notice('Busy — wait for the current turn to finish.');
            return;
          }
          const isAll = res.action === 'resume-all';
          const target = res.args && res.args.length > 0 ? res.args.join(' ') : '';
          void controller.listSessions().then((all) => {
            if (all.length === 0) {
              controller.notice('No previous sessions found.');
              return;
            }
            setResumeSummaries(all);
            setResumeIncludeArchived(isAll);
            if (target) {
              const resolution = resolveDirectResumeTarget(all, target, { includeArchived: isAll });
              if (resolution.type === 'exact' || resolution.type === 'unique') {
                void controller.resumeSession(resolution.sessionId);
                return;
              }
              if (resolution.type === 'ambiguous') {
                setResumeQuery(resolution.query);
                setResumeSelected(0);
                setResumeOpen(true);
                return;
              }
              controller.notice(`No session found matching “${target}”.`);
              return;
            }
            if (resumeChoices(all, Date.now(), '', { includeArchived: isAll }).length === 0) {
              controller.notice('No previous sessions found.');
              return;
            }
            setResumeQuery('');
            setResumeSelected(0);
            setResumeOpen(true);
          });
        } else if (res.action === 'rename') {
          const title = res.args ? res.args.join(' ') : '';
          void controller.renameCurrentSession(title);
        } else if (res.action === 'fork') {
          const title = res.args && res.args.length > 0 ? res.args.join(' ') : undefined;
          void controller.forkCurrentSession(title);
        } else if (res.action === 'session-delete') {
          const target = res.args && res.args.length > 0 ? res.args.join(' ') : '';
          if (target) void controller.deleteSession(target);
        } else if (res.action === 'session-pin') {
          const target = res.args && res.args.length > 0 ? res.args.join(' ') : undefined;
          void controller.pinSession(target, true);
        } else if (res.action === 'session-unpin') {
          const target = res.args && res.args.length > 0 ? res.args.join(' ') : undefined;
          void controller.pinSession(target, false);
        } else if (res.action === 'session-archive') {
          const target = res.args && res.args.length > 0 ? res.args.join(' ') : undefined;
          void controller.archiveSession(target);
        } else if (res.action === 'session-restore') {
          const target = res.args ? res.args.join(' ') : '';
          if (target) void controller.restoreSession(target);
        } else if (res.action === 'memory-delete') {
          const targetId = res.args ? res.args[0] : '';
          if (targetId) void controller.deleteMemoryNode(targetId);
        } else if (res.action === 'memory-add') {
          const label = res.args?.[0] ?? '';
          const content = res.args?.[1] ?? label;
          if (label) void controller.storeMemoryNode(label, content);
        } else if (res.action === 'memory-link') {
          const source = res.args?.[0] ?? '';
          const target = res.args?.[1] ?? '';
          const relation = res.args?.[2];
          if (source && target) void controller.linkMemories(source, target, relation);
        } else if (res.action === 'memory-unlink') {
          const source = res.args?.[0] ?? '';
          const target = res.args?.[1] ?? '';
          const relation = res.args?.[2];
          if (source && target) void controller.unlinkMemories(source, target, relation);
        } else if (res.action === 'memory-consolidate') {
          void controller.consolidateMemories();
        } else if (res.action === 'search') {
          const query = res.args ? res.args.join(' ') : '';
          setSearchQuery(query);
          setSearchMatches([]);
          setSearchSelected(0);
          setSearchOpen(true);
        } else if (res.action === 'model-switch') {
          const query = res.args && res.args.length > 0 ? res.args.join(' ') : undefined;
          void controller.switchModel(query);
        } else if (res.action === 'thinking-set') {
          const arg = res.args?.[0];
          controller.setThinkingBudget(arg === 'off' ? null : Number(arg));
        } else if (res.action === 'thinking-status') {
          controller.reportThinkingStatus();
        }
        break;
      case 'overlay':
        if (res.overlay === 'doctor') setDoctorOpen(true);
        else if (res.overlay === 'context') setContextOpen(true);
        else if (res.overlay === 'model') {
          setModelOpen(true);
          setModelLoading(true);
          setModelError(undefined);
          setModelSelected(0);
          void controller
            .listAvailableModels()
            .then((models) => {
              setModelList(models);
              setModelLoading(false);
              if (models.length > 0) {
                const activeIdx = models.findIndex((m) =>
                  snapshot.activeModel ? m.id === snapshot.activeModel.id : m.isDefault,
                );
                setModelSelected(Math.max(0, activeIdx));
              }
            })
            .catch((err) => {
              setModelError(err instanceof Error ? err.message : String(err));
              setModelLoading(false);
            });
        } else {
          setMemoryQuery('');
          setMemoryRows([]);
          setMemorySelected(0);
          setMemoryExpandedId(null);
          setMemoryOpen(true);
        }
        break;
    }
  };

  const handleSubmit = (text: string, mode: 'prompt' | 'bash' = 'prompt'): void => {
    if (mode === 'bash') void controller.runShellCommand(text);
    else if (text.trimStart().startsWith('/')) runCommand(text);
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
      {snapshot.connection.status !== 'connected' ? (
        <Text color="yellow">⚠ Connection lost — reconnecting…</Text>
      ) : snapshot.connectionError !== undefined ? (
        <Text color="red">⚠ {snapshot.connectionError}</Text>
      ) : null}
      {themeOpen ? (
        <Box marginTop={1}>
          <ThemePickerView
            choices={THEME_CHOICES}
            selectedIndex={themeSelected}
            current={themeSetting}
            tokens={tokens}
          />
        </Box>
      ) : null}
      {resumeOpen ? (
        <Box marginTop={1}>
          <ResumePickerView
            items={resumeItems}
            selectedIndex={resumeSelected}
            tokens={tokens}
            query={resumeQuery}
            currentSessionId={controller.activeSessionId}
          />
        </Box>
      ) : null}
      {permission ? (
        <Box marginTop={1}>
          <PermissionDialogView req={permission} selected={permSelected} tokens={tokens} />
        </Box>
      ) : null}
      {doctorOpen ? (
        <Box marginTop={1}>
          <DoctorOverlayView loading={doctorLoading} report={doctorReport} tokens={tokens} />
        </Box>
      ) : null}
      {contextOpen ? (
        <Box marginTop={1}>
          <ContextOverlayView
            telemetry={controller.latestContextTelemetry}
            activeSessionId={controller.activeSessionId}
            tokens={tokens}
          />
        </Box>
      ) : null}
      {memoryOpen ? (
        <Box marginTop={1}>
          <MemoryOverlayView
            query={memoryQuery}
            state={memoryState}
            rows={memoryRows}
            selectedIndex={memorySelected}
            expandedId={memoryExpandedId}
            tokens={tokens}
          />
        </Box>
      ) : null}
      {searchOpen ? (
        <Box marginTop={1}>
          <SearchOverlayView
            query={searchQuery}
            state={searchState}
            matches={searchMatches.map(toSearchVM)}
            selectedIndex={searchSelected}
            tokens={tokens}
            currentSessionId={controller.activeSessionId}
          />
        </Box>
      ) : null}
      {modelOpen ? (
        <Box marginTop={1}>
          <ModelPickerView
            loading={modelLoading}
            error={modelError}
            models={modelItems}
            selectedIndex={modelSelected}
            tokens={tokens}
          />
        </Box>
      ) : null}
      <Box marginTop={1}>
        <PromptInput
          disabled={false}
          busy={snapshot.busy}
          paused={
            themeOpen ||
            resumeOpen ||
            permission !== undefined ||
            doctorOpen ||
            contextOpen ||
            memoryOpen ||
            searchOpen ||
            modelOpen
          }
          onSubmit={handleSubmit}
          onAbort={() => controller.interruptTurn()}
          onRequestExit={requestExit}
        />
      </Box>
      <StatusBarView
        model={snapshot.activeModelName}
        workspace={workspaceLabel}
        theme={themeSetting}
        expandTools={expandTools}
        tokens={tokens}
        thinkingBudget={snapshot.thinkingBudget}
        supportsThinking={snapshot.activeModel?.supportsThinking ?? true}
        connectionText={connectionStatusText(snapshot.connection)}
      />
    </Box>
  );
}
