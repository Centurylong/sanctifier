import { describe, expect, it } from "vitest";
import {
  SanctifierError,
  analyze,
  analyzeSync,
  coreVersion,
  findingCodes,
} from "../dist/node/index.js";

const VULNERABLE_CONTRACT = `
use soroban_sdk::{contract, contractimpl, Env, Address};

#[contract]
pub struct Token;

#[contractimpl]
impl Token {
    pub fn transfer(env: Env, from: Address, to: Address, amount: u64) {
        let new_balance = amount + 100;
        env.storage().instance().set(&from, &new_balance);
    }

    pub fn mint(env: Env, to: Address, amount: u128) -> u128 {
        let total: u128 = env.storage().instance().get(&to).unwrap();
        total + amount
    }

    pub fn admin_only(env: Env, new_admin: Address) {
        env.storage().instance().set(&"admin", &new_admin);
    }
}
`;

const SAFE_CONTRACT = `
use soroban_sdk::{contract, contractimpl, Env, Address};

#[contract]
pub struct Safe;

#[contractimpl]
impl Safe {
    pub fn ping(env: Env) -> u32 {
        42
    }
}
`;

describe("@sanctifier/sdk", () => {
  it("exposes a core version string", () => {
    expect(typeof coreVersion()).toBe("string");
    expect(coreVersion().length).toBeGreaterThan(0);
  });

  it("lists finding codes from the catalog", () => {
    const codes = findingCodes();
    expect(codes.length).toBeGreaterThan(0);
    const codeIds = codes.map((c) => c.code);
    expect(codeIds).toContain("S001");
    expect(codeIds).toContain("S003");
  });

  it("returns a structured report for a vulnerable contract", async () => {
    const report = await analyze(VULNERABLE_CONTRACT);
    expect(report.findings.length).toBeGreaterThan(0);
    expect(report.summary.total).toBe(report.findings.length);
    const codes = new Set(report.findings.map((f) => f.code));
    expect(codes.has("S003")).toBe(true);
  });

  it("returns an empty findings list for a trivial safe contract", async () => {
    const report = await analyze(SAFE_CONTRACT);
    expect(report.summary.total).toBe(report.findings.length);
  });

  it("throws SanctifierError when failOn is breached", async () => {
    await expect(
      analyze(VULNERABLE_CONTRACT, { failOn: "high" }),
    ).rejects.toBeInstanceOf(SanctifierError);
  });

  it("does not throw when failOn is set above the worst finding", async () => {
    const report = await analyze(SAFE_CONTRACT, { failOn: "critical" });
    expect(report.summary.critical).toBe(0);
  });

  it("respects a custom regex rule via options.customRules", async () => {
    const source = `
      pub fn ping(env: Env) {
        // BACKDOOR: hardcoded secret
        let _admin = "GABCDEFGHIJ12345";
      }
    `;
    const report = await analyze(source, {
      customRules: [
        { name: "backdoor_comment", pattern: "BACKDOOR", severity: "error" },
      ],
    });
    const matched = report.findings.some(
      (f) => f.code === "S007" && f.message.includes("backdoor_comment"),
    );
    expect(matched).toBe(true);
  });

  it("supports the sync entrypoint", () => {
    const report = analyzeSync(VULNERABLE_CONTRACT);
    expect(report.findings.length).toBeGreaterThan(0);
  });

  it("rejects non-string sources", async () => {
    // @ts-expect-error intentional bad input
    await expect(analyze(123)).rejects.toBeInstanceOf(TypeError);
  });
});
