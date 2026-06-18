"use client";

import { useCallback, useMemo, useRef, useState } from "react";
import {
  createColumnHelper,
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  useReactTable,
  type OnChangeFn,
  type Row,
  type SortingState,
} from "@tanstack/react-table";
import { useVirtualizer } from "@tanstack/react-virtual";
import type { Finding, Severity } from "../types";
import { CodeSnippet } from "./CodeSnippet";
import { SEVERITY_RANK, type SortDir, type SortKey } from "../lib/findings-query";

interface FindingsTableProps {
  /** Already filtered findings; the table owns sorting + virtualization. */
  rows: Finding[];
  sort: SortKey | null;
  dir: SortDir;
  onSortChange: (sort: SortKey | null, dir: SortDir) => void;
}

const severityBadge: Record<Severity, string> = {
  critical: "bg-red-500/15 text-red-700 dark:text-red-400 border border-red-500/50",
  high: "bg-orange-500/15 text-orange-700 dark:text-orange-400 border border-orange-500/50",
  medium: "bg-amber-500/15 text-amber-700 dark:text-amber-400 border border-amber-500/50",
  low: "bg-zinc-500/15 text-zinc-700 dark:text-zinc-400 border border-zinc-500/50",
};

// Shared 12-column grid so the header and every row line up exactly.
const GRID = "grid grid-cols-[7rem_8rem_minmax(8rem,1fr)_minmax(10rem,2fr)] items-center gap-3 px-4";

const columnHelper = createColumnHelper<Finding>();

const columns = [
  columnHelper.accessor("severity", {
    id: "severity",
    header: "Severity",
    // Order by weight (critical → low) rather than alphabetically.
    sortingFn: (a, b) =>
      SEVERITY_RANK[a.original.severity] - SEVERITY_RANK[b.original.severity],
    cell: (ctx) => {
      const s = ctx.getValue() as Severity;
      return (
        <span
          className={`inline-block rounded px-2 py-0.5 text-xs font-semibold uppercase tracking-wide ${severityBadge[s]}`}
        >
          {s}
        </span>
      );
    },
  }),
  columnHelper.accessor("category", {
    id: "code",
    header: "Code",
    cell: (ctx) => (
      <span className="truncate text-sm font-medium text-zinc-700 dark:text-zinc-300">
        {ctx.getValue()}
      </span>
    ),
  }),
  columnHelper.accessor("location", {
    id: "location",
    header: "File:Line",
    cell: (ctx) => (
      <span className="truncate font-mono text-xs text-zinc-600 dark:text-zinc-400">
        {ctx.getValue()}
      </span>
    ),
  }),
  columnHelper.accessor("title", {
    id: "message",
    header: "Message",
    cell: (ctx) => (
      <span className="truncate text-sm text-zinc-800 dark:text-zinc-200">
        {ctx.getValue()}
      </span>
    ),
  }),
];

function SortIndicator({ dir }: { dir: false | SortDir }) {
  return (
    <span aria-hidden className="ml-1 inline-block w-3 text-xs">
      {dir === "asc" ? "▲" : dir === "desc" ? "▼" : ""}
    </span>
  );
}

/**
 * Virtualized findings table (handles hundreds of rows). Sorting is controlled
 * by the caller (URL-backed) via TanStack Table; rows are windowed with TanStack
 * Virtual. Rows with a snippet/suggestion expand inline to show detail, using
 * dynamic measurement so the virtualizer stays accurate.
 */
export function FindingsTable({ rows, sort, dir, onSortChange }: FindingsTableProps) {
  // TanStack Table's useReactTable returns a mutable instance the React Compiler
  // can't memoize; opt this component out per TanStack's guidance.
  "use no memo";

  const sorting: SortingState = useMemo(
    () => (sort ? [{ id: sort, desc: dir === "desc" }] : []),
    [sort, dir]
  );

  const handleSortingChange: OnChangeFn<SortingState> = useCallback(
    (updater) => {
      const next = typeof updater === "function" ? updater(sorting) : updater;
      if (next.length === 0) {
        onSortChange(null, "desc");
      } else {
        onSortChange(next[0].id as SortKey, next[0].desc ? "desc" : "asc");
      }
    },
    [sorting, onSortChange]
  );

  const table = useReactTable({
    data: rows,
    columns,
    state: { sorting },
    onSortingChange: handleSortingChange,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
  });

  const sortedRows = table.getRowModel().rows;

  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const toggleExpanded = useCallback((id: string) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const scrollRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: sortedRows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 48,
    overscan: 12,
  });

  const hasDetail = (f: Finding) => Boolean(f.snippet || f.suggestion);

  return (
    <div className="overflow-hidden rounded-lg border border-zinc-200 dark:border-zinc-800 theme-high-contrast:border-white">
      {/* Header */}
      <div
        role="row"
        className={`${GRID} border-b border-zinc-200 dark:border-zinc-800 theme-high-contrast:border-white bg-zinc-50 dark:bg-zinc-900 theme-high-contrast:bg-black py-2`}
      >
        {table.getHeaderGroups()[0].headers.map((header) => {
          const sorted = header.column.getIsSorted();
          return (
            <div
              key={header.id}
              role="columnheader"
              aria-sort={
                sorted === "asc"
                  ? "ascending"
                  : sorted === "desc"
                    ? "descending"
                    : "none"
              }
            >
              <button
                type="button"
                onClick={header.column.getToggleSortingHandler()}
                className="flex w-full items-center text-left text-xs font-semibold uppercase tracking-wide text-zinc-500 dark:text-zinc-400 theme-high-contrast:text-white hover:text-zinc-900 dark:hover:text-zinc-100"
              >
                {flexRender(header.column.columnDef.header, header.getContext())}
                <SortIndicator dir={sorted} />
              </button>
            </div>
          );
        })}
      </div>

      {/* Virtualized body */}
      <div
        ref={scrollRef}
        className="max-h-[60vh] overflow-auto"
        role="rowgroup"
        aria-label="Findings"
      >
        <div
          style={{ height: `${virtualizer.getTotalSize()}px`, position: "relative" }}
        >
          {virtualizer.getVirtualItems().map((virtualItem) => {
            const row: Row<Finding> = sortedRows[virtualItem.index];
            const f = row.original;
            const isExpanded = expanded.has(f.id);
            const expandable = hasDetail(f);

            return (
              <div
                key={f.id}
                ref={virtualizer.measureElement}
                data-index={virtualItem.index}
                className="absolute left-0 top-0 w-full border-b border-zinc-100 dark:border-zinc-800/60"
                style={{ transform: `translateY(${virtualItem.start}px)` }}
              >
                <div
                  role="row"
                  className={`${GRID} py-2.5 ${
                    expandable
                      ? "cursor-pointer hover:bg-zinc-50 dark:hover:bg-zinc-900/60"
                      : ""
                  }`}
                  onClick={expandable ? () => toggleExpanded(f.id) : undefined}
                  aria-expanded={expandable ? isExpanded : undefined}
                >
                  {row.getVisibleCells().map((cell) => (
                    <div key={cell.id} className="min-w-0" role="cell">
                      {flexRender(cell.column.columnDef.cell, cell.getContext())}
                    </div>
                  ))}
                </div>

                {expandable && isExpanded && (
                  <div className="px-4 pb-4 pl-[7.75rem] text-sm">
                    {f.suggestion && (
                      <p className="mb-2 italic text-zinc-600 dark:text-zinc-400">
                        💡 {f.suggestion}
                      </p>
                    )}
                    {f.snippet && (
                      <CodeSnippet code={f.snippet} highlightLine={f.line} />
                    )}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
