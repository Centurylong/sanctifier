"use client";

import type { SanctityScore } from "../types";

interface Props {
    score: SanctityScore;
}

export function SanctityScoreWidget({ score }: Props) {
    const getScoreColor = (value: number) => {
        if (value >= 80) return "text-emerald-550 dark:text-emerald-400";
        if (value >= 50) return "text-amber-550 dark:text-amber-400";
        return "text-rose-550 dark:text-rose-400";
    };

    const getScoreBg = (value: number) => {
        if (value >= 80) return "bg-emerald-50 dark:bg-emerald-950/30 border-emerald-100 dark:border-emerald-900/50";
        if (value >= 50) return "bg-amber-50 dark:bg-amber-950/30 border-amber-100 dark:border-amber-900/50";
        return "bg-rose-50 dark:bg-rose-950/30 border-rose-100 dark:border-rose-900/50";
    };

    return (
        <div className={`rounded-xl border p-6 ${getScoreBg(score.total_score)} space-y-6`}>
            <div className="flex items-center justify-between">
                <div>
                    <h2 className="text-xl font-bold tracking-tight">Sanctity Score</h2>
                    <p className="text-sm opacity-70">Trust aggregate of analysis, proof, and coverage</p>
                </div>
                <div className={`text-5xl font-black ${getScoreColor(score.total_score)}`}>
                    {score.total_score}
                    <span className="text-lg font-normal opacity-50 ml-1">/100</span>
                </div>
            </div>

            <div className="grid grid-cols-3 gap-4">
                <div className="bg-white/50 dark:bg-black/20 rounded-lg p-3 border border-black/5 dark:border-white/5">
                    <div className="text-xs uppercase tracking-wider font-semibold opacity-60">Security</div>
                    <div className="text-2xl font-bold">{score.security_score}</div>
                </div>
                <div className="bg-white/50 dark:bg-black/20 rounded-lg p-3 border border-black/5 dark:border-white/5">
                    <div className="text-xs uppercase tracking-wider font-semibold opacity-60">Proofs</div>
                    <div className="text-2xl font-bold">{score.verification_score}</div>
                </div>
                <div className="bg-white/50 dark:bg-black/20 rounded-lg p-3 border border-black/5 dark:border-white/5">
                    <div className="text-xs uppercase tracking-wider font-semibold opacity-60">Coverage</div>
                    <div className="text-2xl font-bold">{score.coverage_score}%</div>
                </div>
            </div>

            {score.deductions.length > 0 && (
                <div className="space-y-3">
                    <h3 className="text-sm font-semibold uppercase tracking-wider opacity-60">Deductions</h3>
                    <div className="space-y-2">
                        {score.deductions.map((d, i) => (
                            <div key={i} className="flex items-start justify-between text-sm bg-black/5 dark:bg-white/5 rounded p-2 border border-black/5 dark:border-white/5">
                                <div>
                                    <span className="font-semibold">{d.category}:</span> {d.message}
                                </div>
                                <div className="font-mono text-rose-600 dark:text-rose-400">-{d.amount}</div>
                            </div>
                        ))}
                    </div>
                </div>
            )}
        </div>
    );
}
