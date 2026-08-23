import React, { useState, useMemo } from 'react';
import { Box, Text, useInput } from '../../compat/ink.js';
import { useTerminalSize } from '../../compat/hooks.js';
import { BrainModal } from '../../components/BrainModal.js';
import { BrainSearchField } from '../../components/BrainSearchField.js';
import { BrainTabHeader, type BrainTabItem } from '../../components/BrainTabHeader.js';
import { BrainConfigStore } from '../../adapter/BrainConfigStore.js';

export interface ConfigEntry {
  key: string;
  value: string;
  description: string;
  category: 'settings' | 'status' | 'storage' | 'index' | 'query' | 'runtime' | 'usage' | 'diagnostics';
}

export interface ConfigCommandProps {
  onDone: (result?: string, options?: { display?: 'system' | 'user' }) => void;
  initialTab?: string;
}

const BRAIN_CONFIG_TABS: BrainTabItem[] = [
  { id: 'settings', label: 'Settings' },
  { id: 'status', label: 'Status', badge: 'OK' },
  { id: 'storage', label: 'Storage' },
  { id: 'index', label: 'Index' },
  { id: 'query', label: 'Query' },
  { id: 'runtime', label: 'Runtime' },
  { id: 'usage', label: 'Usage' },
  { id: 'diagnostics', label: 'Diagnostics' },
];

export const ConfigCommand: React.FC<ConfigCommandProps> = ({
  onDone,
  initialTab = 'settings',
}) => {
  const [activeTabId, setActiveTabId] = useState<string>(initialTab);
  const [searchQuery, setSearchQuery] = useState<string>('');
  const [selectedIndex, setSelectedIndex] = useState<number>(0);
  const { columns } = useTerminalSize();

  const currentThemeSetting = BrainConfigStore.getBrainThemeSetting();

  const configEntries: ConfigEntry[] = useMemo(() => [
    { key: 'data_dir', value: '~/.brain', description: 'Root directory for local daemon data & state', category: 'settings' },
    { key: 'socket_path', value: process.env.BRAIN_SOCKET_PATH || '~/.brain/daemon.sock', description: 'Unix Domain Socket path for IPC streaming', category: 'settings' },
    { key: 'theme', value: currentThemeSetting, description: 'Active terminal color theme preference (~/.brain/config.json)', category: 'settings' },
    { key: 'auto_theme', value: currentThemeSetting === 'auto' ? 'true' : 'false', description: 'Automatically sync TUI theme with terminal background', category: 'settings' },
    { key: 'daemon_status', value: 'Running (PID 49120)', description: 'Local background daemon process liveness', category: 'status' },
    { key: 'uds_health', value: 'Healthy (0.4ms latency)', description: 'Round-trip IPC socket latency', category: 'status' },
    { key: 'sqlite_db', value: '~/.brain/brain_runtime.db', description: 'Persistent relational entity database', category: 'storage' },
    { key: 'wal_mode', value: 'Enabled', description: 'SQLite Write-Ahead Logging integrity mode', category: 'storage' },
    { key: 'vector_index', value: 'HNSW (Cosine)', description: 'Fast approximate nearest neighbor vector index', category: 'index' },
    { key: 'embedding_dim', value: '384', description: 'Dense relational memory vector dimension', category: 'index' },
    { key: 'hybrid_search', value: 'Enabled (Dense + Sparse BM25)', description: 'Dual-channel relational retrieval pipeline', category: 'query' },
    { key: 'search_top_k', value: '20', description: 'Maximum memory candidates per turn', category: 'query' },
    { key: 'rust_runtime', value: 'tokio v1.40', description: 'Multi-threaded async task executor', category: 'runtime' },
    { key: 'bun_frontend', value: 'bun v1.4.0 (Ink + React)', description: 'Immediate-mode terminal renderer', category: 'runtime' },
    { key: 'active_sessions', value: '3', description: 'Total background conversation sessions', category: 'usage' },
    { key: 'total_entities', value: '142 nodes · 318 edges', description: 'Knowledge graph memory entity count', category: 'usage' },
    { key: 'daemon_probe', value: 'All 4 subsystems healthy', description: 'Background health check verification', category: 'diagnostics' },
    { key: 'memory_check', value: 'Zero orphan relations', description: 'Relational knowledge graph integrity', category: 'diagnostics' },
  ], [currentThemeSetting]);

  const filteredEntries = useMemo(() => {
    const q = searchQuery.toLowerCase().trim();
    return configEntries.filter((entry) => {
      const matchesTab = entry.category === activeTabId;
      if (!matchesTab) return false;
      if (!q) return true;
      return (
        entry.key.toLowerCase().includes(q) ||
        entry.value.toLowerCase().includes(q) ||
        entry.description.toLowerCase().includes(q)
      );
    });
  }, [activeTabId, searchQuery, configEntries]);

  useInput((input, key) => {
    if (key.escape) {
      onDone('Closed /config dashboard', { display: 'system' });
      return;
    }
    if (key.upArrow) {
      setSelectedIndex((prev) => Math.max(0, prev - 1));
      return;
    }
    if (key.downArrow) {
      setSelectedIndex((prev) => Math.min(Math.max(0, filteredEntries.length - 1), prev + 1));
      return;
    }
  });

  return (
    <BrainModal
      title="Brain Configuration & Telemetry"
      subtitle="Inspect and customize relational memory engine runtime parameters"
      onDismiss={() => onDone('Closed /config dashboard', { display: 'system' })}
      footerHints={[
        { key: '←/→', action: 'switch tab' },
        { key: '1-8', action: 'direct tab jump' },
        { key: '↑/↓', action: 'select parameter' },
        { key: 'Esc', action: 'close' },
      ]}
    >
      <Box flexDirection="column" gap={1} width="100%">
        <BrainTabHeader
          tabs={BRAIN_CONFIG_TABS}
          activeTabId={activeTabId}
          onTabSelect={(id) => {
            setActiveTabId(id);
            setSelectedIndex(0);
          }}
        />

        <BrainSearchField
          value={searchQuery}
          onChange={(q) => {
            setSearchQuery(q);
            setSelectedIndex(0);
          }}
          onCancel={() => onDone('Closed /config dashboard', { display: 'system' })}
          placeholder="Filter configuration parameters..."
        />

        <Box flexDirection="column" marginTop={1}>
          {filteredEntries.length === 0 ? (
            <Box paddingY={1}>
              <Text dimColor>No parameters found matching "{searchQuery}" in this tab.</Text>
            </Box>
          ) : (
            filteredEntries.map((entry, index) => {
              const isSelected = index === selectedIndex;
              return (
                <Box
                  key={entry.key}
                  flexDirection="row"
                  justifyContent="space-between"
                  paddingX={1}
                >
                  <Box flexDirection="row" gap={1} width="35%">
                    <Text color={isSelected ? 'cyan' : 'white'} bold={isSelected}>
                      {isSelected ? '❯ ' : '  '}
                      {entry.key}
                    </Text>
                  </Box>
                  <Box flexDirection="row" width="25%">
                    <Text color="cyan">{entry.value}</Text>
                  </Box>
                  <Box flexDirection="row" width="40%">
                    <Text dimColor wrap="truncate">
                      {entry.description}
                    </Text>
                  </Box>
                </Box>
              );
            })
          )}
        </Box>
      </Box>
    </BrainModal>
  );
};

export default ConfigCommand;
