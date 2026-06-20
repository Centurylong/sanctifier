# Sanctity Score Algorithm

The Sanctity Score is an aggregate metric (0-100) that represents the overall trust and security level of a Soroban smart contract.

## Mathematical Model

The score is calculated using a weighted average of three primary indices:

1.  **Security Index (50%)**: Absence of known vulnerabilities found via static analysis.
2.  **Verification Index (30%)**: Coverage and results of formal verification (Kani proofs).
3.  **Coverage Index (20%)**: Test coverage percentage.

### 1. Security Index (SC)
Starts at 100. Deductions are capped at 100 (score cannot be negative).

| Severity | Issues | Deduction | Max Cap |
| :--- | :--- | :--- | :--- |
| **Critical** | Reentrancy Risks | -20 per hit | -40 |
| **High** | Auth Gaps, Arithmetic Overflows, Panics | -15/10/5 per hit | -30 |
| **Medium** | Upgrade safety, Ledger Size Exceeded | -5 / -10 per hit | -15 / -20 |
| **Low** | Deprecated APIs, Approaching Size Limit, Custom Rules | -2 per hit | -10 |

### 2. Verification Index (VC)
Calculated as the percentage of proven assertions out of total assertions in the formal verification suite.
`VC = (Proven Assertions / Total Assertions) * 100`

### 3. Coverage Index (CC)
Calculated as the percentage of code covered by unit and integration tests.
`CC = (Test Coverage Ratio) * 100`

## Final Formula

$$SanctityScore = (SC \times 0.5) + (VC \times 0.3) + (CC \times 0.2)$$

## Interpretations

*   **80 - 100 (Green)**: High Sanctity. The contract is well-tested, proven, and has no significant static analysis hits.
*   **50 - 79 (Yellow)**: Medium Sanctity. Some warnings or lack of full proof/coverage.
*   **0 - 49 (Red)**: Low Sanctity. Significant risks or lacks fundamental assurance metrics.
