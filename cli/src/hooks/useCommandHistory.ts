import { useState, useEffect } from 'react';
import { HistoryStore } from '../services/HistoryStore';

export const useCommandHistory = (initialValue: string = '') => {
  const [value, setValue] = useState(initialValue);
  const [history, setHistory] = useState<string[]>([]);
  const [historyIndex, setHistoryIndex] = useState(-1);
  const [draft, setDraft] = useState('');

  useEffect(() => {
    setHistory(HistoryStore.getHistory());
  }, []);

  const appendHistory = (cmd: string) => {
    HistoryStore.append(cmd);
    setHistory(HistoryStore.getHistory());
    setHistoryIndex(-1);
    setDraft('');
  };

  const handleArrowKeys = (key: { upArrow: boolean; downArrow: boolean }): boolean => {
    if (key.upArrow) {
      if (history.length === 0) return false;
      let nextIndex = historyIndex === -1 ? history.length - 1 : historyIndex - 1;
      if (nextIndex < 0) {
        nextIndex = 0; // clamp to oldest entry
      }
      if (historyIndex === -1) {
        setDraft(value); // save draft
      }
      setHistoryIndex(nextIndex);
      setValue(history[nextIndex]);
      return true;
    } else if (key.downArrow) {
      if (historyIndex === -1) return false;
      const nextIndex = historyIndex + 1;
      if (nextIndex >= history.length) {
        setHistoryIndex(-1);
        setValue(draft); // restore draft
      } else {
        setHistoryIndex(nextIndex);
        setValue(history[nextIndex]);
      }
      return true;
    }
    return false;
  };

  const resetHistoryIndex = () => {
    setHistoryIndex(-1);
    setDraft('');
  };

  return {
    value,
    setValue,
    history,
    appendHistory,
    handleArrowKeys,
    resetHistoryIndex,
  };
};
