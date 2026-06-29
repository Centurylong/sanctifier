"use client";

import { useCallback, useRef, useState, Suspense } from "react";
import Link from "next/link";
import type { AnalysisReport, Finding } from "../types";
import { transformReport } from "../lib/transform";
import { FindingsPanel } from "../components/FindingsPanel";
import { SanctityScore } from "../components/SanctityScore";
import { SummaryChart } from "../components/SummaryChart";
import { ThemeToggle } from "../components/ThemeToggle";
import { ErrorBoundary } from "../components/ErrorBoundary";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const MAX_BYTES = 512 * 1024;
const ALLOWED_EXT = [".rs", ".zip"];
const PLACEHOLDER = `// Paste your Soroban contract source here, e.g.:
#![no_std]
use soroban_sdk::{contract, contractimpl, Env};

#[contract]
pub struct TokenContract;

#[contractimpl]
impl TokenContract {
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        // Missing require_auth → auth gap
        env.storage().instance().set(&from, &(amount - 1)); // unchecked arithmetic
    }
}`;

type InputTab = "paste" | "upload";
type PageState = "idle" | "loading" | "results" | "error";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function bytesOf(s: string) {
  return new TextEncoder().encode(s).length;
}

function validateFile(file: File): string | null {
  const ext = "." + file.name.split(".").pop()?.toLowerCase();
  if (!ALLOWED_EXT.includes(ext))
    return `Only ${ALLOWED_EXT.join(", ")} files are accepted.`;
  if (file.size > MAX_BYTES)
    return `File too large — maximum size is ${Math.round(MAX_BYTES / 1024)} KB.`;
  return null;
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

function Spinner() {
  return (
    <svg
      className="h-5 w-5 animate-spin"
      viewBox="0 0 24 24"
      fill="none"
      aria-hidden="true"
    >
      <circle
        className="opacity-25"
        cx="12"
        cy="12"
        r="10"
        stroke="currentColor"
        strokeWidth="4"
      />
      <path
        className="opacity-75"
        fill="currentColor"
        d="M4 12a8 8 0 018-8v8H4z"
      />
    </svg>
  );
}

interface DropZoneProps {
  file: File | null;
  error: string | null;
  onFile: (f: File) => void;
}

function DropZone({ file, error, onFile }: DropZoneProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [dragging, setDragging] = useState(false);

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      setDragging(false);
      const dropped = e.dataTransfer.files[0];
      if (dropped) onFile(dropped);
    },
    [onFile]
  );

  const handleChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const picked = e.target.files?.[0];
      if (picked) onFile(picked);
      e.target.value = "";
    },
    [onFile]
  );

  return (
    <div className="space-y-3">
      <div
        role="button"
        tabIndex={0}
        aria-label="Drop zone — drag and drop a .rs or .zip file, or press Enter to browse"
        onDragOver={(e) => {
          e.preventDefault();
          setDragging(true);
        }}
        onDragLeave={() => setDragging(false)}
        onDrop={handleDrop}
        onClick={() => inputRef.current?.click()}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            inputRef.current?.click();
          }
        }}
        className={`flex cursor-pointer flex-col items-center justify-center gap-3 rounded-xl border-2 border-dashed px-6 py-14 text-center transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-zinc-400 ${
          dragging
            ? "border-blue-500 bg-blue-500/5"
            : error
              ? "border-red-400 dark:border-red-500"
              : "border-zinc-300 hover:border-zinc-400 dark:border-zinc-700 dark:hover:border-zinc-500"
        }`}
      >
        <svg
          width="36"
          height="36"
          viewBox="0 0 24 24"
          fill="none"
          aria-hidden="true"
          className="text-zinc-400 dark:text-zinc-500"
        >
          <path
            d="M12 16V8M12 8l-3 3M12 8l3 3"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
          <path
            d="M3 16.5V18a2 2 0 002 2h14a2 2 0 002-2v-1.5"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
          />
        </svg>

        {file ? (
          <p className="text-sm font-medium text-zinc-800 dark:text-zinc-200">
            {file.name}{" "}
            <span className="font-normal text-zinc-500 dark:text-zinc-400">
              ({Math.round(file.size / 1024)} KB)
            </span>
          </p>
        ) : (
          <>
            <p className="text-sm font-medium text-zinc-700 dark:text-zinc-300">
              Drag &amp; drop a <code className="font-mono">.rs</code> or{" "}
              <code className="font-mono">.zip</code> file
            </p>
            <p className="text-xs text-zinc-500 dark:text-zinc-400">
              or{" "}
              <span className="underline underline-offset-2">browse files</span>{" "}
              — max 512 KB
            </p>
          </>
        )}

        <input
          ref={inputRef}
          type="file"
          accept=".rs,.zip"
          className="sr-only"
          onChange={handleChange}
          aria-hidden="true"
          tabIndex={-1}
        />
      </div>

      {error && (
        <p role="alert" className="text-sm text-red-600 dark:text-red-400">
          {error}
        </p>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Page
// ---------------------------------------------------------------------------

export default function ScanPage() {
  // Input state
  const [tab, setTab] = useState<InputTab>("paste");
  const [source, setSource] = useState("");
  const [file, setFile] = useState<File | null>(null);
  const [fileError, setFileError] = useState<string | null>(null);

  // Page state
  const [pageState, setPageState] = useState<PageState>("idle");
  const [apiError, setApiError] = useState<string | null>(null);
  const [findings, setFindings] = useState<Finding[]>([]);
  const [reportId, setReportId] = useState<string | null>(null);
  const [copySuccess, setCopySuccess] = useState(false);

  // ── File selection ─────────────────────────────────────────────────────────
  const handleFile = useCallback((f: File) => {
    const err = validateFile(f);
    setFileError(err);
    setFile(err ? null : f);
  }, []);

  // ── Submit ─────────────────────────────────────────────────────────────────
  const handleSubmit = useCallback(async () => {
    setApiError(null);
    setFindings([]);
    setReportId(null);
    setCopySuccess(false);
    setPageState("loading");

    try {
      let body: BodyInit;
      const headers: Record<string, string> = {};

      if (tab === "paste") {
        if (!source.trim()) {
          setApiError("Please paste some Rust source code before scanning.");
          setPageState("idle");
          return;
        }
        if (bytesOf(source) > MAX_BYTES) {
          setApiError("Source exceeds the 512 KB limit.");
          setPageState("idle");
          return;
        }
        body = JSON.stringify({ source });
        headers["Content-Type"] = "application/json";
      } else {
        if (!file) {
          setApiError("Please select a file before scanning.");
          setPageState("idle");
          return;
        }
        const fd = new FormData();
        fd.append("file", file);
        body = fd;
        // No Content-Type header — browser sets multipart boundary automatically
      }

      const res = await fetch("/api/analyze", {
        method: "POST",
        headers,
        body,
      });

      const json = await res.json() as AnalysisReport & { error?: string; reportId?: string };

      if (!res.ok || json.error) {
        setApiError(json.error ?? `Server error (${res.status})`);
        setPageState("error");
        return;
      }

      // Extract reportId if present
      const { reportId, ...reportData } = json;
      if (reportId) {
        setReportId(reportId);
      }

      // Handle new CI/CD format with nested "findings" key
      const report = (reportData as Record<string, unknown>).findings
        ? ((reportData as Record<string, unknown>).findings as AnalysisReport)
        : reportData;

      const transformed = transformReport(report);
      setFindings(transformed);
      setPageState("results");
    } catch (err) {
      setApiError(
        err instanceof Error ? err.message : "Unexpected error — please try again."
      );
      setPageState("error");
    }
  }, [tab, source, file]);

  const canSubmit =
    pageState !== "loading" &&
    (tab === "paste" ? source.trim().length > 0 : file !== null);

  // ── Copy Link ──────────────────────────────────────────────────────────────
  const handleCopyLink = useCallback(async () => {
    if (!reportId) return;

    try {
      const url = `${window.location.origin}/report/${reportId}`;
      await navigator.clipboard.writeText(url);
      setCopySuccess(true);
      setTimeout(() => setCopySuccess(false), 2000);
    } catch (err) {
      console.error("Failed to copy link:", err);
    }
  }, [reportId]);

  // ── Render ─────────────────────────────────────────────────────────────────
  return (
    <div
      className="min-h-screen"
      style={{
        backgroundColor: "var(--background)",
        color: "var(--foreground)",
      }}
    >
      {/* Header */}
      <header
        className="flex flex-col gap-4 border-b px-4 py-4 sm:flex-row sm:items-center sm:justify-between sm:px-6"
        style={{ borderColor: "var(--border)", backgroundColor: "var(--card)" }}
        role="banner"
      >
        <div className="flex items-center gap-4 sm:gap-6">
          <Link
            href="/"
            className="text-lg font-bold focus:outline-none focus:ring-2"
            style={{ color: "var(--foreground)" }}
          >
            Sanctifier
          </Link>
          <span className="text-sm sm:text-base" style={{ color: "var(--muted-foreground)" }}>
            Scan Contract
          </span>
        </div>
        <nav className="flex items-center gap-4" aria-label="Main navigation">
          <Link
            href="/dashboard"
            className="text-sm font-medium transition-colors focus:outline-none focus:ring-2"
            style={{ color: "var(--muted-foreground)" }}
          >
            Dashboard
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

      <main id="main-content" className="mx-auto max-w-4xl space-y-8 px-4 py-8 sm:px-6">
        {/* Hero */}
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Scan a Contract</h1>
          <p className="mt-2 text-sm" style={{ color: "var(--muted-foreground)" }}>
            Paste Rust source or upload a <code className="font-mono">.rs</code> /{" "}
            <code className="font-mono">.zip</code> file — no install required.
          </p>
        </div>

        {/* Input card */}
        <section
          className="rounded-xl border p-6 space-y-5"
          style={{
            borderColor: "var(--border)",
            backgroundColor: "var(--card)",
          }}
          aria-label="Contract input"
        >
          {/* Tabs */}
          <div
            role="tablist"
            aria-label="Input method"
            className="flex gap-1 rounded-lg p-1"
            style={{ backgroundColor: "var(--muted)" }}
          >
            {(["paste", "upload"] as InputTab[]).map((t) => (
              <button
                key={t}
                role="tab"
                aria-selected={tab === t}
                aria-controls={`panel-${t}`}
                id={`tab-${t}`}
                onClick={() => setTab(t)}
                className="flex-1 rounded-md px-4 py-1.5 text-sm font-medium transition-all focus:outline-none focus:ring-2 focus:ring-inset focus:ring-zinc-400"
                style={
                  tab === t
                    ? {
                        backgroundColor: "var(--background)",
                        color: "var(--foreground)",
                        boxShadow: "0 1px 3px 0 rgba(0,0,0,.1)",
                      }
                    : { color: "var(--muted-foreground)" }
                }
              >
                {t === "paste" ? "Paste Source" : "Upload File"}
              </button>
            ))}
          </div>

          {/* Paste panel */}
          <div
            role="tabpanel"
            id="panel-paste"
            aria-labelledby="tab-paste"
            hidden={tab !== "paste"}
          >
            <label className="sr-only" htmlFor="source-input">
              Rust source code
            </label>
            <textarea
              id="source-input"
              value={source}
              onChange={(e) => setSource(e.target.value)}
              placeholder={PLACEHOLDER}
              rows={16}
              spellCheck={false}
              className="w-full resize-y rounded-lg border p-4 font-mono text-xs leading-relaxed focus:outline-none focus:ring-2 focus:ring-zinc-400 dark:focus:ring-zinc-500"
              style={{
                borderColor: "var(--border)",
                backgroundColor: "var(--background)",
                color: "var(--foreground)",
              }}
            />
            {source.length > 0 && (
              <p
                className="mt-1 text-right text-xs"
                style={{ color: "var(--muted-foreground)" }}
              >
                {Math.round(bytesOf(source) / 1024)} / 512 KB
              </p>
            )}
          </div>

          {/* Upload panel */}
          <div
            role="tabpanel"
            id="panel-upload"
            aria-labelledby="tab-upload"
            hidden={tab !== "upload"}
          >
            <DropZone file={file} error={fileError} onFile={handleFile} />
          </div>

          {/* Analyse button */}
          <button
            type="button"
            onClick={handleSubmit}
            disabled={!canSubmit}
            className="flex w-full items-center justify-center gap-2 rounded-lg px-5 py-2.5 text-sm font-semibold transition-all disabled:cursor-not-allowed disabled:opacity-50 focus:outline-none focus:ring-2 focus:ring-offset-2"
            style={{
              backgroundColor: "var(--primary)",
              color: "var(--primary-foreground)",
            }}
            aria-busy={pageState === "loading"}
          >
            {pageState === "loading" ? (
              <>
                <Spinner />
                Analysing…
              </>
            ) : (
              "Analyse →"
            )}
          </button>
        </section>

        {/* Error banner */}
        {pageState === "error" && apiError && (
          <div
            role="alert"
            className="rounded-xl border border-red-300 bg-red-50 px-5 py-4 text-sm text-red-700 dark:border-red-800 dark:bg-red-950/40 dark:text-red-400"
          >
            <strong className="font-semibold">Analysis error: </strong>
            {apiError}
          </div>
        )}

        {/* Loading progress */}
        {pageState === "loading" && (
          <div
            className="flex items-center gap-3 rounded-xl border px-5 py-4"
            style={{
              borderColor: "var(--border)",
              backgroundColor: "var(--card)",
              color: "var(--muted-foreground)",
            }}
            aria-live="polite"
            aria-label="Analysing contract"
          >
            <Spinner />
            <span className="text-sm">
              Running static analysis — this usually takes under a second…
            </span>
          </div>
        )}

        {/* Results */}
        {pageState === "results" && (
          <ErrorBoundary>
            <>
              {findings.length === 0 ? (
                <div
                  className="rounded-xl border border-dashed px-6 py-14 text-center"
                  style={{ borderColor: "var(--border)" }}
                >
                  <p
                    className="text-sm font-medium"
                    style={{ color: "var(--foreground)" }}
                  >
                    No issues found
                  </p>
                  <p
                    className="mt-1 text-xs"
                    style={{ color: "var(--muted-foreground)" }}
                  >
                    Great — the static analysis raised zero findings for this
                    source.
                  </p>
                </div>
              ) : (
                <div className="space-y-6">
                  <div className="flex items-center justify-between">
                    <h2 className="text-xl font-semibold">Results</h2>
                    <button
                      type="button"
                      onClick={() => {
                        setPageState("idle");
                        setFindings([]);
                        setSource("");
                        setFile(null);
                        setReportId(null);
                        setCopySuccess(false);
                      }}
                      className="text-xs underline underline-offset-2 focus:outline-none"
                      style={{ color: "var(--muted-foreground)" }}
                    >
                      Scan another
                    </button>
                  </div>

                  {/* Share Link */}
                  {reportId && (
                    <div
                      className="rounded-xl border p-4"
                      style={{
                        borderColor: "var(--border)",
                        backgroundColor: "var(--card)",
                      }}
                    >
                      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                        <div className="flex-1">
                          <p
                            className="text-xs font-medium"
                            style={{ color: "var(--muted-foreground)" }}
                          >
                            Shareable link — expires in 30 days
                          </p>
                          <code
                            className="mt-1 block truncate text-xs"
                            style={{ color: "var(--foreground)" }}
                          >
                            {typeof window !== "undefined"
                              ? `${window.location.origin}/report/${reportId}`
                              : `/report/${reportId}`}
                          </code>
                        </div>
                        <button
                          type="button"
                          onClick={handleCopyLink}
                          className="flex items-center gap-2 rounded-lg px-4 py-2 text-sm font-medium transition-colors focus:outline-none focus:ring-2"
                          style={{
                            backgroundColor: copySuccess
                              ? "var(--primary)"
                              : "var(--muted)",
                            color: copySuccess
                              ? "var(--primary-foreground)"
                              : "var(--foreground)",
                          }}
                        >
                          {copySuccess ? (
                            <>
                              <svg
                                width="16"
                                height="16"
                                viewBox="0 0 24 24"
                                fill="none"
                                aria-hidden="true"
                              >
                                <path
                                  d="M20 6L9 17l-5-5"
                                  stroke="currentColor"
                                  strokeWidth="2"
                                  strokeLinecap="round"
                                  strokeLinejoin="round"
                                />
                              </svg>
                              Copied!
                            </>
                          ) : (
                            <>
                              <svg
                                width="16"
                                height="16"
                                viewBox="0 0 24 24"
                                fill="none"
                                aria-hidden="true"
                              >
                                <path
                                  d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"
                                  stroke="currentColor"
                                  strokeWidth="2"
                                  strokeLinecap="round"
                                  strokeLinejoin="round"
                                />
                                <path
                                  d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"
                                  stroke="currentColor"
                                  strokeWidth="2"
                                  strokeLinecap="round"
                                  strokeLinejoin="round"
                                />
                              </svg>
                              Copy Link
                            </>
                          )}
                        </button>
                      </div>
                    </div>
                  )}

                  <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
                    <SanctityScore findings={findings} />
                    <SummaryChart findings={findings} />
                  </div>

                  <section aria-label="Findings">
                    <Suspense
                      fallback={
                        <p
                          className="py-6 text-center text-sm"
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
            </>
          </ErrorBoundary>
        )}
      </main>
    </div>
  );
}
