"use strict";

// DangerJS adapter for the exact Sanctifier CLI JSON report format.
// No npm runtime dependencies beyond the host application's DangerJS installation.

function shortText(value, max = 220) {
  let t = String(value == null ? "" : value).replace(/[\r\n\t]+/g, " ").replace(/\s{2,}/g, " ").trim();
  return t.length > max ? t.slice(0, max - 1) + "…" : t;
}

function safeLocation(finding) {
  const source = String(finding.location || finding.file || "");
  let file = "";
  let line = Number.isInteger(finding.line) ? finding.line : null;
  const match = source.replace(/\\/g, "/").match(/^(.+?\.rs):(?:line\s*)?(\d+)(?:\b|:)/i);
  if (match) {
    file = match[1];
    line = Number(match[2]);
  } else if (/\.rs$/i.test(source)) {
    file = source.replace(/\\/g, "/");
  } else if (finding.file) {
    file = String(finding.file).replace(/\\/g, "/");
  }
  file = file.replace(/^\.\//, "").trim();
  if (!file || file.startsWith("/") || file.split("/").includes("..") || !/\.rs$/i.test(file)) return null;
  return { file, line: Number.isSafeInteger(line) && line > 0 ? line : null };
}

function severityFor(group, finding) {
  const explicit = String(finding.severity || finding.level || "").toLowerCase();
  if (["critical", "high", "error", "danger"].includes(explicit)) return "fail";
  if (["info", "informational", "low", "note"].includes(explicit)) return "message";
  if (["warning", "medium", "moderate"].includes(explicit)) return "warn";
  return ["auth_gaps", "panic_issues", "arithmetic_issues", "storage_collisions", "unsafe_patterns", "smt_issues"].includes(group)
    ? "fail" : "warn";
}

function describeFinding(group, finding) {
  const code = shortText(finding.code || finding.vuln_id || group.toUpperCase(), 60);
  const title = shortText(
    finding.message || finding.description || finding.suggestion ||
    finding.name || finding.function_name || finding.rule_name ||
    finding.function || finding.issue_type || finding.struct_name ||
    finding.snippet || finding.location || "Potential issue detected",
    280
  );
  return `[${code}] ${title}`;
}

function collectFindings(report) {
  if (!report || typeof report !== "object" || Array.isArray(report) || report.success === false ||
      report.metadata?.format !== "sanctifier-ci-v1" || !report.findings ||
      typeof report.findings !== "object" || Array.isArray(report.findings)) {
    throw new Error("Expected a successful Sanctifier analyze --format json report (sanctifier-ci-v1)");
  }
  const groups = [
    "storage_collisions", "ledger_size_warnings", "unsafe_patterns", "auth_gaps",
    "panic_issues", "arithmetic_issues", "custom_rules", "event_issues",
    "unhandled_results", "upgrade_risks", "smt_issues"
  ];
  const output = [];
  for (const group of groups) {
    const items = report.findings[group] ?? [];
    if (!Array.isArray(items)) throw new Error(`Invalid Sanctifier report: findings.${group} must be an array`);
    for (const item of items) {
      if (!item || typeof item !== "object" || Array.isArray(item)) {
        throw new Error(`Invalid Sanctifier finding in ${group}`);
      }
      output.push({ group, level: severityFor(group, item), text: describeFinding(group, item), at: safeLocation(item) });
    }
  }
  const knownVulnerabilities = report.vulnerability_db_matches ?? [];
  if (!Array.isArray(knownVulnerabilities)) throw new Error("Invalid Sanctifier report: vulnerability_db_matches must be an array");
  for (const item of knownVulnerabilities) {
    if (!item || typeof item !== "object" || Array.isArray(item)) throw new Error("Invalid vulnerability_db_matches item");
    output.push({ group: "vulnerability_db_matches", level: severityFor("vulnerability_db_matches", item),
      text: describeFinding("vulnerability_db_matches", item), at: safeLocation(item) });
  }
  return output;
}

function reportSanctifierFindings(report, actions, options = {}) {
  for (const name of ["fail", "warn", "message", "markdown"]) {
    if (typeof actions?.[name] !== "function") throw new Error(`Danger action ${name} is required`);
  }
  const findings = collectFindings(report);
  const limit = Number.isInteger(options.maxAnnotations) && options.maxAnnotations >= 1 && options.maxAnnotations <= 50
    ? options.maxAnnotations : 12;
  const changed = new Set(
    Array.isArray(options.changedFiles)
      ? options.changedFiles.map(f => String(f).replace(/\\/g, "/").replace(/^\.\//, ""))
      : []
  );
  // Severity-first fanout: a stream of warnings must never hide a later
  // critical/high finding behind the per-PR annotation budget.
  const severityOrder = { fail: 0, warn: 1, message: 2 };
  const ordered = [...findings].sort((a, b) => severityOrder[a.level] - severityOrder[b.level]);
  let posted = 0;
  for (const item of ordered) {
    if (posted >= limit) break;
    const message = `Sanctifier ${item.text}`;
    const action = actions[item.level];
    // Only create inline comments for changed repository files. Other findings
    // are still included in the single top-level summary, without invalid anchors.
    if (item.at?.line && changed.has(item.at.file)) action(message, item.at.file, item.at.line);
    else action(message);
    posted++;
  }
  const totals = { fail: 0, warn: 0, message: 0 };
  for (const item of findings) totals[item.level]++;
  const suppressed = Number(report.baseline?.suppressed_count || 0);
  const displayed = findings.slice(0, 30).map(f => {
    const where = f.at ? `${f.at.file}${f.at.line ? ":" + f.at.line : ""}` : "project";
    const escaped = shortText(f.text, 180).replace(/\|/g, "\\|").replace(/`/g, "'");
    return `| ${f.level} | ${where.replace(/\|/g, "\\|")} | ${escaped} |`;
  });
  const table = displayed.length ? [
    "| Level | Location | Finding |",
    "| --- | --- | --- |",
    ...displayed
  ].join("\n") : "No findings in this report.";
  actions.markdown(
    `### Sanctifier static analysis\n\n` +
    `${findings.length} reported findings (${totals.fail} high/critical, ${totals.warn} warnings, ` +
    `${totals.message} informational); ${Number.isFinite(suppressed) ? suppressed : 0} baseline-suppressed. ` +
    `${posted} Danger annotations sent (maximum ${limit}).\n\n${table}` +
    (findings.length > 30 ? `\n\nShowing first 30 of ${findings.length} findings.` : "")
  );
  return { count: findings.length, annotated: posted, ...totals };
}

function runSanctifierCli(actions, options = {}) {
  const { spawnSync } = require("node:child_process");
  const command = options.command || process.env.SANCTIFIER_BIN || "sanctifier";
  const target = options.projectPath || process.env.SANCTIFIER_TARGET || ".";
  const result = spawnSync(command, ["analyze", target, "--format", "json"], {
    encoding: "utf8",
    shell: false,
    maxBuffer: 16 * 1024 * 1024,
    timeout: 180000
  });
  if (result.error || (result.status !== 0 && result.status !== 1) || !result.stdout) {
    actions.fail(`Sanctifier scan could not complete (status ${result.status ?? "unavailable"}). Check CLI availability and target.`);
    return null;
  }
  let report;
  try { report = JSON.parse(result.stdout); }
  catch { actions.fail("Sanctifier produced non-JSON output; check the CLI version and --format json."); return null; }
  try { return reportSanctifierFindings(report, actions, options); }
  catch (e) {
    actions.fail(`Sanctifier report rejected: ${shortText(e.message, 160)}`);
    return null;
  }
}

module.exports = { collectFindings, reportSanctifierFindings, runSanctifierCli };
