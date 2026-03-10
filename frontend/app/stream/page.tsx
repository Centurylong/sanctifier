"use client";

import { useState, useCallback } from "react";
import { LogViewer } from "../components/LogViewer";
import { transformReport } from "../lib/transform";
import { FindingsList } from "../components/FindingsList";
import { SummaryChart } from "../components/SummaryChart";
import { ThemeToggle } from "../components/ThemeToggle";
import type { AnalysisReport, Finding, Severity } from "../types";
import Link from "next/link";

export default function StreamPage() {
  const [wsUrl, setWsUrl] = useState("ws://localhost:9001");
  const [isStreaming, setIsStreaming] = useState(false);
  const [findings, setFindings] = useState<Finding[]>([]);
  const [reportData, setReportData] = useState<AnalysisReport | null>(null);
  const [severityFilter, setSeverityFilter] = useState<Severity | "all">("all");

  const handleStartStream = useCallback(() => {
    setIsStreaming(true);
    setFindings([]);
    setReportData(null);
  }, []);

  const handleReport = useCallback((report: unknown) => {
    try {
      const parsed = report as AnalysisReport;
      setReportData(parsed);
      setFindings(transformReport(parsed));
    } catch (err) {
      console.error("Failed to parse report:", err);
    }
  }, []);

  const filteredFindings =
    severityFilter === "all"
      ? findings
      : findings.filter((f) => f.severity === severityFilter);

  return (
    <div className="min-h-screen bg-zinc-50 dark:bg-zinc-950 text-zinc-900 dark:text-zinc-100">
      <header className="border-b border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900 px-6 py-4 flex items-center justify-between">
        <div className="flex items-center gap-6">
          <Link href="/" className="font-bold text-lg">
            Sanctifier
          </Link>
          <span className="text-zinc-500 dark:text-zinc-400">Real-time Analysis Stream</span>
        </div>
        <ThemeToggle />
      </header>

      <div className="container mx-auto p-6">
        <div className="mb-6">
          <h1 className="text-3xl font-bold mb-2">Real-time Analysis Log Streamer</h1>
          <p className="text-zinc-600 dark:text-zinc-400">
            Watch your contract analysis progress in real-time via WebSocket
          </p>
        </div>

        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6 mb-6">
          <div className="bg-white dark:bg-zinc-900 rounded-lg border border-zinc-200 dark:border-zinc-800 p-6">
            <h2 className="text-xl font-semibold mb-4">Connection Settings</h2>
            <div className="space-y-4">
              <div>
                <label className="block text-sm font-medium mb-2">WebSocket URL</label>
                <input
                  type="text"
                  value={wsUrl}
                  onChange={(e) => setWsUrl(e.target.value)}
                  disabled={isStreaming}
                  className="w-full px-3 py-2 border border-zinc-300 dark:border-zinc-700 rounded bg-white dark:bg-zinc-800 disabled:opacity-50"
                  placeholder="ws://localhost:9001"
                />
              </div>
              <button
                onClick={handleStartStream}
                disabled={isStreaming}
                className="w-full px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-zinc-400 text-white rounded font-medium"
              >
                {isStreaming ? "Streaming..." : "Start Streaming"}
              </button>
              <div className="text-sm text-zinc-600 dark:text-zinc-400 space-y-1">
                <p>To start the analysis server, run:</p>
                <code className="block bg-zinc-100 dark:bg-zinc-800 p-2 rounded">
                  sanctifier stream --port 9001
                </code>
              </div>
            </div>
          </div>

          <div className="bg-white dark:bg-zinc-900 rounded-lg border border-zinc-200 dark:border-zinc-800 p-6">
            <h2 className="text-xl font-semibold mb-4">Analysis Summary</h2>
            {reportData ? (
              <SummaryChart findings={findings} />
            ) : (
              <div className="text-zinc-400 dark:text-zinc-600 text-center py-8">
                No analysis data yet
              </div>
            )}
          </div>
        </div>

        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          <div className="bg-white dark:bg-zinc-900 rounded-lg border border-zinc-200 dark:border-zinc-800 overflow-hidden">
            <div className="px-6 py-4 border-b border-zinc-200 dark:border-zinc-800">
              <h2 className="text-xl font-semibold">Live Logs</h2>
            </div>
            <div className="h-[600px]">
              {isStreaming ? (
                <LogViewer wsUrl={wsUrl} onReport={handleReport} />
              ) : (
                <div className="flex items-center justify-center h-full text-zinc-400 dark:text-zinc-600">
                  Click "Start Streaming" to begin
                </div>
              )}
            </div>
          </div>

          <div className="bg-white dark:bg-zinc-900 rounded-lg border border-zinc-200 dark:border-zinc-800 overflow-hidden">
            <div className="px-6 py-4 border-b border-zinc-200 dark:border-zinc-800 flex items-center justify-between">
              <h2 className="text-xl font-semibold">Findings</h2>
              {findings.length > 0 && (
                <select
                  value={severityFilter}
                  onChange={(e) => setSeverityFilter(e.target.value as Severity | "all")}
                  className="px-3 py-1 border border-zinc-300 dark:border-zinc-700 rounded bg-white dark:bg-zinc-800 text-sm"
                >
                  <option value="all">All ({findings.length})</option>
                  <option value="critical">
                    Critical ({findings.filter((f) => f.severity === "critical").length})
                  </option>
                  <option value="high">
                    High ({findings.filter((f) => f.severity === "high").length})
                  </option>
                  <option value="medium">
                    Medium ({findings.filter((f) => f.severity === "medium").length})
                  </option>
                  <option value="low">
                    Low ({findings.filter((f) => f.severity === "low").length})
                  </option>
                </select>
              )}
            </div>
            <div className="h-[600px] overflow-y-auto">
              {filteredFindings.length > 0 ? (
                <FindingsList findings={filteredFindings} />
              ) : (
                <div className="flex items-center justify-center h-full text-zinc-400 dark:text-zinc-600">
                  {findings.length > 0
                    ? "No findings match the selected filter"
                    : "No findings yet"}
                </div>
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
