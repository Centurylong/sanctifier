"use client";

import { useState, useEffect, useMemo } from "react";
import Link from "next/link";
import { ThemeToggle } from "../components/ThemeToggle";
import { CodeSnippet } from "../components/CodeSnippet";
import { listSavedReports } from "../lib/reports-store";
import { diffFindings } from "../lib/diff";
import type { SavedReport, Finding, Severity, DiffBucket } from "../types";

// ─── severity colours (same palette as FindingsList) ─────────────────────────
const severityColors: Record<Severity, string> = {
  critical: "bg-red-500/10 border-red-500/50 text-red-700 dark:text-red-400",
  high: "bg-orange-500/10 border-orange-500/50 text-orange-700 dark:text-orange-400",
  medium: "bg-amber-500/10 border-amber-500/50 text-amber-700 dark:text-amber-400",
  low: "bg-zinc-500/10 border-zinc-500/50 text-zinc-700 dark:text-zinc-400",
};

// ─── bucket presentation ──────────────────────────────────────────────────────
const bucketMeta: Record<
  DiffBucket,
  { label: string; icon: string; ring: string; badge: string }
> = {
  added: {
    label: "Added",
    icon: "➕",
    ring: "border-emerald-400 dark:border-emerald-600",
    badge:
      "bg-emerald-100 dark:bg-emerald-900/30 text-emerald-700 dark:text-emerald-300",
  },
  removed: {
    label: "Removed",
    icon: "➖",
    ring: "border-red-400 dark:border-red-600",
    badge:
      "bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300",
  },
  unchanged: {
    label: "Unchanged",
    icon: "＝",
    ring: "border-zinc-300 dark:border-zinc-700",
    badge:
      "bg-zinc-100 dark:bg-zinc-800 text-zinc-600 dark:text-zinc-400",
  },
};

// ─── sub-components ───────────────────────────────────────────────────────────

function ReportSelect({
  label,
  reports,
  value,
  onChange,
}: {
  label: string;
  reports: SavedReport[];
  value: string;
  onChange: (id: string) => void;
}) {
  return (
    <div className="flex flex-col gap-1 flex-1 min-w-0">
      <label className="text-sm font-medium text-zinc-700 dark:text-zinc-300">
        {label}
      </label>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="rounded-lg border border-zinc-300 dark:border-zinc-600 bg-white dark:bg-zinc-900 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-zinc-400 dark:focus:ring-zinc-600"
      >
        <option value="">— Select a report —</option>
        {reports.map((r) => (
          <option key={r.id} value={r.id}>
            {r.label}
          </option>
        ))}
      </select>
    </div>
  );
}

function BucketColumn({
  bucket,
  findings,
}: {
  bucket: DiffBucket;
  findings: Finding[];
}) {
  const { label, icon, ring, badge } = bucketMeta[bucket];

  return (
    <div className={`flex flex-col rounded-xl border-2 ${ring} overflow-hidden`}>
      {/* header */}
      <div className="flex items-center justify-between px-4 py-3 bg-white dark:bg-zinc-900 border-b border-zinc-200 dark:border-zinc-800">
        <span className="font-semibold text-sm flex items-center gap-2">
          <span>{icon}</span>
          {label}
        </span>
        <span className={`text-xs font-bold px-2 py-0.5 rounded-full ${badge}`}>
          {findings.length}
        </span>
      </div>

      {/* body */}
      <div className="flex-1 overflow-y-auto max-h-[60vh] p-3 space-y-3 bg-zinc-50 dark:bg-zinc-950">
        {findings.length === 0 ? (
          <p className="text-center text-zinc-400 dark:text-zinc-600 py-10 text-sm">
            No findings in this bucket.
          </p>
        ) : (
          findings.map((f) => (
            <FindingCard key={f.fingerprint + f.id} finding={f} />
          ))
        )}
      </div>
    </div>
  );
}

function FindingCard({ finding: f }: { finding: Finding }) {
  return (
    <div className={`rounded-lg border p-3 ${severityColors[f.severity]}`}>
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0 flex-1">
          <span className="text-xs font-semibold uppercase tracking-wide opacity-75">
            {f.category}
          </span>
          <p className="mt-0.5 text-sm font-medium leading-snug">{f.title}</p>
          <p className="mt-0.5 text-xs opacity-80 truncate" title={f.location}>
            {f.location}
          </p>
          {f.suggestion && (
            <p className="mt-1 text-xs italic opacity-80">💡 {f.suggestion}</p>
          )}
        </div>
        <span
          className={`shrink-0 rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase ${severityColors[f.severity]}`}
        >
          {f.severity}
        </span>
      </div>
      {f.snippet && (
        <div className="mt-2">
          <CodeSnippet code={f.snippet} highlightLine={f.line} />
        </div>
      )}
    </div>
  );
}

function CountPill({
  count,
  bucket,
}: {
  count: number;
  bucket: DiffBucket;
}) {
  const { icon, badge } = bucketMeta[bucket];
  return (
    <span className={`inline-flex items-center gap-1 rounded-full px-3 py-1 text-sm font-semibold ${badge}`}>
      {icon} {count}
    </span>
  );
}

// ─── main page ────────────────────────────────────────────────────────────────

export default function DiffPage() {
  const [reports, setReports] = useState<SavedReport[]>([]);
  const [baseId, setBaseId] = useState("");
  const [nextId, setNextId] = useState("");

  // Load saved reports from localStorage on mount
  useEffect(() => {
    setReports(listSavedReports());
  }, []);

  const baseReport = useMemo(
    () => reports.find((r) => r.id === baseId),
    [reports, baseId]
  );
  const nextReport = useMemo(
    () => reports.find((r) => r.id === nextId),
    [reports, nextId]
  );

  const diffed = useMemo(() => {
    if (!baseReport || !nextReport) return null;
    return diffFindings(baseReport.findings, nextReport.findings);
  }, [baseReport, nextReport]);

  const added = useMemo(
    () => diffed?.filter((d) => d.bucket === "added").map((d) => d.finding) ?? [],
    [diffed]
  );
  const removed = useMemo(
    () => diffed?.filter((d) => d.bucket === "removed").map((d) => d.finding) ?? [],
    [diffed]
  );
  const unchanged = useMemo(
    () =>
      diffed?.filter((d) => d.bucket === "unchanged").map((d) => d.finding) ?? [],
    [diffed]
  );

  const noReports = reports.length === 0;
  const sameReport = baseId && nextId && baseId === nextId;
  const readyToCompare = !noReports && baseId && nextId && !sameReport;

  return (
    <div className="min-h-screen bg-zinc-50 dark:bg-zinc-950 text-zinc-900 dark:text-zinc-100">
      {/* ── header ── */}
      <header className="border-b border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900 px-6 py-4 flex items-center justify-between">
        <div className="flex items-center gap-6">
          <Link href="/" className="font-bold text-lg">
            Sanctifier
          </Link>
          <Link
            href="/dashboard"
            className="text-zinc-500 dark:text-zinc-400 hover:text-zinc-700 dark:hover:text-zinc-200 text-sm transition-colors"
          >
            Dashboard
          </Link>
          <span className="text-zinc-900 dark:text-zinc-100 text-sm font-medium">
            Scan Diff
          </span>
        </div>
        <ThemeToggle />
      </header>

      <main className="max-w-7xl mx-auto px-6 py-8 space-y-8">
        {/* ── report picker ── */}
        <section className="rounded-lg border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900 p-6">
          <h2 className="text-lg font-semibold mb-1">Compare Two Reports</h2>
          <p className="text-sm text-zinc-500 dark:text-zinc-400 mb-6">
            Select a base and a target report to see which findings were added,
            removed, or persisted between scans.
          </p>

          {noReports ? (
            <div className="rounded-lg border border-amber-300 dark:border-amber-700 bg-amber-50 dark:bg-amber-950/20 p-4 text-sm text-amber-700 dark:text-amber-300">
              No saved reports found. Go to the{" "}
              <Link
                href="/dashboard"
                className="underline font-medium hover:opacity-75"
              >
                Dashboard
              </Link>{" "}
              and save a report first.
            </div>
          ) : (
            <div className="flex flex-col sm:flex-row gap-4 items-end">
              <ReportSelect
                label="Base report (older)"
                reports={reports}
                value={baseId}
                onChange={setBaseId}
              />
              <span className="hidden sm:flex items-center pb-2 text-zinc-400 dark:text-zinc-600 font-bold text-lg">
                →
              </span>
              <ReportSelect
                label="Target report (newer)"
                reports={reports}
                value={nextId}
                onChange={setNextId}
              />
            </div>
          )}

          {sameReport && (
            <p className="mt-3 text-sm text-amber-600 dark:text-amber-400">
              Please select two different reports.
            </p>
          )}
        </section>

        {/* ── summary counts ── */}
        {readyToCompare && diffed && (
          <>
            <section className="rounded-lg border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900 p-5">
              <h3 className="text-sm font-semibold text-zinc-500 dark:text-zinc-400 uppercase tracking-wider mb-4">
                Diff summary
              </h3>
              <div className="flex flex-wrap gap-4 items-center">
                <div>
                  <p className="text-xs text-zinc-500 dark:text-zinc-400 mb-1 font-medium">
                    Base
                  </p>
                  <p className="text-sm font-semibold truncate max-w-xs">
                    {baseReport?.label}
                  </p>
                </div>
                <span className="text-zinc-300 dark:text-zinc-700 font-bold text-xl">
                  →
                </span>
                <div>
                  <p className="text-xs text-zinc-500 dark:text-zinc-400 mb-1 font-medium">
                    Target
                  </p>
                  <p className="text-sm font-semibold truncate max-w-xs">
                    {nextReport?.label}
                  </p>
                </div>
                <div className="ml-auto flex gap-3 flex-wrap">
                  <CountPill count={added.length} bucket="added" />
                  <CountPill count={removed.length} bucket="removed" />
                  <CountPill count={unchanged.length} bucket="unchanged" />
                </div>
              </div>
            </section>

            {/* ── three-bucket side-by-side ── */}
            <section>
              <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
                <BucketColumn bucket="added" findings={added} />
                <BucketColumn bucket="removed" findings={removed} />
                <BucketColumn bucket="unchanged" findings={unchanged} />
              </div>
            </section>
          </>
        )}

        {/* ── empty state before selection ── */}
        {!noReports && !readyToCompare && !sameReport && (
          <p className="text-center text-zinc-400 dark:text-zinc-600 py-16">
            Select two reports above to compare them.
          </p>
        )}
      </main>
    </div>
  );
}
