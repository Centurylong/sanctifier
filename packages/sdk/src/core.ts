import { SanctifierError } from "./errors.js";
import type {
  AnalysisReport,
  AnalyzeOptions,
  Finding,
  FindingCodeEntry,
  SanctifierWasm,
  Severity,
} from "./types.js";

const SEVERITY_ORDER: Record<Severity, number> = {
  info: 0,
  low: 1,
  medium: 2,
  high: 3,
  critical: 4,
};

function severityAtLeast(actual: Severity, threshold: Severity): boolean {
  return SEVERITY_ORDER[actual] >= SEVERITY_ORDER[threshold];
}

// Translate the SDK-facing options into the JSON shape the Rust core expects.
// Anything not provided is left out so the Rust side falls back to defaults.
function buildConfigJson(options: AnalyzeOptions): string {
  const config: Record<string, unknown> = {};

  if (options.ignorePaths) config.ignore_paths = options.ignorePaths;
  if (options.enabledRules) config.enabled_rules = options.enabledRules;
  if (typeof options.ledgerLimit === "number") {
    config.ledger_limit = options.ledgerLimit;
  }
  if (typeof options.approachingThreshold === "number") {
    config.approaching_threshold = options.approachingThreshold;
  }
  if (options.customRules) {
    config.custom_rules = options.customRules.map((rule) => ({
      name: rule.name,
      pattern: rule.pattern,
      severity: rule.severity ?? "warning",
    }));
  }

  return JSON.stringify(config);
}

function ensureReport(value: unknown): AnalysisReport {
  if (value === null || typeof value !== "object") {
    throw new Error(
      "Sanctifier WASM returned an unexpected value. Expected an AnalysisReport object.",
    );
  }
  return value as AnalysisReport;
}

function topFinding(findings: Finding[], threshold: Severity): Finding | undefined {
  for (const f of findings) {
    if (severityAtLeast(f.severity, threshold)) return f;
  }
  return undefined;
}

// Drive the WASM analyzer. This is the shared core used by both the node
// and browser entry points after they finish their environment-specific
// init (loading the binary, etc.).
export function runAnalyze(
  wasm: SanctifierWasm,
  source: string,
  options: AnalyzeOptions = {},
): AnalysisReport {
  if (typeof source !== "string") {
    throw new TypeError("analyze(source): source must be a string of Rust code");
  }

  const hasConfig =
    options.ignorePaths !== undefined ||
    options.enabledRules !== undefined ||
    options.ledgerLimit !== undefined ||
    options.approachingThreshold !== undefined ||
    (options.customRules !== undefined && options.customRules.length > 0);

  const raw = hasConfig
    ? wasm.analyze_with_config(buildConfigJson(options), source)
    : wasm.analyze(source);

  const report = ensureReport(raw);

  if (options.failOn) {
    const offender = topFinding(report.findings, options.failOn);
    if (offender) {
      throw new SanctifierError(
        `Sanctifier found ${report.summary.total} issue(s); at least one at or above ${options.failOn}: ${offender.code} ${offender.message}`,
        report,
        options.failOn,
      );
    }
  }

  return report;
}

export function listFindingCodes(wasm: SanctifierWasm): FindingCodeEntry[] {
  const raw = wasm.finding_code_catalog();
  if (!Array.isArray(raw)) return [];
  return raw as FindingCodeEntry[];
}

export function getCoreVersion(wasm: SanctifierWasm): string {
  return wasm.version();
}
