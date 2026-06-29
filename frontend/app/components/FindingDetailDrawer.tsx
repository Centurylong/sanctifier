"use client";

import { useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import type { Finding, Severity } from "../types";
import { CodeSnippet } from "./CodeSnippet";

const CATEGORY_TO_CODE: Record<string, string> = {
  "Auth Gap": "S001",
  "Panic/Unwrap": "S002",
  Arithmetic: "S003",
  "Ledger Size": "S004",
  "Unsafe Pattern": "S006",
  "Custom Rule": "S007",
};

const DOCS_BASE =
  "https://github.com/Centurylong/sanctifier/blob/main/docs/error-codes.md";

const severityBadge: Record<Severity, string> = {
  critical:
    "bg-red-500/15 text-red-700 dark:text-red-400 border border-red-500/50",
  high: "bg-orange-500/15 text-orange-700 dark:text-orange-400 border border-orange-500/50",
  medium:
    "bg-amber-500/15 text-amber-700 dark:text-amber-400 border border-amber-500/50",
  low: "bg-zinc-500/15 text-zinc-700 dark:text-zinc-400 border border-zinc-500/50",
};

const FOCUSABLE =
  'button,[href],input,select,textarea,[tabindex]:not([tabindex="-1"])';

interface FindingDetailDrawerProps {
  finding: Finding | null;
  onClose: () => void;
}

export function FindingDetailDrawer({
  finding,
  onClose,
}: FindingDetailDrawerProps) {
  const drawerRef = useRef<HTMLDivElement>(null);
  const savedFocusRef = useRef<Element | null>(null);

  useEffect(() => {
    if (!finding) return;

    savedFocusRef.current = document.activeElement;

    const drawer = drawerRef.current;
    if (!drawer) return;

    // Move focus into the drawer on open.
    const focusables = drawer.querySelectorAll<HTMLElement>(FOCUSABLE);
    focusables[0]?.focus();

    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") {
        onClose();
        return;
      }

      if (e.key === "Tab") {
        const elements = Array.from(
          drawer!.querySelectorAll<HTMLElement>(FOCUSABLE)
        ).filter((el) => !el.hasAttribute("disabled"));

        if (elements.length === 0) return;

        const first = elements[0];
        const last = elements[elements.length - 1];

        if (e.shiftKey && document.activeElement === first) {
          e.preventDefault();
          last.focus();
        } else if (!e.shiftKey && document.activeElement === last) {
          e.preventDefault();
          first.focus();
        }
      }
    }

    document.addEventListener("keydown", handleKeyDown);

    return () => {
      document.removeEventListener("keydown", handleKeyDown);
      // Restore focus to the element that triggered the drawer.
      if (savedFocusRef.current instanceof HTMLElement) {
        savedFocusRef.current.focus();
      }
    };
  }, [finding, onClose]);

  if (!finding || typeof document === "undefined") return null;

  const code = CATEGORY_TO_CODE[finding.category];
  const docsHref = code ? `${DOCS_BASE}#${code.toLowerCase()}` : DOCS_BASE;

  return createPortal(
    <>
      {/* Backdrop */}
      <div
        className="fixed inset-0 z-40 bg-black/40 backdrop-blur-sm"
        aria-hidden="true"
        onClick={onClose}
      />

      {/* Drawer panel */}
      <div
        ref={drawerRef}
        role="dialog"
        aria-modal="true"
        aria-label={`Finding detail: ${finding.title}`}
        className="fixed right-0 top-0 z-50 flex h-full w-full max-w-xl flex-col overflow-y-auto border-l border-zinc-200 bg-white shadow-2xl dark:border-zinc-800 dark:bg-zinc-950 theme-high-contrast:border-white theme-high-contrast:bg-black"
      >
        {/* Header */}
        <div className="flex items-center justify-between border-b border-zinc-200 px-6 py-4 dark:border-zinc-800 theme-high-contrast:border-white">
          <h2 className="text-base font-semibold text-zinc-900 dark:text-zinc-100 theme-high-contrast:text-white">
            Finding Detail
          </h2>
          <button
            type="button"
            onClick={onClose}
            aria-label="Close"
            className="rounded-lg p-1.5 text-zinc-500 hover:bg-zinc-100 hover:text-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800 dark:hover:text-zinc-100"
          >
            <svg
              width="16"
              height="16"
              viewBox="0 0 16 16"
              fill="none"
              aria-hidden="true"
            >
              <path
                d="M12 4L4 12M4 4l8 8"
                stroke="currentColor"
                strokeWidth="1.5"
                strokeLinecap="round"
              />
            </svg>
          </button>
        </div>

        {/* Body */}
        <div className="flex flex-1 flex-col gap-6 px-6 py-5">
          {/* Severity + code + docs link */}
          <div className="flex flex-wrap items-center gap-2">
            <span
              className={`inline-block rounded px-2.5 py-0.5 text-xs font-semibold uppercase tracking-wide ${severityBadge[finding.severity]}`}
            >
              {finding.severity}
            </span>
            <span className="rounded bg-zinc-100 px-2.5 py-0.5 font-mono text-xs font-medium text-zinc-700 dark:bg-zinc-800 dark:text-zinc-300 theme-high-contrast:bg-zinc-900 theme-high-contrast:text-white">
              {finding.category}
              {code && <> · {code}</>}
            </span>
            <a
              href={docsHref}
              target="_blank"
              rel="noopener noreferrer"
              className="ml-auto text-xs text-blue-600 underline-offset-2 hover:underline dark:text-blue-400 theme-high-contrast:text-blue-300"
            >
              docs&nbsp;/&nbsp;explain ↗
            </a>
          </div>

          {/* Message */}
          <section aria-label="Message">
            <p className="mb-1 text-xs font-semibold uppercase tracking-wide text-zinc-500 dark:text-zinc-400 theme-high-contrast:text-zinc-300">
              Message
            </p>
            <p className="text-sm text-zinc-900 dark:text-zinc-100 theme-high-contrast:text-white">
              {finding.title}
            </p>
          </section>

          {/* Location */}
          <section aria-label="Location">
            <p className="mb-1 text-xs font-semibold uppercase tracking-wide text-zinc-500 dark:text-zinc-400 theme-high-contrast:text-zinc-300">
              Location
            </p>
            <p className="font-mono text-xs text-zinc-600 dark:text-zinc-400 theme-high-contrast:text-zinc-300">
              {finding.location}
            </p>
          </section>

          {/* Remediation */}
          {finding.suggestion && (
            <section aria-label="Remediation">
              <p className="mb-1 text-xs font-semibold uppercase tracking-wide text-zinc-500 dark:text-zinc-400 theme-high-contrast:text-zinc-300">
                Remediation
              </p>
              <p className="text-sm italic text-zinc-700 dark:text-zinc-300 theme-high-contrast:text-white">
                {finding.suggestion}
              </p>
            </section>
          )}

          {/* Code snippet with highlighted offending line */}
          {finding.snippet && (
            <section aria-label="Code excerpt">
              <p className="mb-2 text-xs font-semibold uppercase tracking-wide text-zinc-500 dark:text-zinc-400 theme-high-contrast:text-zinc-300">
                Code Excerpt
              </p>
              <CodeSnippet
                code={finding.snippet}
                highlightLine={finding.line}
              />
            </section>
          )}
        </div>
      </div>
    </>,
    document.body
  );
}
