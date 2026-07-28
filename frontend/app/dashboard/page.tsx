"use client";

import { useState, useCallback, Suspense } from "react";
import type { AnalysisReport, CallGraphNode, CallGraphEdge, Finding } from "../types";
import { transformReport, extractCallGraph } from "../lib/transform";
import { exportToPdf } from "../lib/export-pdf";
import { FindingsPanel } from "../components/FindingsPanel";
import { SummaryChart } from "../components/SummaryChart";
import { SanctityScore } from "../components/SanctityScore";
import { ScoreTrendChart } from "../components/ScoreTrendChart";
import { CallGraph } from "../components/CallGraph";
import { ThemeToggle } from "../components/ThemeToggle";
import { ErrorBoundary } from "../components/ErrorBoundary";
import { DashboardSkeleton } from "../components/LoadingSkeleton";
import Link from "next/link";
import type { Metadata } from "next";

const SAMPLE_JSON = `{
  "size_warnings": [],
  "unsafe_patterns": [],
  "auth_gaps": [],
  "panic_issues": [],
  "arithmetic_issues": []
}`;

type Tab = "findings" | "callgraph";

export default function DashboardPage() {
  const [findings, setFindings] = useState<Finding[]>([]);
  const [callGraphNodes, setCallGraphNodes] = useState<CallGraphNode[]>([]);
  const [callGraphEdges, setCallGraphEdges] = useState<CallGraphEdge[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [jsonInput, setJsonInput] = useState("");
  const [activeTab, setActiveTab] = useState<Tab>("findings");
  const [isLoading, setIsLoading] = useState(false);

  const parseReport = useCallback((text: string) => {
    setError(null);
    setIsLoading(true);
    
    try {
      const parsed = JSON.parse(text || SAMPLE_JSON) as AnalysisReport;

      // Handle new CI/CD format with nested "findings" key
      const report = (parsed as Record<string, unknown>).findings
        ? ((parsed as Record<string, unknown>).findings as AnalysisReport)
        : parsed;

      setFindings(transformReport(report));
      const { nodes, edges } = extractCallGraph(report);
      setCallGraphNodes(nodes);
      setCallGraphEdges(edges);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Invalid JSON");
      setFindings([]);
      setCallGraphNodes([]);
      setCallGraphEdges([]);
    } finally {
      setIsLoading(false);
    }
  }, []);

  const loadReport = useCallback(() => {
    parseReport(jsonInput);
  }, [jsonInput, parseReport]);

  const handleFileUpload = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    
    setIsLoading(true);
    const reader = new FileReader();
    reader.onload = (ev) => {
      const text = ev.target?.result as string;
      setJsonInput(text);
      parseReport(text);
    };
    reader.onerror = () => {
      setError("Failed to read file");
      setIsLoading(false);
    };
    reader.readAsText(file);
    e.target.value = "";
  }, [parseReport]);

  const hasData = findings.length > 0;

  return (
    <div className="min-h-screen" style={{ backgroundColor: "var(--background)", color: "var(--foreground)" }}>
      <header 
        className="border-b px-4 sm:px-6 py-4 flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4"
        style={{ borderColor: "var(--border)", backgroundColor: "var(--card)" }}
        role="banner"
      >
        <div className="flex items-center gap-4 sm:gap-6">
          <Link 
            href="/" 
            className="font-bold text-lg whitespace-nowrap focus:outline-none focus:ring-2"
            style={{ color: "var(--foreground)" }}
          >
            Sanctifier
          </Link>
          <span className="text-sm sm:text-base" style={{ color: "var(--muted-foreground)" }}>
            Security Dashboard
          </span>
        </div>
        <nav className="flex items-center gap-4" aria-label="Main navigation">
          <Link
            href="/scan"
            className="text-sm font-medium transition-colors focus:outline-none focus:ring-2"
            style={{ color: "var(--muted-foreground)" }}
          >
            Scan
          </Link>
          <Link
            href="/terminal"
            className="text-sm font-medium transition-colors focus:outline-none focus:ring-2"
            style={{ color: "var(--muted-foreground)" }}
          >
            Live Terminal
          </Link>
          <ThemeToggle />
        </nav>
      </header>

      <main id="main-content" className="max-w-6xl mx-auto px-4 sm:px-6 py-8 space-y-8">
        <section 
          className="rounded-lg border p-6"
          style={{ borderColor: "var(--border)", backgroundColor: "var(--card)" }}
        >
          <h2 className="text-lg font-semibold mb-4">Load Analysis Report</h2>
          <p className="text-sm mb-4" style={{ color: "var(--muted-foreground)" }}>
            Paste JSON from{" "}
            <code 
              className="px-1 rounded font-mono text-xs"
              style={{ backgroundColor: "var(--muted)" }}
            >
              sanctifier analyze --format json
            </code>{" "}
            or upload a file.
          </p>
          <div className="flex flex-wrap gap-2 sm:gap-4">
            <label 
              className="flex-1 sm:flex-none text-center cursor-pointer rounded-lg border px-4 py-2 text-sm transition-colors focus-within:ring-2"
              style={{ borderColor: "var(--border)" }}
            >
              Upload JSON
              <input
                type="file"
                accept=".json"
                className="hidden"
                onChange={handleFileUpload}
                aria-label="Upload JSON file"
              />
            </label>
            <button
              onClick={loadReport}
              disabled={isLoading}
              className="flex-1 sm:flex-none rounded-lg px-4 py-2 text-sm font-medium transition-colors disabled:opacity-50 focus:outline-none focus:ring-2"
              style={{
                backgroundColor: "var(--primary)",
                color: "var(--primary-foreground)",
              }}
            >
              {isLoading ? "Parsing..." : "Parse JSON"}
            </button>
            <button
              onClick={() => {
                exportToPdf(findings);
              }}
              disabled={!hasData || isLoading}
              className="flex-1 sm:flex-none rounded-lg border px-4 py-2 text-sm transition-colors disabled:opacity-50 focus:outline-none focus:ring-2"
              style={{ borderColor: "var(--border)" }}
            >
              Export PDF
            </button>
          </div>
          {error && (
            <div
              role="alert"
              aria-live="polite"
              className="mt-2 text-sm p-2 rounded"
              style={{ color: "var(--destructive)" }}
            >
              {error}
            </div>
          )}
          <textarea
            value={jsonInput}
            onChange={(e) => setJsonInput(e.target.value)}
            placeholder={SAMPLE_JSON}
            className="mt-4 w-full h-32 rounded-lg border p-3 font-mono text-sm focus:ring-2 outline-none transition-colors"
            style={{
              borderColor: "var(--border)",
              backgroundColor: "var(--background)",
              color: "var(--foreground)",
            }}
            aria-label="JSON input"
          />
        </section>

        {isLoading && <DashboardSkeleton />}

        {!isLoading && hasData && (
          <ErrorBoundary>
            <>
              <section className="grid grid-cols-1 md:grid-cols-2 gap-6">
                <SanctityScore findings={findings} />
                <SummaryChart findings={findings} />
              </section>

              {/* Historical trend. Reads from an in-memory adapter today and
                  will swap to the persisted source from the shareable
                  permalink work without touching this call site. */}
              <ScoreTrendChart contractId="demo-healthy" />

              {/* Tab navigation */}
              <div 
                className="flex gap-2 border-b"
                style={{ borderColor: "var(--border)" }}
                role="tablist"
                aria-label="Report sections"
              >
                <button
                  role="tab"
                  aria-selected={activeTab === "findings"}
                  aria-controls="findings-panel"
                  id="findings-tab"
                  onClick={() => setActiveTab("findings")}
                  className={`px-4 py-2 text-sm font-medium border-b-2 transition-colors focus:outline-none focus:ring-2`}
                  style={{
                    borderColor: activeTab === "findings" ? "var(--primary)" : "transparent",
                    color: activeTab === "findings" ? "var(--foreground)" : "var(--muted-foreground)",
                  }}
                >
                  Findings
                </button>
                <button
                  role="tab"
                  aria-selected={activeTab === "callgraph"}
                  aria-controls="callgraph-panel"
                  id="callgraph-tab"
                  onClick={() => setActiveTab("callgraph")}
                  className={`px-4 py-2 text-sm font-medium border-b-2 transition-colors focus:outline-none focus:ring-2`}
                  style={{
                    borderColor: activeTab === "callgraph" ? "var(--primary)" : "transparent",
                    color: activeTab === "callgraph" ? "var(--foreground)" : "var(--muted-foreground)",
                  }}
                >
                  Call Graph
                </button>
              </div>

              {activeTab === "findings" && (
                <div
                  role="tabpanel"
                  id="findings-panel"
                  aria-labelledby="findings-tab"
                >
                  <section>
                    <h2 className="text-lg font-semibold mb-4">Findings</h2>
                    <Suspense
                      fallback={
                        <p
                          className="py-8 text-center"
                          style={{ color: "var(--muted-foreground)" }}
                        >
                          Loading findings…
                        </p>
                      }
                    >
                      <FindingsPanel findings={findings} />
                    </Suspense>
                  </section>
                </div>
              )}

              {activeTab === "callgraph" && (
                <section
                  role="tabpanel"
                  id="callgraph-panel"
                  aria-labelledby="callgraph-tab"
                >
                  <CallGraph nodes={callGraphNodes} edges={callGraphEdges} />
                </section>
              )}
            </>
          </ErrorBoundary>
        )}

        {!isLoading && !hasData && !error && (
          <p className="text-center py-12" style={{ color: "var(--muted-foreground)" }}>
            Load a report to view findings.
          </p>
        )}
      </main>
    </div>
  );
}
