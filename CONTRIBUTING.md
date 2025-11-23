# Contributing to MBRAS C2S Enrichment API

Thank you for your interest in contributing! This document provides guidelines for contributing to this project.

## Code of Conduct

- Be respectful and inclusive
- Focus on constructive feedback
- Help maintain our 100/100 code quality standards

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/YOUR_USERNAME/mbras-c2s-enrichment.git`
3. Create a branch: `git checkout -b feature/your-feature-name`
4. Set up environment: `cp .env.example .env` and fill in values

## Development Process

### Before You Start

1. Check existing issues to avoid duplicates
2. Create an issue to discuss major changes
3. Read the [documentation](docs/README.md)

### Making Changes

1. **Code Style**
   - Run `cargo fmt` before committing
   - Run `cargo clippy -- -D warnings` and fix all warnings
   - Follow Rust naming conventions

2. **Testing**
   - Add unit tests for new functionality
   - Add integration tests for API changes
   - Add property tests for validation logic
   - Ensure all tests pass: `cargo test`

3. **Documentation**
   - Add `///` doc comments to all public APIs
   - Update README.md if adding features
   - Update CLAUDE.md for significant changes
   - Add examples to doc comments

4. **Error Handling**
   - Always use `.context()` for database operations
   - Provide clear, actionable error messages
   - Use custom error types when appropriate

### Quality Standards

We maintain a 100/100 code quality score. All contributions must meet these standards:

- ✅ **Error Handling**: Context chains on all DB operations
- ✅ **Testing**: Comprehensive test coverage (unit, integration, property)
- ✅ **Documentation**: Doc comments with examples for all public APIs
- ✅ **Performance**: Sub-100ms response times for interactive endpoints
- ✅ **Code Quality**: Zero clippy warnings, formatted with rustfmt

### Commit Messages

Use conventional commits format:

```
<type>(<scope>): <subject>

<body>

<footer>
```

**Types**:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting, etc.)
- `refactor`: Code refactoring
- `perf`: Performance improvements
- `test`: Adding or updating tests
- `chore`: Build process or tooling changes

**Example**:
```
feat(caching): add Work API response caching

Implements 1-hour TTL cache for Work API responses to improve
performance by 98% on repeated queries.

- Added moka cache dependency
- Created work_api_cache in AppState
- Updated fetch_work_api_data to use cache

Closes #42
```

### Pull Request Process

1. **Create PR** with clear description
2. **Link related issues**
3. **Ensure CI passes**:
   - ✅ All tests pass
   - ✅ Clippy clean
   - ✅ Formatted with rustfmt
   - ✅ No security vulnerabilities
4. **Request review** from maintainers
5. **Address feedback** promptly
6. **Wait for approval** before merging

### Testing Checklist

Before submitting PR:

- [ ] `cargo test` - All tests pass
- [ ] `cargo clippy -- -D warnings` - No warnings
- [ ] `cargo fmt --check` - Properly formatted
- [ ] `cargo audit` - No security issues
- [ ] Documentation updated
- [ ] CHANGELOG.md updated (if applicable)

## Project Structure

```
src/
├── main.rs              # Entry point & routing
├── handlers.rs          # HTTP handlers
├── services.rs          # External API integrations
├── db_storage.rs        # Database operations
├── enrichment.rs        # Core enrichment logic
├── errors.rs            # Error types
└── models.rs            # Data models

docs/                    # Documentation
tests/                   # Integration tests
scripts/                 # Utility scripts
```

## Areas for Contribution

### High Priority
- Redis integration for distributed caching
- Direct C2S webhook implementation
- Additional property-based tests
- Performance optimizations

### Medium Priority
- Additional enrichment data sources
- Enhanced error recovery
- Monitoring and alerting improvements
- API versioning

### Low Priority
- UI for manual enrichment
- Batch processing improvements
- Analytics dashboard

## Getting Help

- Read the [documentation](docs/README.md)
- Check [existing issues](https://github.com/MbInteligen/mbras-c2s-enrichment/issues)
- Review [session notes](docs/session-notes/)
- Ask questions in issues (tag with `question`)

## License

By contributing, you agree that your contributions will be licensed under the project's proprietary license.

## Recognition

Contributors will be recognized in:
- CHANGELOG.md
- GitHub contributors page
- Special acknowledgments for significant contributions

---

Thank you for contributing to making this project better! 🦀
