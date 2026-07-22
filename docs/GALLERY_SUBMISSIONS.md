# Adopters & Findings Gallery: Submission Guidelines

This document provides clear guidelines for submitting projects to the **Adopters** list and contributing **Findings** to the Sanctifier Gallery.

---

## 📝 For Projects: Joining the Adopters List

### Why Submit?

- **Social Proof**: Be recognized as a security-conscious Soroban project
- **Community Trust**: Show that your contracts are actively monitored
- **Visibility**: Gain exposure in the Sanctifier ecosystem
- **Security Signal**: Demonstrate commitment to responsible security practices

### Submission Requirements

To be listed as an adopter, your project must:

1. **Be an Active Soroban Smart Contract Project**
   - Deployed or in active development
   - Public repository (GitHub preferred)

2. **Use Sanctifier in Your Workflow**
   - At least one completed scan with Sanctifier
   - Ideally: integrated into CI/CD pipeline or pre-deployment checklist

3. **Be Willing to Share Impact Data** (optional but encouraged)
   - Number of vulnerabilities discovered
   - Severity distribution
   - Patch timelines
   - Does not require disclosing specific findings

4. **Support Responsible Disclosure**
   - If you discover vulnerabilities via Sanctifier, commit to:
     - Patching responsibly before public disclosure
     - Minimal 14-day disclosure window for critical issues
     - Crediting Sanctifier in security advisories (optional)

### How to Submit

#### Option 1: Open an Issue (Easiest)

Click here to open a pre-filled issue:
[**Submit Adopter Application →**](https://github.com/OluRemiFour/sanctifier/issues/new?template=adopter-submission.yml)

Fill out the template with:

- Project name
- Repository URL
- Brief description (1-2 sentences)
- Number of Sanctifier scans completed
- Vulnerabilities found (count and severity breakdown, if public)
- Contact email for future updates

#### Option 2: Submit a PR

1. Fork the repository
2. Update `data/adopters.json`:
   ```json
   {
     "id": "your-project-slug",
     "name": "Your Project Name",
     "repository": "https://github.com/your-org/your-repo",
     "description": "Brief description of your project",
     "category": "defi|infrastructure|governance|nft|other",
     "findings_count": 0,
     "vulnerabilities_found": [],
     "date_added": "2024-07-22",
     "logo_url": "https://your-domain/logo.png",
     "verified": false
   }
   ```
3. Submit the PR with:
   - Clear commit message: `Add [Your Project] to adopters gallery`
   - Link to your Sanctifier scan in PR description (can be private/obfuscated)

### Verification Process

1. Maintainers will review your submission
2. We may ask for:
   - Evidence of Sanctifier usage (redacted scan output)
   - Confirmation of responsible disclosure commitment
   - Public project details validation
3. Once verified:
   - `"verified": true` added to your entry
   - ✅ badge added to gallery listing
   - Featured in gallery homepage

### Staying Current

Once listed, you can keep your entry updated by:

- Opening an issue with tag `[gallery-update]`
- Submitting a PR to `data/adopters.json` with updated findings count
- Notifying maintainers via email

**Example update scenario:**

```
Project: SoroSwap DEX
Old: findings_count: 8
New: findings_count: 12 (found 4 more issues, all patched)
```

---

## 🔍 For Researchers: Contributing Findings

### Types of Contributions Welcome

#### 1. **Featured Vulnerability Cases** (Highest Priority)

- Real vulnerability discovered via Sanctifier in production/testnet code
- Responsibly disclosed and patched
- Educational value and community impact
- Requirements:
  - Complete responsible disclosure timeline documented
  - Clear technical explanation of the bug
  - Patch/fix code example
  - Proof of discovery via Sanctifier

#### 2. **Detector Improvements**

- New vulnerability class covered by Sanctifier
- False positive reports and fixes
- Detector performance optimizations
- See: [Detector Cookbook](detector-cookbook.md)

#### 3. **Case Studies**

- Post-mortem: "How Sanctifier Prevented Exploit XYZ"
- Integration story: "We saved $XXX by scanning before deployment"
- Multi-finding analysis: How interconnected vulnerabilities compound

### Responsible Disclosure Process

If you've discovered a vulnerability using Sanctifier:

#### Step 1: Prepare Your Report (CONFIDENTIAL)

Document:

- **Vulnerability Details**
  - What Sanctifier finding code (S001-S007)
  - Severity and CVSS score
  - Affected contract/function
  - Root cause analysis

- **Timeline**
  - Date discovered
  - Date reported to project team
  - Expected patch date (14+ days for critical)
  - Target public disclosure date (30+ days after patch)

- **Impact Assessment**
  - How would an attacker exploit this?
  - What's the financial impact?
  - Are there known exploits?
  - How many contracts affected?

- **Proof of Concept** (if applicable)
  - Minimal code showing the bug
  - Before/after patch comparison
  - Test case demonstrating the fix

#### Step 2: Report to Project Team (CONFIDENTIAL)

1. **Contact the project directly**
   - Email security contact (check SECURITY.md or GitHub security tab)
   - Use PGP encryption if available
   - Include: title, CVSS, brief description

2. **Allow response time**
   - Critical issues: 5-7 days for project to acknowledge
   - High issues: 7-10 days
   - Medium issues: 10-14 days

3. **Coordinate timeline**
   - Agree on patch deployment date
   - Coordinate public disclosure date
   - Discuss credit/attribution

#### Step 3: Submit to Sanctifier Gallery

**After public disclosure is coordinated**, submit your finding:

##### Option A: Confidential Submission (Before Disclosure)

1. Email `security@sanctifier.dev` with:

   ```
   Subject: [CONFIDENTIAL] Featured Finding Submission

   - GitHub issue number (if any)
   - Project being reported to
   - Timeline to disclosure
   - Expected submission date to gallery
   - Your contact info
   ```

2. We'll keep it confidential until public disclosure date

##### Option B: Public Submission (After Disclosure)

1. Prepare your finding in JSON format:

   ```json
   {
     "id": "SOB-YYYY-NNN-projectslug",
     "vulnerability_id": "SOB-YYYY-NNN",
     "title": "Vulnerability Title",
     "severity": "critical|high|medium",
     "cvss": 8.5,
     "project": "project-slug",
     "project_name": "Full Project Name",
     "detected_by": "Sanctifier",
     "detection_date": "2024-07-22T10:30:00Z",
     "disclosure_date": "2024-07-28T16:00:00Z",
     "status": "disclosed_and_patched",
     "description": "Detailed technical explanation...",
     "impact": "Financial or security impact description",
     "finding_code": "S006",
     "detection_category": "unsafe_pattern",
     "patch_summary": "How the issue was fixed",
     "timeline": {
       "detected": "2024-07-22T10:30:00Z",
       "reported_to_team": "2024-07-22T11:00:00Z",
       "team_acknowledged": "2024-07-22T15:00:00Z",
       "patch_deployed": "2024-07-27T12:00:00Z",
       "public_disclosure": "2024-07-28T16:00:00Z"
     },
     "references": [
       {
         "title": "Security Advisory Link",
         "url": "https://..."
       }
     ]
   }
   ```

2. Submit via PR to `data/findings-showcase.json`

3. Create an accompanying markdown case study in `docs/cases/` explaining:
   - The vulnerability in plain English
   - Why it matters for Soroban developers
   - How to detect similar issues
   - How to fix it

### Featured Finding Template

Create a detailed case study document (markdown):

```markdown
# Case Study: [Vulnerability Title]

**Project**: [Name]  
**Finding ID**: SOB-YYYY-NNN  
**Severity**: [Critical/High/Medium]  
**CVSS**: X.X  
**Discoverer**: [Your name/org]

## The Vulnerability

[Technical explanation]

## How Sanctifier Detected It
```

[Sanctifier output showing the detection]

````

## Impact

[What could go wrong if not fixed]

## The Fix

```rust
[Before code]
→ [After code]
````

## Timeline

- **Detected**: [Date]
- **Reported**: [Date]
- **Patched**: [Date]
- **Disclosed**: [Date]

## References

- [Project Security Advisory](link)
- [GitHub Patch PR](link)

```

### Attribution & Credit

Contributors will be credited as:

- **In the gallery**: Name/organization linked to submission
- **In case study**: "Discovered by [Your Name]"
- **In CHANGELOG**: Featured findings listed in release notes
- **Optional**: Badge/icon on project README if desired

### Review Process

1. Submission reviewed for completeness and accuracy
2. Fact-checking against public disclosures
3. Technical review for clarity
4. 2-5 business days to approval
5. Featured in next gallery update

---

## 📋 Submission Checklists

### ✅ Adopter Submission Checklist

- [ ] Project has GitHub repository (or equivalent)
- [ ] Sanctifier has been run at least once
- [ ] Can provide redacted evidence of scan (or permission for maintainers to verify)
- [ ] Project follows responsible disclosure (if applicable)
- [ ] Filled out issue template or PR with required fields
- [ ] Provided accurate project description and category

### ✅ Finding Submission Checklist

- [ ] Vulnerability is responsibly disclosed or soon to be
- [ ] Have written permission from project to submit (if still confidential)
- [ ] Prepared complete technical explanation
- [ ] Included discovery timeline
- [ ] Documented the Sanctifier finding code used
- [ ] Created before/after code comparison
- [ ] Formatted JSON and markdown according to templates
- [ ] Included all references and links

---

## 🤝 Code of Conduct

When submitting to the gallery, please:

1. **Be Responsible**: Follow disclosure timelines; don't publish before coordinating
2. **Be Respectful**: Credit the projects you discovered issues in
3. **Be Accurate**: Verify technical claims before submission
4. **Be Helpful**: Provide clear explanations to help others learn from findings
5. **Be Collaborative**: Work with maintainers for accuracy and clarity

---

## ❓ FAQ

**Q: Can I submit an issue my company patched internally without public disclosure?**
A: No. All featured findings must be publicly disclosed. Private issues are not published.

**Q: How long until my submission appears?**
A: Typically 2-5 business days for review + approval. We'll keep you updated in the PR/issue.

**Q: Can I submit anonymously?**
A: Yes. We'll use pseudonym or "Anonymous Security Researcher" if requested. Email us to arrange.

**Q: What if someone else submits the same finding?**
A: First credible submission gets credit. We may note multiple discoverers in edge cases.

**Q: Do I need to provide a PoC exploit?**
A: Not required for listing, but strongly encouraged. Educational value is higher with PoC.

**Q: Can I update my submission after listing?**
A: Yes! Submit updates via PR or issue. We keep the gallery current.

---

## 📞 Contact

- **Gallery submissions**: Open issue or PR on GitHub
- **Confidential findings**: [security@sanctifier.dev](mailto:security@sanctifier.dev)
- **Questions**: GitHub Discussions or project Discord

---

**Thank you for securing the Soroban ecosystem! 🛡️**
```
