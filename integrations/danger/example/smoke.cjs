"use strict";
const assert = require("node:assert/strict");
const report = require("./sample-report.json");
const { reportSanctifierFindings, collectFindings } = require("../sanctifier-danger.cjs");
const calls = { fail: [], warn: [], message: [], markdown: [] };
const actions = Object.fromEntries(Object.entries(calls).map(([key, entries]) => [key, (...args) => entries.push(args)]));
const result = reportSanctifierFindings(report, actions, {
  changedFiles: ["contracts/example/src/lib.rs"], maxAnnotations: 3
});
assert.deepEqual({ count: result.count, fail: result.fail, warn: result.warn, message: result.message, annotated: result.annotated }, { count: 3, fail: 1, warn: 1, message: 1, annotated: 3 });
assert.equal(calls.fail[0][1], "contracts/example/src/lib.rs");
assert.equal(calls.fail[0][2], 42);
assert.equal(calls.markdown.length, 1);
assert.match(calls.markdown[0][0], /baseline-suppressed/);
assert.throws(() => collectFindings({ metadata: { format: "unknown" }, findings: {} }), /successful Sanctifier/);
// A high-severity group can occur after many earlier warnings in the JSON schema.
// Annotation capping must not turn that scan into a pass.
const capped = JSON.parse(JSON.stringify(report));
capped.findings.panic_issues = [];
capped.findings.custom_rules = [];
capped.findings.ledger_size_warnings = Array.from({length: 15}, (_, i) => ({ code: "LEDGER_SIZE_RISK", struct_name: "Risk" + i, level: "warning" }));
capped.findings.smt_issues = [{ code: "SMT_INVARIANT_VIOLATION", description: "Critical invariant", location: "contracts/example/src/lib.rs:48" }];
capped.vulnerability_db_matches = [];
const cap = { fail: [], warn: [], message: [], markdown: [] };
const capActions = Object.fromEntries(Object.entries(cap).map(([k, list]) => [k, (...args) => list.push(args)]));
reportSanctifierFindings(capped, capActions, { maxAnnotations: 1 });
assert.equal(cap.fail.length, 1);
assert.equal(cap.warn.length, 0);
console.log("Sanctifier Danger example smoke: PASS (counts, inline anchors, baseline summary, malformed rejection, high-severity budget)");
