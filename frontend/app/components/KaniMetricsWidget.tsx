import React from 'react';
import type { KaniVerificationMetrics } from '../types';

interface Props {
    metrics?: KaniVerificationMetrics;
}

export function KaniMetricsWidget({ metrics }: Props) {
    if (!metrics) {
        return null;
    }

    const { total_assertions, proven, failed, unreachable } = metrics;

    // Prevent division by zero if there are no assertions
    const percentage = total_assertions > 0
        ? Math.round((proven / total_assertions) * 100)
        : 0;

    return (
        <div className="rounded-lg border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900 overflow-hidden shadow-sm">
            <div className="border-b border-zinc-200 dark:border-zinc-800 bg-zinc-50/50 dark:bg-zinc-900/50 px-6 py-4">
                <div className="flex items-center gap-2">
                    <span className="text-xl">🛡️</span>
                    <h2 className="text-lg font-semibold text-zinc-900 dark:text-zinc-100">
                        Kani Formal Verification
                    </h2>
                </div>
                <p className="mt-1 text-sm text-zinc-500 dark:text-zinc-400">
                    Mathematical assertions proven vs failed during the last run.
                </p>
            </div>

            <div className="p-6">
                <div className="flex flex-col md:flex-row items-center gap-8">
                    {/* Circular Progress Indicator */}
                    <div className="relative w-32 h-32 flex-shrink-0">
                        <svg className="w-full h-full transform -rotate-90" viewBox="0 0 100 100">
                            {/* Background circle */}
                            <circle
                                cx="50"
                                cy="50"
                                r="40"
                                className="stroke-zinc-100 dark:stroke-zinc-800"
                                strokeWidth="12"
                                fill="none"
                            />
                            {/* Progress circle */}
                            <circle
                                cx="50"
                                cy="50"
                                r="40"
                                className={failed > 0 ? "stroke-red-500" : "stroke-green-500"}
                                strokeWidth="12"
                                fill="none"
                                strokeDasharray="251.2" /* 2 * PI * 40 */
                                strokeDashoffset={251.2 - (251.2 * percentage) / 100}
                                strokeLinecap="round"
                            />
                        </svg>
                        <div className="absolute inset-0 flex flex-col items-center justify-center">
                            <span className="text-2xl font-bold text-zinc-900 dark:text-zinc-100">
                                {percentage}%
                            </span>
                        </div>
                    </div>

                    {/* Metric Stats */}
                    <div className="flex-1 grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4 w-full">
                        <div className="rounded-md border border-zinc-200 dark:border-zinc-800 p-4 text-center">
                            <div className="text-sm font-medium text-zinc-500 dark:text-zinc-400 mb-1">Total Assertions</div>
                            <div className="text-2xl font-bold text-zinc-900 dark:text-zinc-100">{total_assertions}</div>
                        </div>

                        <div className="rounded-md border border-green-200 dark:border-green-900/50 bg-green-50 dark:bg-green-900/10 p-4 text-center">
                            <div className="text-sm font-medium text-green-700 dark:text-green-400 mb-1">Proven Valid</div>
                            <div className="text-2xl font-bold text-green-700 dark:text-green-400">{proven}</div>
                        </div>

                        <div className="rounded-md border border-red-200 dark:border-red-900/50 bg-red-50 dark:bg-red-900/10 p-4 text-center">
                            <div className="text-sm font-medium text-red-700 dark:text-red-400 mb-1">Failed Proofs</div>
                            <div className="text-2xl font-bold text-red-700 dark:text-red-400">{failed}</div>
                        </div>

                        <div className="rounded-md border border-yellow-200 dark:border-yellow-900/50 bg-yellow-50 dark:bg-yellow-900/10 p-4 text-center">
                            <div className="text-sm font-medium text-yellow-700 dark:text-yellow-400 mb-1">Unreachable</div>
                            <div className="text-2xl font-bold text-yellow-700 dark:text-yellow-400">{unreachable}</div>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    );
}
