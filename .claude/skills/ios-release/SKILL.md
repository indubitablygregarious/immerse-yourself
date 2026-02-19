---
name: ios-release
description: Cut an iOS-only TestFlight release — bumps version, pushes to main (no tag), monitors the iOS build workflow. Use when cutting a new iOS/TestFlight build without a desktop release.
allowed-tools: Bash(python3 scripts/desktop-release.py *), Bash(make release-ios*), Bash(gh run list *), Bash(gh run view *)
---

# iOS Release - Cut a TestFlight Build

Delegate to the unified release script with `--ios-only`. This bumps the version in `tauri.conf.json`, commits, pushes to main **without** creating a tag (so only `ios-build.yml` triggers, not `desktop-build.yml`), and monitors the CI build.

## Process

1. Run `make release-ios` (or `python3 scripts/desktop-release.py --ios-only`) from the repo root.
2. The script handles: preflight checks, version bump, commit, push, and CI monitoring.
3. Report the build status to the user when it completes.

## Options

- **Dry run**: `make release-ios-dry-run` to preview without changes.
- **Version bump**: Pass via `RELEASE_ARGS`, e.g., `make release-ios RELEASE_ARGS="--minor"`.
- **Skip monitoring**: `make release-ios RELEASE_ARGS="--no-monitor"`.

## When to Use

Use `make release-ios` for iOS-only TestFlight iterations. Use `make release` when you want both desktop and iOS builds (the tag triggers desktop, the push triggers iOS).
