"use client";

import { useMemo, useEffect, useState } from "react";
import type { Finding, Severity } from "../types";

interface SanctityScoreProps {
  findings: Finding[];
}

const SEVERITY_WEIGHTS: Record<string, number> = {
  critical: 15,
  high: 10,
  medium: 5,
  low: 2,
};

function calculateScore(findings: Finding[]): number {
  let score = 100;
  for (const f of findings) {
    score -= SEVERITY_WEIGHTS[f.severity] ?? 0;
  }
  return Math.max(0, Math.min(100, score));
}

function getGrade(score: number): string {
  if (score >= 90) return "A";
  if (score >= 80) return "B";
  if (score >= 65) return "C";
  if (score >= 50) return "D";
  return "F";
}

function getColorVar(score: number): string {
  if (score >= 76) return "var(--success)";
  if (score >= 61) return "var(--warning)";
  if (score >= 41) return "var(--severity-high)";
  return "var(--severity-critical)";
}

function getSeverityBreakdown(findings: Finding[]) {
  const breakdown: Record<Severity, number> = {
    critical: 0,
    high: 0,
    medium: 0,
    low: 0,
  };

  for (const f of findings) {
    breakdown[f.severity]++;
  }

  return breakdown;
}

export function SanctityScore({ findings }: SanctityScoreProps) {
  const score = useMemo(() => calculateScore(findings), [findings]);
  const grade = getGrade(score);
  const color = getColorVar(score);
  const breakdown = useMemo(() => getSeverityBreakdown(findings), [findings]);

  const [animatedProgress, setAnimatedProgress] = useState(0);

  const radius = 70;
  const strokeWidth = 12;
  const circumference = Math.PI * radius;
  const targetProgress = (score / 100) * circumference;

  // Animate the progress arc
  useEffect(() => {
    const duration = 1000; // 1 second animation
    const startTime = Date.now();
    const animate = () => {
      const elapsed = Date.now() - startTime;
      const progress = Math.min(elapsed / duration, 1);
      
      // Ease-out cubic
      const easeOut = 1 - Math.pow(1 - progress, 3);
      setAnimatedProgress(targetProgress * easeOut);

      if (progress < 1) {
        requestAnimationFrame(animate);
      }
    };

    animate();
  }, [targetProgress]);

  return (
    <div
      className="rounded-lg border p-6"
      style={{
        borderColor: "var(--border)",
        backgroundColor: "var(--card)",
        color: "var(--card-foreground)",
      }}
    >
      <h3 className="text-sm font-semibold mb-4" style={{ color: "var(--muted-foreground)" }}>
        Sanctity Score
      </h3>
      
      {/* Visual gauge */}
      <div className="flex items-center justify-center mb-6">
        <svg
          viewBox="0 0 180 110"
          className="w-full h-auto max-w-[180px]"
          role="img"
          aria-label={`Security score: ${score} out of 100, grade ${grade}`}
        >
          {/* Background arc */}
          <path
            d={`M ${90 - radius} 95 A ${radius} ${radius} 0 0 1 ${90 + radius} 95`}
            fill="none"
            stroke="var(--muted)"
            strokeWidth={strokeWidth}
            strokeLinecap="round"
          />
          {/* Animated progress arc */}
          <path
            d={`M ${90 - radius} 95 A ${radius} ${radius} 0 0 1 ${90 + radius} 95`}
            fill="none"
            stroke={color}
            strokeWidth={strokeWidth}
            strokeLinecap="round"
            strokeDasharray={`${animatedProgress} ${circumference}`}
            style={{ transition: "stroke-dasharray 0.3s ease" }}
          />
          {/* Score text */}
          <text
            x="90"
            y="75"
            textAnchor="middle"
            fontSize="28"
            fontWeight="bold"
            fill="currentColor"
          >
            {score}
          </text>
          {/* Grade label */}
          <text
            x="90"
            y="95"
            textAnchor="middle"
            fontSize="14"
            fontWeight="600"
            fill={color}
          >
            Grade: {grade}
          </text>
        </svg>
      </div>

      {/* Status text */}
      <p className="text-center text-xs mb-4" style={{ color: "var(--muted-foreground)" }}>
        {score >= 76
          ? "Good security posture"
          : score >= 50
            ? "Moderate risk — review findings"
            : "High risk — immediate attention needed"}
      </p>

      {/* Severity breakdown table (ARIA-compliant) */}
      <div className="mt-4">
        <h4 className="text-xs font-semibold mb-2 sr-only">
          Severity Breakdown
        </h4>
        <table className="w-full text-sm" aria-label="Security findings by severity">
          <thead className="sr-only">
            <tr>
              <th scope="col">Severity</th>
              <th scope="col">Count</th>
            </tr>
          </thead>
          <tbody className="space-y-1">
            {(["critical", "high", "medium", "low"] as Severity[]).map((severity) => (
              <tr key={severity} className="flex justify-between items-center py-1">
                <td className="flex items-center gap-2">
                  <span
                    className="w-3 h-3 rounded-full"
                    style={{ backgroundColor: `var(--severity-${severity})` }}
                    aria-hidden="true"
                  />
                  <span className="capitalize text-xs">{severity}</span>
                </td>
                <td className="font-medium text-xs" style={{ color: "var(--muted-foreground)" }}>
                  {breakdown[severity]}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {/* Screen reader only summary */}
      <div className="sr-only">
        Security score is {score} out of 100 with grade {grade}.
        {breakdown.critical > 0 && ` ${breakdown.critical} critical issues.`}
        {breakdown.high > 0 && ` ${breakdown.high} high severity issues.`}
        {breakdown.medium > 0 && ` ${breakdown.medium} medium severity issues.`}
        {breakdown.low > 0 && ` ${breakdown.low} low severity issues.`}
      </div>
    </div>
  );
}

export { calculateScore };
