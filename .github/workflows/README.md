# GitHub Actions CI/CD Workflows

This directory contains automated workflows for continuous integration and deployment.

## Workflows

### 1. CI Workflow (`ci.yml`)

Runs on every push and pull request to `main` and `develop` branches.

**Jobs:**
- **Test**: Runs all unit tests and integration tests
- **Lint**: Checks code formatting (rustfmt) and runs clippy linter
- **Build**: Builds both debug and release binaries
- **Security**: Runs cargo-audit for dependency vulnerabilities

**Requirements:**
- Rust nightly toolchain
- All tests must pass
- Code must be formatted with `cargo fmt`
- No clippy warnings allowed

### 2. Deploy Workflow (`deploy.yml`)

Runs on every push to `main` branch or manual trigger.

**Jobs:**
- **Deploy**: Deploys to Fly.io and runs smoke tests

**Requirements:**
- `FLY_API_TOKEN` secret must be set in GitHub repository settings
- CI workflow must pass (if integrated)

## Setup Instructions

### 1. Enable GitHub Actions

GitHub Actions are enabled by default for all repositories. No setup needed.

### 2. Add Fly.io API Token (for deployment)

1. Get your Fly.io API token:
   ```bash
   fly auth token
   ```

2. Add to GitHub repository:
   - Go to repository **Settings** → **Secrets and variables** → **Actions**
   - Click **New repository secret**
   - Name: `FLY_API_TOKEN`
   - Value: (paste your token)
   - Click **Add secret**

### 3. Configure Branch Protection (Optional but Recommended)

1. Go to repository **Settings** → **Branches**
2. Click **Add branch protection rule**
3. Branch name pattern: `main`
4. Enable:
   - ✅ Require status checks to pass before merging
   - ✅ Require branches to be up to date before merging
   - Select status checks: `test`, `lint`, `build`
   - ✅ Require linear history
5. Click **Create**

## Local Testing

Before pushing, you can run the same checks locally:

```bash
# Run tests
cargo test

# Check formatting
cargo fmt --all -- --check

# Run clippy
cargo clippy --all-targets --all-features -- -D warnings

# Build release
cargo build --release

# Security audit
cargo install cargo-audit
cargo audit
```

## Workflow Status Badges

Add to your README.md:

```markdown
![CI](https://github.com/YOUR_USERNAME/rust-c2s-api/workflows/CI/badge.svg)
![Deploy](https://github.com/YOUR_USERNAME/rust-c2s-api/workflows/Deploy%20to%20Fly.io/badge.svg)
```

## Troubleshooting

### Tests failing on CI but passing locally

**Cause**: Different Rust versions or missing environment variables

**Solution**:
- Ensure you're using Rust nightly locally: `rustup default nightly`
- Check that all required env vars are mocked in tests

### Clippy warnings on CI

**Cause**: Strict `-D warnings` flag treats warnings as errors

**Solution**:
- Run `cargo clippy --all-targets --all-features -- -D warnings` locally
- Fix all warnings before pushing

### Deployment failing

**Cause**: Invalid or expired FLY_API_TOKEN

**Solution**:
1. Generate new token: `fly auth token`
2. Update GitHub secret
3. Re-run workflow

### Cargo cache issues

**Cause**: Corrupted cache

**Solution**:
- Go to **Actions** tab → Select workflow run → **Re-run jobs** → **Re-run all jobs**
- Or manually delete cache from **Actions** → **Caches**

## Performance Optimization

The workflows use GitHub Actions caching to speed up builds:

- **Cargo registry cache**: Saves downloaded dependencies
- **Cargo index cache**: Saves crate index
- **Target directory cache**: Saves compiled artifacts

Typical build times:
- First run: ~5-8 minutes
- Cached run: ~2-3 minutes

## Cost

GitHub Actions is free for public repositories with generous limits:
- 2,000 minutes/month for private repos
- Unlimited for public repos

Fly.io deployment is free for the first 3 machines.

## Further Improvements

Consider adding:

1. **Code coverage**: Use `cargo-tarpaulin` to generate coverage reports
2. **Performance testing**: Add benchmark jobs with `cargo bench`
3. **Docker build caching**: Use Docker layer caching for faster builds
4. **Staging environment**: Deploy to staging before production
5. **Rollback automation**: Auto-rollback on failed health checks
6. **Slack/Discord notifications**: Alert on deployment success/failure

## Related Documentation

- [GitHub Actions Documentation](https://docs.github.com/en/actions)
- [Fly.io CI/CD Guide](https://fly.io/docs/app-guides/continuous-deployment-with-github-actions/)
- [Rust GitHub Actions](https://github.com/actions-rs)
