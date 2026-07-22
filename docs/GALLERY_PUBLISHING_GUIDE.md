# Gallery Publishing & Maintenance Guide

This guide explains how to publish, update, and maintain the Sanctifier Adopters & Findings Gallery.

## 📊 What's Published

The gallery consists of two main components:

1. **Adopters Gallery** (`data/adopters.json`)
   - Real projects using Sanctifier
   - Verified status and integration details
   - Vulnerability statistics

2. **Featured Findings** (`data/findings-showcase.json`)
   - Real vulnerabilities discovered by Sanctifier
   - Responsibly disclosed and patched
   - Timeline and impact details

## 🚀 Publishing the Gallery

### Frontend (Next.js)

The gallery is published on the web at:
- **URL**: `https://sanctifier.dev/gallery` (or `/gallery` route)
- **Pages**: 
  - `frontend/app/gallery/page.tsx` - Main gallery page
  - `frontend/app/components/AdopterCard.tsx` - Adopter display component
  - `frontend/app/components/FindingCard.tsx` - Finding display component
- **API Routes**:
  - `/api/gallery/adopters` - Adopters data endpoint
  - `/api/gallery/findings` - Findings data endpoint

### Markdown Documentation

Documentation is published in docs:
- `docs/ADOPTERS_AND_FINDINGS.md` - Complete gallery with all details
- `docs/GALLERY_SUBMISSIONS.md` - Guidelines for submitting adopters and findings

### GitHub Issue Template

Submissions are collected via:
- `.github/ISSUE_TEMPLATE/adopter_submission.yml` - Adopter submission form

## 📋 How to Update the Gallery

### Adding a New Adopter

#### Option 1: GitHub Issue (Easiest)
1. Direct projects to the [Adopter Submission Form](https://github.com/OluRemiFour/sanctifier/issues/new?template=adopter_submission.yml)
2. Collect submission details
3. Verify the project uses Sanctifier
4. Add to `data/adopters.json`

#### Option 2: Direct PR Update
1. Edit `data/adopters.json`
2. Add new adopter entry:

```json
{
  "id": "project-slug",
  "name": "Project Name",
  "repository": "https://github.com/org/repo",
  "description": "Brief description",
  "category": "defi|infrastructure|governance|etc",
  "findings_count": 5,
  "vulnerabilities_found": ["SOB-2024-001", "SOB-2024-002"],
  "date_added": "2024-07-22",
  "logo_url": "https://example.com/logo.png",
  "verified": true,
  "notes": "Optional notes"
}
```

3. Update `statistics` object in `adopters.json`
4. Test with: `npm run gallery:validate`
5. Submit PR

### Adding a New Finding

#### Prerequisites
- Vulnerability is **responsibly disclosed**
- **Patch has been deployed** and verified
- **30+ days** have passed since disclosure
- Project team has **coordinated timing**

#### Steps

1. **Prepare finding details**:
   - Complete technical description
   - Patch timeline and dates
   - Impact assessment
   - References and links

2. **Add to `data/findings-showcase.json`**:

```json
{
  "id": "SOB-YYYY-NNN-projectslug",
  "vulnerability_id": "SOB-YYYY-NNN",
  "title": "Vulnerability Title",
  "severity": "critical|high|medium",
  "cvss": 8.5,
  "project": "project-slug",
  "project_name": "Project Name",
  "detected_by": "Sanctifier",
  "detection_date": "2024-07-22T10:30:00Z",
  "disclosure_date": "2024-07-28T16:00:00Z",
  "status": "disclosed_and_patched",
  "description": "Technical explanation...",
  "impact": "Impact description...",
  "finding_code": "S006",
  "detection_category": "unsafe_pattern",
  "patch_summary": "How the issue was fixed...",
  "timeline": {
    "detected": "2024-07-22T10:30:00Z",
    "reported_to_team": "2024-07-22T11:00:00Z",
    "team_acknowledged": "2024-07-22T15:00:00Z",
    "patch_deployed": "2024-07-27T12:00:00Z",
    "public_disclosure": "2024-07-28T16:00:00Z"
  },
  "references": [
    {
      "title": "Security Advisory",
      "url": "https://..."
    }
  ]
}
```

3. **Create case study (optional but encouraged)**:
   - File: `docs/cases/SOB-YYYY-NNN.md`
   - Template: See `docs/GALLERY_SUBMISSIONS.md`
   - Include: Root cause, Sanctifier detection output, fix code

4. **Update statistics** in `data/findings-showcase.json`

5. **Test**: `npm run gallery:validate`

6. **Submit PR** with all updates

## 🔄 Maintenance Tasks

### Weekly
- Monitor GitHub issues for adopter submissions
- Review and respond to submissions
- Process verified submissions

### Monthly
- Run validation script: `./scripts/gallery-maintenance.sh --update`
- Update statistics in both JSON files
- Generate gallery report for analytics
- Review and publish any pending findings

### Quarterly
- Update `ADOPTERS_AND_FINDINGS.md` with latest stats
- Review adoption trends
- Plan featured finding case studies
- Update README gallery section if needed

## 🛠️ Maintenance Scripts

### Validate Gallery Data

```bash
./scripts/gallery-maintenance.sh
```

Checks:
- JSON validity
- Repository URL formats
- Statistics accuracy
- Data consistency

### Update Statistics

```bash
./scripts/gallery-maintenance.sh --update
```

- Updates `last_updated` dates
- Generates gallery report
- Creates backup of data

### Frontend Build & Deploy

```bash
# Development
cd frontend
npm run dev

# Production build
npm run build
npm start
```

The gallery page will be available at `/gallery`

## 🔗 Links to Update

When publishing, ensure these links are updated:

1. **Main README.md**
   - Gallery section with link to `/gallery` route
   - Summary metrics
   - "Recent Highlights" section

2. **Getting Started Guide**
   - Link to gallery for real-world examples
   - Mention adopter communities

3. **Grants/Media Materials**
   - Gallery URL for proof of adoption
   - Stats for impact claims
   - Featured findings for credibility

## 📊 Metrics Dashboard

Key metrics to track:

```
Adopters:
- Total verified adopters
- By category (DeFi, Infrastructure, Governance, etc.)
- By integration type (CI/CD, Pre-deployment, Monitoring)

Findings:
- Total vulnerabilities discovered
- By severity (Critical, High, Medium)
- By detection category
- Total financial impact (when known)
- Average disclosure timeline
```

## 🚨 Responsible Disclosure Checklist

Before publishing a finding:

- [ ] Vulnerability is **real and verified**
- [ ] Project **has been notified confidentially**
- [ ] Project **has deployed a patch**
- [ ] **30+ days** have passed since notification
- [ ] Patch **is live in production/testnet**
- [ ] Public disclosure is **coordinated** with project
- [ ] All details are **technically accurate**
- [ ] Financial impact is **documented** (if applicable)
- [ ] References and links are **working**

## ❓ FAQ

**Q: How often is the gallery updated?**
A: Adopters are added as submissions are verified (typically weekly). Findings are added quarterly or as they reach public disclosure milestone.

**Q: Can projects update their info?**
A: Yes! Projects can open an issue tagged `[gallery-update]` to request changes.

**Q: What if a vulnerability hasn't been fully disclosed yet?**
A: It cannot be published. All findings must be responsibly disclosed before going in the gallery.

**Q: Can we include pre-release or unreleased projects?**
A: No. Only projects with deployed contracts using Sanctifier are eligible.

**Q: Who maintains the gallery?**
A: Core maintainers review submissions and manage publication. Community can contribute via PRs and issues.

## 📞 Support

- **Questions about submission**: Open GitHub issue
- **Confidential findings**: Email `security@sanctifier.dev`
- **Bug reports**: Use GitHub Issues
- **Feedback**: GitHub Discussions
