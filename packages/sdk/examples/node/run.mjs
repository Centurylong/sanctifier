// Minimal end to end example. Run with: node examples/node/run.mjs
import { analyze, SanctifierError, coreVersion } from "../../dist/node/index.js";

const source = `
use soroban_sdk::{contract, contractimpl, Env, Address};

#[contract]
pub struct Token;

#[contractimpl]
impl Token {
    pub fn transfer(env: Env, from: Address, to: Address, amount: u64) {
        let new_balance = amount + 100;
        env.storage().instance().set(&from, &new_balance);
    }
}
`;

console.log("Sanctifier core version:", coreVersion());

try {
  const report = await analyze(source, { failOn: "high" });
  console.log("Clean. No issues at or above 'high'.");
  console.log("Summary:", report.summary);
} catch (err) {
  if (err instanceof SanctifierError) {
    console.log("Found", err.report.summary.total, "issue(s). Breakdown:", err.report.summary);
    for (const finding of err.report.findings) {
      console.log(`  ${finding.code} [${finding.severity}] ${finding.message}`);
      console.log(`      at ${finding.location}`);
    }
    process.exitCode = 1;
  } else {
    throw err;
  }
}
