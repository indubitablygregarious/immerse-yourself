import { useState, useEffect, useRef, useCallback, type FC } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface DebugLogPanelProps {
  onClose: () => void;
}

export const DebugLogPanel: FC<DebugLogPanelProps> = ({ onClose }) => {
  const [lines, setLines] = useState<string[]>([]);
  const [autoScroll, setAutoScroll] = useState(true);
  const [copyFeedback, setCopyFeedback] = useState(false);
  const preRef = useRef<HTMLPreElement>(null);

  const fetchLog = useCallback(async () => {
    try {
      const log = await invoke<string[]>('get_debug_log');
      setLines(log);
    } catch (e) {
      console.error('Failed to fetch debug log:', e);
    }
  }, []);

  const handleClear = useCallback(async () => {
    try {
      await invoke('clear_debug_log');
      setLines([]);
    } catch (e) {
      console.error('Failed to clear debug log:', e);
    }
  }, []);

  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(lines.join('\n'));
      setCopyFeedback(true);
      setTimeout(() => setCopyFeedback(false), 2000);
    } catch {
      // Fallback for environments where clipboard API isn't available
      const textarea = document.createElement('textarea');
      textarea.value = lines.join('\n');
      textarea.style.position = 'fixed';
      textarea.style.opacity = '0';
      document.body.appendChild(textarea);
      textarea.select();
      document.execCommand('copy');
      document.body.removeChild(textarea);
      setCopyFeedback(true);
      setTimeout(() => setCopyFeedback(false), 2000);
    }
  }, [lines]);

  // Initial fetch + auto-refresh every 2 seconds
  useEffect(() => {
    fetchLog();
    const interval = setInterval(fetchLog, 2000);
    return () => clearInterval(interval);
  }, [fetchLog]);

  // Auto-scroll to bottom when new lines arrive
  useEffect(() => {
    if (autoScroll && preRef.current) {
      preRef.current.scrollTop = preRef.current.scrollHeight;
    }
  }, [lines, autoScroll]);

  // Escape to close
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [onClose]);

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div
        className="dialog-content debug-log-dialog"
        onClick={e => e.stopPropagation()}
      >
        <div className="debug-log-header">
          <h2>Debug Log</h2>
          <button className="debug-log-done-button" onClick={onClose}>Done</button>
        </div>

        <pre className="debug-log-content" ref={preRef}>
          {lines.length === 0 ? '(no log entries yet)' : lines.join('\n')}
        </pre>

        <div className="debug-log-footer">
          <span className="debug-log-count">{lines.length} entries</span>
          <label className="debug-log-autoscroll">
            <input
              type="checkbox"
              checked={autoScroll}
              onChange={e => setAutoScroll(e.target.checked)}
            />
            Auto-scroll
          </label>
          <div className="debug-log-actions">
            <button className="settings-button-small" onClick={fetchLog}>Refresh</button>
            <button className="settings-button-small" onClick={handleCopy}>
              {copyFeedback ? 'Copied!' : 'Copy'}
            </button>
            <button className="settings-button-small" onClick={handleClear}>Clear</button>
          </div>
        </div>
      </div>
    </div>
  );
};
