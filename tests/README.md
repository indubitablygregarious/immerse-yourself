# Tests

Test suites for Immerse Yourself.

## Test Directories

| Directory | Description | How to Run |
|-----------|-------------|------------|
| [`e2e/`](./e2e/) | E2E tests and screenshot automation via WebKit Inspector Protocol | `make screenshot` / `make e2e` |

## Quick Reference

```bash
# Rust unit tests (no Docker needed)
make test

# Rust type-check
make check

# Python lint (scripts/ and tests/e2e/)
make lint

# TypeScript type-check
cd rust/immerse-tauri/ui && npx tsc --noEmit

# E2E tests (requires Docker)
make e2e

# Capture project screenshot
make screenshot

# Windows smoke test (GitHub Actions)
make test-windows
```
