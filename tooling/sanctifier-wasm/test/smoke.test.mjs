import assert from "node:assert/strict";
import { test } from "node:test";
import { analyze, analyzeReport, findingCodes, init, version } from "../js/index.js";

const VULNERABLE = `
use soroban_sdk::{contract, contractimpl, Address, Env};

#[contract]
pub struct Vault;

#[contractimpl]
impl Vault {
    pub fn withdraw(env: Env, from: Address, amount: i128) {
        let balance: i128 = env.storage().persistent().get(&from).unwrap();
        env.storage().persistent().set(&from, &(balance - amount));
    }
}
`;

const SEVERITIES = ["critical", "high", "medium", "low", "info"];

test("init resolves and is idempotent under concurrency", async () => {
  // Two callers racing to initialize must share one load, not start two.
  await Promise.all([init(), init()]);
  await init();
  assert.equal(typeof version(), "string");
});

test("analyze returns a Finding[] as documented", async () => {
  await init();
  const findings = analyze(VULNERABLE);
  assert.ok(Array.isArray(findings), "analyze must return an array");
  assert.ok(findings.length > 0, "expected findings for a deliberately vulnerable contract");
  for (const f of findings) {
    assert.equal(typeof f.code, "string");
    assert.equal(typeof f.message, "string");
    assert.ok(SEVERITIES.includes(f.severity), `unexpected severity ${f.severity}`);
  }
});

test("analyzeReport exposes the summary and raw per-detector output", async () => {
  await init();
  const report = analyzeReport(VULNERABLE);
  assert.ok(report.summary, "report.summary missing");
  assert.equal(
    report.summary.total,
    report.findings.length,
    "summary.total must match the findings array",
  );
  assert.ok(report.raw, "report.raw missing");
});

test("arbitrary text does not trap the wasm instance", async () => {
  await init();
  // Source arrives from a browser textarea. A panic traps the instance and
  // takes the whole page's module with it, so every one of these must return.
  for (const source of ["", "not rust {{{", "fn main( {", " "]) {
    assert.doesNotThrow(() => analyze(source), `threw on ${JSON.stringify(source)}`);
  }
});

test("non-string input is rejected before reaching wasm", async () => {
  await init();
  assert.throws(() => analyze(42), TypeError);
});

test("the finding code catalog is populated", async () => {
  await init();
  const codes = findingCodes();
  assert.ok(Array.isArray(codes) && codes.length > 0, "catalog should be a non-empty array");
});

test("analysis of a realistic contract completes well under the 2s demo budget", async () => {
  await init();
  const started = performance.now();
  analyze(VULNERABLE.repeat(8));
  const elapsed = performance.now() - started;
  assert.ok(elapsed < 2000, `analysis took ${elapsed.toFixed(0)}ms, over the 2000ms budget`);
});
