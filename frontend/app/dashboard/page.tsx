"use client";

import { useState, useCallback } from "react";
import type { AnalysisReport, Finding, Severity } from "../types";
import { transformReport } from "../lib/transform";
import { exportToPdf } from "../lib/export-pdf";
import { SeverityFilter } from "../components/SeverityFilter";
import { FindingsList } from "../components/FindingsList";
import { SummaryChart } from "../components/SummaryChart";
import { KaniMetricsWidget } from "../components/KaniMetricsWidget";
import { SymbolicGraphWidget } from "../components/SymbolicGraphWidget";
import { ThemeToggle } from "../components/ThemeToggle";
import { SanctityScoreWidget } from "../components/SanctityScoreWidget";
import Link from "next/link";

import { analyzeSourceInBrowser } from "../lib/wasm";

const SAMPLE_JSON = `{
  "size_warnings": [],
  "unsafe_patterns": [],
  "auth_gaps": [],
  "panic_issues": [],
  "arithmetic_issues": []
}`;

export default function DashboardPage() {
  const [findings, setFindings] = useState<Finding[]>([]);
  const [severityFilter, setSeverityFilter] = useState<Severity | "all">("all");
  const [error, setError] = useState<string | null>(null);
  const [jsonInput, setJsonInput] = useState("");
  const [reportData, setReportData] = useState<AnalysisReport | null>(null);
  const [rustSource, setRustSource] = useState<string>("");
  const [wasmBusy, setWasmBusy] = useState(false);

  const loadReport = useCallback(() => {
    setError(null);
    try {
      const parsed = JSON.parse(jsonInput || SAMPLE_JSON) as AnalysisReport;
      setFindings(transformReport(parsed));
      setReportData(parsed);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Invalid JSON");
      setFindings([]);
      setReportData(null);
    }
  }, [jsonInput]);

  const handleFileUpload = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = (ev) => {
      const text = ev.target?.result as string;
      setJsonInput(text);
      setError(null);
      try {
        const parsed = JSON.parse(text) as AnalysisReport;
        setFindings(transformReport(parsed));
        setReportData(parsed);
      } catch (err) {
        setError(err instanceof Error ? err.message : "Invalid JSON");
        setReportData(null);
      }
    };
    reader.readAsText(file);
    e.target.value = "";
  }, []);

  const runWasmAnalysis = useCallback(async () => {
    setError(null);
    setWasmBusy(true);
    try {
      const report = await analyzeSourceInBrowser(rustSource);
      setReportData(report);
      setFindings(transformReport(report));
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setError(
        `WASM module not found or failed to load. Build it with: wasm-pack build tooling/sanctifier-wasm --release --target web --out-dir frontend/public/wasm. Details: ${msg}`
      );
    } finally {
      setWasmBusy(false);
    }
  }, [rustSource]);

  return (
    <div className="min-h-screen bg-zinc-50 dark:bg-zinc-950 text-zinc-900 dark:text-zinc-100">
      <header className="border-b border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900 px-6 py-4 flex items-center justify-between">
        <div className="flex items-center gap-6">
          <Link href="/" className="font-bold text-lg">
            Sanctifier
          </Link>
          <span className="text-zinc-500 dark:text-zinc-400">Security Dashboard</span>
        </div>
        <ThemeToggle />
      </header>

      <main className="max-w-6xl mx-auto px-6 py-8 space-y-8">
        <section className="rounded-lg border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900 p-6">
          <h2 className="text-lg font-semibold mb-4">Load Analysis Report</h2>
          <p className="text-sm text-zinc-600 dark:text-zinc-400 mb-4">
            Paste JSON from <code className="bg-zinc-100 dark:bg-zinc-800 px-1 rounded">sanctifier analyze --format json</code> or upload a file.
          </p>
          <div className="flex flex-wrap gap-4">
            <label className="cursor-pointer rounded-lg border border-zinc-300 dark:border-zinc-600 px-4 py-2 text-sm hover:bg-zinc-100 dark:hover:bg-zinc-800">
              Upload JSON
              <input
                type="file"
                accept=".json"
                className="hidden"
                onChange={handleFileUpload}
              />
            </label>
            <button
              onClick={loadReport}
              className="rounded-lg bg-zinc-900 dark:bg-zinc-100 text-white dark:text-zinc-900 px-4 py-2 text-sm font-medium hover:bg-zinc-800 dark:hover:bg-zinc-200"
            >
              Parse JSON
            </button>
            <button
              onClick={() => {
                exportToPdf(findings);
              }}
              disabled={findings.length === 0}
              className="rounded-lg border border-zinc-300 dark:border-zinc-600 px-4 py-2 text-sm disabled:opacity-50 hover:bg-zinc-100 dark:hover:bg-zinc-800"
            >
              Export PDF
            </button>
          </div>
          {error && (
            <p className="mt-2 text-sm text-red-600 dark:text-red-400">{error}</p>
          )}
          <textarea
            value={jsonInput}
            onChange={(e) => setJsonInput(e.target.value)}
            placeholder={SAMPLE_JSON}
            className="mt-4 w-full h-32 rounded-lg border border-zinc-300 dark:border-zinc-600 bg-white dark:bg-zinc-950 p-3 font-mono text-sm focus:ring-2 focus:ring-zinc-400 dark:focus:ring-zinc-600 outline-none"
          />
        </section>

        <section className="rounded-lg border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900 p-6">
          <h2 className="text-lg font-semibold mb-4">Analyze Rust Source (Runs in Your Browser)</h2>
          <p className="text-sm text-zinc-600 dark:text-zinc-400 mb-4">
            Paste Soroban contract Rust code and run the Sanctifier engine compiled to WebAssembly locally.
          </p>
          <textarea
            value={rustSource}
            onChange={(e) => setRustSource(e.target.value)}
            placeholder={"// Paste your Soroban contract here"}
            className="mt-2 w-full h-40 rounded-lg border border-zinc-300 dark:border-zinc-600 bg-white dark:bg-zinc-950 p-3 font-mono text-sm focus:ring-2 focus:ring-zinc-400 dark:focus:ring-zinc-600 outline-none"
          />
          <div className="mt-3">
            <button
              onClick={runWasmAnalysis}
              disabled={wasmBusy || rustSource.trim().length === 0}
              className="rounded-lg bg-zinc-900 dark:bg-zinc-100 text-white dark:text-zinc-900 px-4 py-2 text-sm font-medium disabled:opacity-50 hover:bg-zinc-800 dark:hover:bg-zinc-200"
            >
              {wasmBusy ? "Analyzing…" : "Run in Browser (WASM)"}
            </button>
          </div>
        </section>

        {(findings.length > 0 || reportData?.kani_metrics || (reportData?.symbolic_paths && reportData.symbolic_paths.length > 0) || reportData?.sanctity_score) && (
          <>
            {reportData?.sanctity_score && (
              <section>
                <SanctityScoreWidget score={reportData.sanctity_score} />
              </section>
            )}

            {reportData?.kani_metrics && (
              <section>
                <KaniMetricsWidget metrics={reportData.kani_metrics} />
              </section>
            )}


            {reportData?.symbolic_paths && reportData.symbolic_paths.length > 0 && (
              <section>
                <SymbolicGraphWidget graphs={reportData.symbolic_paths} />
              </section>
            )}

            {findings.length > 0 && (
              <section>
                <SummaryChart findings={findings} />
              </section>
            )}

            <section>
              <h2 className="text-lg font-semibold mb-4">Filter by Severity</h2>
              <SeverityFilter selected={severityFilter} onChange={setSeverityFilter} />
            </section>

            <section>
              <h2 className="text-lg font-semibold mb-4">Findings</h2>
              <FindingsList findings={findings} severityFilter={severityFilter} />
            </section>
          </>
        )}

        {findings.length === 0 && !reportData?.kani_metrics && (!reportData?.symbolic_paths || reportData.symbolic_paths.length === 0) && !error && (
          <p className="text-center text-zinc-500 dark:text-zinc-400 py-12">
            Load a report to view findings.
          </p>
        )}
      </main>
    </div>
  );
}
