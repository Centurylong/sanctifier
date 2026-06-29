# Contributor On‑Ramp (One Coherent Start)

Welcome to Sanctifier. This doc exists to keep onboarding consistent: templates, devcontainer, starter pack, and “where to ask what” are all linked here.

## 1) First read
- **Quick start:** [QUICK_START.md](../QUICK_START.md)
- **Full onboarding:** [GETTING_STARTED.md](../GETTING_STARTED.md)
- **How to contribute (main guide):** [CONTRIBUTING.md](../CONTRIBUTING.md)

## 2) Set up your environment
### Option A (recommended): Devcontainer / Codespaces
A `.devcontainer` configuration is included.

- VS Code: Ctrl+Shift+P → “Reopen in Container”
- Codespaces: create a new codespace from the repo page

Bootstrap script (inside the container):
- `.devcontainer/setup.sh`

### Option B: Manual setup
See **Quick start** and run:
- `bash scripts/setup.sh`

## 3) Find your first task
- **Good first issues (full starter pack list):** [.github/GOOD_FIRST_ISSUES.md](../.github/GOOD_FIRST_ISSUES.md)
- **Starter pack tracking meta issue:** [.github/STARTER_PACK_TRACKING.md](../.github/STARTER_PACK_TRACKING.md)

Tip: Don’t guess—open the issue, leave: **“I’d like to work on this.”**, and wait for assignment.

## 4) Where to ask questions
Use GitHub Discussions:
- [Discussions categories](../.github/discussions.yml) (Q&A, Ideas, Show and tell, General)

Rule of thumb:
- Questions → **Discussions (Q&A)**
- Proposals → **Discussions (Ideas)**
- Actionable work → **Issues**

## 5) Roadmap / project planning
- [Public roadmap board](../.github/ROADMAP.md)

Roadmap automation: new issues auto-sync into the board when `ROADMAP_PROJECT_URL` is configured.

## 6) Submitting changes (templates)
### Pull Requests
Use the repo PR template (forces summary, fix/close, motivation, dependencies, testing, checklist):
- `.github/PULL_REQUEST_TEMPLATE.md`

When your PR resolves an issue, include:
- `Fixes #524` / `closes #524` (or any other issue number)

### Issues
Use the issue templates under `.github/ISSUE_TEMPLATE/`:
- Bug report
- Feature request
- Documentation update
- Security (points to `SECURITY.md`)

