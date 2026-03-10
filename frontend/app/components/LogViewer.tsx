"use client";

import { useEffect, useRef, useState } from "react";

export interface LogEntry {
  type: "info" | "progress" | "warning" | "error" | "complete" | "file_analysis";
  message?: string;
  current?: number;
  total?: number;
  file?: string;
  status?: string;
  timestamp: number;
}

interface LogViewerProps {
  wsUrl: string;
  onReport?: (report: unknown) => void;
}

export function LogViewer({ wsUrl, onReport }: LogViewerProps) {
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [connected, setConnected] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const wsRef = useRef<WebSocket | null>(null);
  const logsEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const ws = new WebSocket(wsUrl);
    wsRef.current = ws;

    ws.onopen = () => {
      setConnected(true);
      setError(null);
    };

    ws.onmessage = (event) => {
      try {
        const logEntry = JSON.parse(event.data) as LogEntry;
        
        // Check if this is a report message
        if (logEntry.type === "info" && logEntry.message?.startsWith("Report:")) {
          try {
            const reportJson = logEntry.message.substring(8);
            const report = JSON.parse(reportJson);
            onReport?.(report);
          } catch {
            // Not a valid report, just add as log
          }
        }
        
        setLogs((prev) => [...prev, logEntry]);
      } catch (err) {
        console.error("Failed to parse log message:", err);
      }
    };

    ws.onerror = () => {
      setError("WebSocket connection error");
      setConnected(false);
    };

    ws.onclose = () => {
      setConnected(false);
    };

    return () => {
      ws.close();
    };
  }, [wsUrl, onReport]);

  useEffect(() => {
    logsEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [logs]);

  const getLogIcon = (type: LogEntry["type"]) => {
    switch (type) {
      case "info":
        return "ℹ️";
      case "progress":
        return "⏳";
      case "warning":
        return "⚠️";
      case "error":
        return "❌";
      case "complete":
        return "✅";
      case "file_analysis":
        return "📄";
      default:
        return "•";
    }
  };

  const getLogColor = (type: LogEntry["type"]) => {
    switch (type) {
      case "info":
        return "text-blue-600 dark:text-blue-400";
      case "progress":
        return "text-purple-600 dark:text-purple-400";
      case "warning":
        return "text-yellow-600 dark:text-yellow-400";
      case "error":
        return "text-red-600 dark:text-red-400";
      case "complete":
        return "text-green-600 dark:text-green-400";
      case "file_analysis":
        return "text-zinc-600 dark:text-zinc-400";
      default:
        return "text-zinc-900 dark:text-zinc-100";
    }
  };

  const formatTimestamp = (timestamp: number) => {
    const date = new Date(timestamp);
    return date.toLocaleTimeString();
  };

  return (
    <div className="flex flex-col h-full">
      <div className="flex items-center justify-between px-4 py-2 border-b border-zinc-200 dark:border-zinc-800 bg-zinc-50 dark:bg-zinc-900">
        <div className="flex items-center gap-2">
          <div
            className={`w-2 h-2 rounded-full ${
              connected ? "bg-green-500" : "bg-red-500"
            }`}
          />
          <span className="text-sm font-medium">
            {connected ? "Connected" : "Disconnected"}
          </span>
        </div>
        <button
          onClick={() => setLogs([])}
          className="text-sm px-3 py-1 rounded bg-zinc-200 dark:bg-zinc-800 hover:bg-zinc-300 dark:hover:bg-zinc-700"
        >
          Clear
        </button>
      </div>

      {error && (
        <div className="px-4 py-2 bg-red-50 dark:bg-red-900/20 text-red-600 dark:text-red-400 text-sm">
          {error}
        </div>
      )}

      <div className="flex-1 overflow-y-auto p-4 space-y-1 font-mono text-sm bg-white dark:bg-zinc-950">
        {logs.length === 0 && (
          <div className="text-zinc-400 dark:text-zinc-600 text-center py-8">
            Waiting for analysis logs...
          </div>
        )}
        {logs.map((log, idx) => (
          <div key={idx} className={`flex items-start gap-2 ${getLogColor(log.type)}`}>
            <span className="flex-shrink-0">{getLogIcon(log.type)}</span>
            <span className="flex-shrink-0 text-zinc-400 dark:text-zinc-600 text-xs">
              {formatTimestamp(log.timestamp)}
            </span>
            <div className="flex-1">
              {log.type === "progress" && (
                <div>
                  <div>{log.message}</div>
                  <div className="text-xs text-zinc-500 dark:text-zinc-500">
                    Progress: {log.current}/{log.total} (
                    {Math.round(((log.current || 0) / (log.total || 1)) * 100)}%)
                  </div>
                </div>
              )}
              {log.type === "file_analysis" && (
                <div>
                  <span className="text-zinc-500 dark:text-zinc-500">{log.status}:</span>{" "}
                  {log.file}
                </div>
              )}
              {log.type !== "progress" && log.type !== "file_analysis" && (
                <div>{log.message}</div>
              )}
            </div>
          </div>
        ))}
        <div ref={logsEndRef} />
      </div>
    </div>
  );
}
