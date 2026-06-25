"use client";

import { useEffect, useState } from "react";
import { useParams, useRouter } from "next/navigation";
import Link from "next/link";
import type { Finding } from "../../types";
import type { StoredReport } from "../../lib/report-storage";
import { FindingsPanel } from "../../components/FindingsPanel";
import { SanctityScore } from "../../components/SanctityScore";
import { SummaryChart } from "../../components/SummaryChart";
import { ThemeToggle } from "../../components/ThemeToggle";
import { ErrorBoundary } from "../../components/ErrorBoundary";

type PageState = "loading" | "loaded" | "not-found" | "error";

export default function ReportPage() {
  const params = useParams();
  const router = useRouter();
  const id = params.id as string;

  const [state, setState] = useState<PageState>("loading");
  const [report, setReport] = useState<StoredReport | null>(null);
  const [copySuccess, setCopySuccess] = useState(false);

  useEffect(() => {
    async function fetchReport() {
      try {
        const res = await fetch(`/api/report/${id}`);

        if (res.status === 404) {
          setState("not-found");
          return;
        }

        if (!res.ok) {
          setState("error");
          return;
        }

        const data: StoredReport = await res.json();
        setReport(data);
        setState("loaded");
      } catch (err) {
        console.error("Failed to fetch report:", err);
        setState("error");
      }
    }

    fetchReport();
  }, [id]);

  const handleCopyLink = async () => {
    try {
      const url = window.location.href;
      await navigator.clipboard.writeText(url);
      setCopySuccess(true);
      setTimeout(() => setCopySuccess(false), 2000);
    } catch (err) {
      console.error("Failed to copy link:", err);
    }
  };

  const formatDate = (timestamp: number) => {
    return new Date(timestamp).toLocaleDateString("en-US", {
      year: "numeric",
      month: "long",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  };

  const formatExpiry = (expiresAt: number) => {
    const days = Math.ceil((expiresAt - Date.now()) / (1000 * 60 * 60 * 24));
    return days > 0 ? `${days} day${days !== 1 ? "s" : ""}` : "expired";
  };

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
            Shared Report
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
            href="/dashboard"
            className="text-sm font-medium transition-colors focus:outline-none focus:ring-2"
            style={{ color: "var(--muted-foreground)" }}
          >
            Dashboard
          </Link>
          <ThemeToggle />
        </nav>
      </header>

      <main id="main-content" className="mx-auto max-w-4xl space-y-8 px-4 py-8 sm:px-6">
        {/* Loading */}
        {state === "loading" && (
          <div
            className="flex items-center justify-center py-20"
            role="status"
            aria-live="polite"
          >
            <div className="flex items-center gap-3">
              <svg
                className="h-6 w-6 animate-spin"
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
              <span style={{ color: "var(--muted-foreground)" }}>
                Loading report...
              </span>
            </div>
          </div>
        )}

        {/* Not Found */}
        {state === "not-found" && (
          <div className="py-20 text-center">
            <div
              className="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-full"
              style={{ backgroundColor: "var(--muted)" }}
            >
              <svg
                width="32"
                height="32"
                viewBox="0 0 24 24"
                fill="none"
                aria-hidden="true"
                style={{ color: "var(--muted-foreground)" }}
              >
                <path
                  d="M12 9v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
                  stroke="currentColor"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
              </svg>
            </div>
            <h1 className="text-2xl font-bold">Report Not Found</h1>
            <p className="mt-2 text-sm" style={{ color: "var(--muted-foreground)" }}>
              This report doesn't exist or has expired.
            </p>
            <div className="mt-6">
              <Link
                href="/scan"
                className="inline-flex items-center gap-2 rounded-lg px-5 py-2.5 text-sm font-semibold transition-colors focus:outline-none focus:ring-2"
                style={{
                  backgroundColor: "var(--primary)",
                  color: "var(--primary-foreground)",
                }}
              >
                Scan a New Contract
              </Link>
            </div>
          </div>
        )}

        {/* Error */}
        {state === "error" && (
          <div className="py-20 text-center">
            <div
              className="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-full bg-red-100 dark:bg-red-950/40"
            >
              <svg
                width="32"
                height="32"
                viewBox="0 0 24 24"
                fill="none"
                aria-hidden="true"
                className="text-red-600 dark:text-red-400"
              >
                <path
                  d="M6 18L18 6M6 6l12 12"
                  stroke="currentColor"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
              </svg>
            </div>
            <h1 className="text-2xl font-bold">Error Loading Report</h1>
            <p className="mt-2 text-sm" style={{ color: "var(--muted-foreground)" }}>
              Something went wrong. Please try again later.
            </p>
            <div className="mt-6">
              <button
                onClick={() => router.refresh()}
                className="inline-flex items-center gap-2 rounded-lg px-5 py-2.5 text-sm font-semibold transition-colors focus:outline-none focus:ring-2"
                style={{
                  backgroundColor: "var(--primary)",
                  color: "var(--primary-foreground)",
                }}
              >
                Retry
              </button>
            </div>
          </div>
        )}

        {/* Report Loaded */}
        {state === "loaded" && report && (
          <ErrorBoundary>
            <div className="space-y-6">
              {/* Report Header */}
              <div
                className="rounded-xl border p-6"
                style={{
                  borderColor: "var(--border)",
                  backgroundColor: "var(--card)",
                }}
              >
                <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
                  <div>
                    <h1 className="text-2xl font-bold">Analysis Report</h1>
                    <p className="mt-1 text-sm" style={{ color: "var(--muted-foreground)" }}>
                      Generated {formatDate(report.timestamp)}
                    </p>
                    <p className="mt-1 text-xs" style={{ color: "var(--muted-foreground)" }}>
                      Expires in {formatExpiry(report.expiresAt)}
                    </p>
                  </div>

                  <button
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

                {report.sourceSnippet && (
                  <details className="mt-4">
                    <summary
                      className="cursor-pointer text-sm font-medium"
                      style={{ color: "var(--muted-foreground)" }}
                    >
                      View source snippet
                    </summary>
                    <pre
                      className="mt-2 overflow-x-auto rounded-lg border p-3 text-xs"
                      style={{
                        borderColor: "var(--border)",
                        backgroundColor: "var(--background)",
                      }}
                    >
                      <code>{report.sourceSnippet}</code>
                    </pre>
                  </details>
                )}
              </div>

              {/* Results */}
              {report.findings.length === 0 ? (
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
                    The static analysis raised zero findings for this source.
                  </p>
                </div>
              ) : (
                <>
                  <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
                    <SanctityScore findings={report.findings} />
                    <SummaryChart findings={report.findings} />
                  </div>

                  <section aria-label="Findings">
                    <FindingsPanel findings={report.findings} />
                  </section>
                </>
              )}
            </div>
          </ErrorBoundary>
        )}
      </main>
    </div>
  );
}
