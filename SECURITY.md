# Security Policy

Sanctifier is a security and formal verification suite for Soroban smart
contracts. We welcome good-faith vulnerability research into Sanctifier itself
and ask researchers to report issues privately so maintainers can investigate,
fix, and coordinate disclosure responsibly.

## Supported Versions

| Version | Supported          |
|---------|--------------------|
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

We take security vulnerabilities in Sanctifier seriously. If you discover a
security issue, please follow the responsible disclosure process outlined below.

### How to Report

1. **Do not** open a public GitHub issue for security vulnerabilities.
2. Open a private GitHub security advisory at
   <https://github.com/Centurylong/sanctifier/security/advisories/new>.
3. If you cannot use GitHub private advisories, email **security@sanctifier.dev**
   with the same information.
4. Include:
   - A clear description of the vulnerability
   - Steps to reproduce the issue
   - The affected component, version, commit, or configuration
   - The potential impact and severity assessment
   - Any proof-of-concept code, logs, screenshots, or sample inputs
   - Any suggested fix or mitigation (optional)

### What to Expect

- **Acknowledgement**: You will receive an acknowledgement within **48 hours** of your report.
- **Assessment**: We will assess the vulnerability and determine its severity within **5 business days**.
- **Status updates**: We will provide an update at least every **7 days** while
  the report is under active investigation.
- **Resolution**: We aim to release a fix within **30 days** of confirming the
  vulnerability, depending on complexity and impact.
- **Disclosure**: We will coordinate public disclosure with you. We request a
  **90-day disclosure window** from the initial report unless there is active
  exploitation, a fix requires more time, or we mutually agree on another
  timeline.

### Severity Levels

| Level    | Description                                              | Response Time |
|----------|----------------------------------------------------------|---------------|
| Critical | Remote code execution, data loss, authentication bypass  | 24 hours      |
| High     | Significant impact on analysis accuracy or data integrity| 3 days        |
| Medium   | Limited impact, requires specific conditions             | 7 days        |
| Low      | Minimal impact, informational                            | 30 days       |

### Scope

The following components are in scope for vulnerability reports:

- **sanctifier-core**: static analysis engine, detector logic, parsers, and
  finding generation
- **sanctifier-cli**: command-line workflows, file handling, report generation,
  update logic, and webhook integrations
- **WASM and SDK packages**: generated WebAssembly bindings, Node/browser SDK
  entry points, and package distribution artifacts
- **Frontend dashboard**: web-based analysis, visualization, upload, export, and
  API routes
- **CI/CD and release automation**: workflows and scripts that build, test,
  publish, or deploy Sanctifier artifacts
- **Runtime guard libraries and examples**: only when the issue affects
  Sanctifier's reusable guard logic, templates, generated guidance, or analysis
  tooling

### Out of Scope

- Vulnerabilities solely in third-party dependencies, unless Sanctifier uses the
  dependency in a way that creates additional impact. Please report dependency
  issues to the relevant maintainers first.
- Issues in intentionally vulnerable example contracts or fixtures, unless they
  demonstrate a flaw in Sanctifier's analysis, templates, or documentation.
- Denial of service caused only by unreasonably large input files, excessive
  request volume, or infrastructure stress testing.
- Spam, phishing, social engineering, or physical attacks against maintainers,
  contributors, or service providers.
- Reports that require access to secrets, credentials, private repositories, or
  systems you do not own or have explicit permission to test.
- Missing security headers, version banners, or informational findings that do
  not create a practical security impact.

### Research Guidelines

When testing Sanctifier, please:

- Use local repositories, test contracts, and accounts you control.
- Minimize access to data that is not yours and stop testing if you encounter
  private information.
- Avoid disrupting hosted services, package distribution, CI/CD, or other users.
- Do not publicly disclose the issue until we have investigated and coordinated
  a disclosure plan.

## Safe Harbor

We consider security research conducted in accordance with this policy to be:

- Authorized and welcome
- Conducted in good faith
- Helpful to the security of Sanctifier and its users
- Eligible for coordinated disclosure and public recognition

If you make a good-faith effort to comply with this policy, we will not initiate
or support legal action against you for the research described in your report.
If a third party takes legal action related to your report, we will make it
known that your research was conducted under this policy.

Safe harbor does not cover activity that intentionally harms users, accesses or
exfiltrates data beyond what is necessary to prove the issue, disrupts service,
or violates the law outside the bounds of good-faith security research.

## Recognition

We appreciate the security research community's efforts. With your permission,
we will acknowledge your contribution in release notes and security advisories.
Sanctifier does not currently operate a paid bug bounty program unless a
separate campaign explicitly states otherwise.
