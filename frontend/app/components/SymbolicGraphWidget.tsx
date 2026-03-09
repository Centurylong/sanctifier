import React from "react";
import type { SymbolicGraph, ExecutionPath, PathNode } from "../types";

export function SymbolicGraphWidget({ graphs }: { graphs: SymbolicGraph[] }) {
  if (!graphs || graphs.length === 0) return null;

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold flex items-center gap-2">
          <svg className="w-5 h-5 text-indigo-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 7v10c0 2.21 3.582 4 8 4s8-1.79 8-4V7M4 7c0 2.21 3.582 4 8 4s8-1.79 8-4M4 7c0-2.21 3.582-4 8-4s8 1.79 8 4m0 5c0 2.21-3.582 4-8 4s-8-1.79-8-4" />
          </svg>
          Symbolic Execution Paths
        </h2>
        <span className="text-sm text-zinc-500 dark:text-zinc-400">
          Generated possible logic branches for public functions
        </span>
      </div>

      <div className="grid grid-cols-1 gap-6">
        {graphs.map((graph, gIdx) => (
          <div key={gIdx} className="rounded-xl border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-950/50 p-5 overflow-x-auto">
            <h3 className="font-mono text-md font-semibold mb-4 text-zinc-800 dark:text-zinc-200">
              fn {graph.function_name}()
            </h3>
            
            <div className="flex gap-6 min-w-max pb-2">
              {graph.paths.map((path, pIdx) => (
                <PathLane key={pIdx} path={path} index={pIdx + 1} />
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function PathLane({ path, index }: { path: ExecutionPath; index: number }) {
  const isPanic = path.is_panic;

  return (
    <div className={`flex flex-col w-64 flex-shrink-0 rounded-lg border ${isPanic ? 'border-red-500/30 dark:border-red-500/20 bg-red-50 dark:bg-red-950/10' : 'border-zinc-200 dark:border-zinc-800 bg-zinc-50 dark:bg-zinc-900/50'} p-4`}>
      <div className="flex items-center justify-between mb-4 pb-2 border-b border-zinc-200 dark:border-zinc-800">
        <span className="text-xs font-semibold text-zinc-500 dark:text-zinc-400 uppercase tracking-wider">
          Path {index}
        </span>
        {isPanic ? (
          <span className="text-xs font-medium text-red-600 dark:text-red-400 bg-red-100 dark:bg-red-900/30 px-2 py-0.5 rounded-full flex items-center gap-1">
            <svg className="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
            </svg>
            Panic
          </span>
        ) : (
          <span className="text-xs font-medium text-emerald-600 dark:text-emerald-400 bg-emerald-100 dark:bg-emerald-900/30 px-2 py-0.5 rounded-full flex items-center gap-1">
            <svg className="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
            </svg>
            Safe
          </span>
        )}
      </div>

      <div className="flex flex-col gap-2">
        {path.nodes.length === 0 ? (
          <div className="text-sm italic text-zinc-400 text-center py-4">No operations recorded</div>
        ) : (
          path.nodes.map((node, i) => (
            <React.Fragment key={i}>
              <NodeItem node={node} />
              {i < path.nodes.length - 1 && (
                <div className="flex justify-center py-1 text-zinc-300 dark:text-zinc-700">
                  <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 14l-7 7m0 0l-7-7m7 7V3" />
                  </svg>
                </div>
              )}
            </React.Fragment>
          ))
        )}
      </div>
    </div>
  );
}

function NodeItem({ node }: { node: PathNode }) {
  let bgColor = "bg-white dark:bg-zinc-800";
  let borderColor = "border-zinc-200 dark:border-zinc-700";
  let icon = null;

  if (node.node_type === "condition") {
    bgColor = "bg-orange-50 dark:bg-orange-950/20";
    borderColor = "border-orange-200 dark:border-orange-900/30";
    icon = (
      <svg className="w-3 h-3 text-orange-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8 7h12m0 0l-4-4m4 4l-4 4m0 6H4m0 0l4 4m-4-4l4-4" />
      </svg>
    );
  } else if (node.node_type === "panic") {
    bgColor = "bg-red-50 dark:bg-red-950/20";
    borderColor = "border-red-200 dark:border-red-900/30";
    icon = (
      <svg className="w-3 h-3 text-red-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
      </svg>
    );
  } else {
    icon = (
      <svg className="w-3 h-3 text-zinc-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
      </svg>
    );
  }

  return (
    <div className={`p-2.5 rounded border ${bgColor} ${borderColor} shadow-sm flex flex-col gap-1`}>
      <div className="flex items-start justify-between">
        <div className="flex items-center gap-1.5 font-medium text-xs text-zinc-700 dark:text-zinc-300">
          {icon}
          <span className="truncate">{node.node_type}</span>
        </div>
        <span className="text-[10px] text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-300">
          L{node.line}
        </span>
      </div>
      <div className="text-xs font-mono text-zinc-600 dark:text-zinc-400 break-words line-clamp-2">
        {node.description}
      </div>
    </div>
  );
}
