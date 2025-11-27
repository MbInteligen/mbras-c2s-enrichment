# GitHub Repository Setup Guide

This guide contains all the specifications and configurations to properly showcase this project on GitHub.

## Repository Description

Update the repository description on GitHub with:

```
🦀 Production-grade Rust API for automated lead enrichment with Contact2Sale integration. Features Work API & Diretrix data enrichment, intelligent caching (98% faster), property-based testing, and live Swagger UI documentation. 100/100 code quality. Deployed on Fly.io.
```

## Repository Settings

### About Section

1. Go to repository settings → "About" section (top right)
2. Set **Website**: `https://mbras-c2s.fly.dev/docs`
3. Add **Topics** (click "Add topics"):
   ```
   rust
   axum
   lead-enrichment
   crm
   contact2sale
   api
   postgresql
   swagger
   openapi
   property-based-testing
   proptest
   flyio
   async
   tokio
   real-estate
   data-enrichment
   webhook
   performance
   caching
   ```

### General Settings

Navigate to Settings → General:

- ✅ **Issues**: Enable issues
- ✅ **Projects**: Enable projects
- ❌ **Wiki**: Disable (we use `/docs` instead)
- ❌ **Sponsorships**: Disable
- ❌ **Discussions**: Disable (can enable later if needed)
- ✅ **Allow merge commits**: Enable
- ✅ **Allow squash merging**: Enable
- ✅ **Allow rebase merging**: Enable
- ✅ **Automatically delete head branches**: Enable

### Security & Analysis

Navigate to Settings → Code security and analysis:

- ✅ **Dependency graph**: Enable
- ✅ **Dependabot alerts**: Enable
- ✅ **Dependabot security updates**: Enable
- ⚠️ **Code scanning**: Optional (requires GitHub Advanced Security)

### Branch Protection Rules

Navigate to Settings → Branches → Add branch protection rule:

**Branch name pattern**: `main`

Enable:
- ✅ Require a pull request before merging
  - Required approvals: 1
  - Dismiss stale pull request approvals when new commits are pushed
- ✅ Require status checks to pass before merging
  - ✅ Require branches to be up to date before merging
  - Required status checks:
    - `Test`
    - `Lint`
    - `Build`
- ⚠️ Require conversation resolution before merging (optional)
- ⚠️ Include administrators (optional - recommended for team projects)

### Secrets Configuration

Navigate to Settings → Secrets and variables → Actions:

Add repository secrets (for CI/CD):
- `CODECOV_TOKEN` (if using Codecov)

**Note**: Database credentials and API keys should NOT be stored in GitHub secrets for this project since tests use mocks.

## GitHub Actions Workflows

All workflow files are already created in `.github/workflows/`:

- ✅ `rust.yml` - Main CI/CD pipeline with:
  - Test job (runs all tests)
  - Lint job (clippy + rustfmt)
  - Build job (release build)
  - Security audit job
  - Code coverage job

These will automatically run on:
- Every push to `main`
- Every pull request to `main`

## Issue Templates

Created templates in `.github/ISSUE_TEMPLATE/`:

- ✅ `bug_report.md` - For bug reports
- ✅ `feature_request.md` - For feature requests

## Pull Request Template

Created in `.github/PULL_REQUEST_TEMPLATE.md`:

- Standardized PR format
- Checklist for code quality
- Performance impact assessment

## Social Preview Image (Optional)

Create a custom social preview image (1280x640px) with:

**Content**:
- Title: "MBRAS C2S Enrichment API"
- Subtitle: "Production-Ready Rust Lead Enrichment Pipeline"
- Key features:
  - 🦀 Rust + Axum
  - ⚡ 98% Cache Performance
  - 🎯 100/100 Quality Score
  - 📚 Live Swagger UI
- Tech stack logos: Rust, PostgreSQL, Fly.io
- Background: Dark gradient (e.g., #1e293b to #0f172a)

**Upload**:
1. Go to Settings → General
2. Scroll to "Social preview"
3. Click "Edit" and upload image

**Tools to create**:
- Canva (free): https://www.canva.com
- Figma (free): https://www.figma.com
- Photopea (free, online): https://www.photopea.com

## Release Creation

### Create v1.0.0 Release

1. Go to Releases → "Draft a new release"
2. Click "Choose a tag" → Create new tag: `v1.0.0`
3. Release title: `🎉 v1.0.0 - Production Ready`
4. Copy content from `CHANGELOG.md` (the v1.0.0 section)
5. Check "Set as the latest release"
6. Click "Publish release"

## README Enhancements

The README.md already includes:
- ✅ Badges at the top
- ✅ Comprehensive feature list
- ✅ Architecture diagram
- ✅ Quick start guide
- ✅ API endpoints with Swagger UI link
- ✅ Testing instructions
- ✅ Deployment guide
- ✅ Performance metrics
- ✅ Code quality standards

## Additional Recommendations

### 1. Add GitHub Actions Status Badge (Optional)

Add to README.md after existing badges:

```markdown
[![CI](https://github.com/MbInteligen/mbras-c2s-enrichment/actions/workflows/rust.yml/badge.svg)](https://github.com/MbInteligen/mbras-c2s-enrichment/actions/workflows/rust.yml)
```

### 2. Enable GitHub Pages (Optional)

If you want to host documentation:

1. Go to Settings → Pages
2. Source: Deploy from a branch
3. Branch: `main` → `/docs`
4. Click Save

### 3. Add Labels

Go to Issues → Labels and ensure these exist:
- `bug` (red)
- `enhancement` (blue)
- `documentation` (light blue)
- `good first issue` (green)
- `help wanted` (purple)
- `performance` (orange)
- `question` (pink)
- `wontfix` (gray)

### 4. Create Project Board (Optional)

For tracking work:
1. Go to Projects → New project
2. Choose "Board" template
3. Add columns: Backlog, In Progress, In Review, Done
4. Link issues and PRs

## Verification Checklist

After setup, verify:

- [ ] Repository description is set
- [ ] Website URL points to Swagger UI
- [ ] All topics are added
- [ ] Branch protection rules are active
- [ ] GitHub Actions workflows are running
- [ ] Issue templates are working
- [ ] PR template appears on new PRs
- [ ] README badges are displaying correctly
- [ ] CONTRIBUTING.md is accessible
- [ ] CHANGELOG.md is up to date
- [ ] Release v1.0.0 is published

## Maintenance

### Regular Tasks

**Weekly**:
- Review Dependabot alerts
- Check GitHub Actions logs for failures
- Respond to new issues

**Monthly**:
- Update dependencies: `cargo update`
- Review performance metrics
- Update documentation if needed

**Per Release**:
- Update CHANGELOG.md
- Create GitHub release
- Update version in Cargo.toml
- Deploy to production
- Announce in relevant channels

## Support Resources

- **GitHub Docs**: https://docs.github.com
- **GitHub Actions**: https://docs.github.com/en/actions
- **Dependabot**: https://docs.github.com/en/code-security/dependabot
- **Branch Protection**: https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches

---

**Last Updated**: 2025-11-23  
**Repository**: https://github.com/MbInteligen/mbras-c2s-enrichment
