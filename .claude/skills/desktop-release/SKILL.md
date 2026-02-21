---
name: desktop-release
description: Cut a desktop release by bumping the version, tagging, pushing, and monitoring the GitHub Actions build. Use when the user says "cut a release", "desktop release", "push a release", or "make a release".
allowed-tools: Bash(python3 *), Bash(git *), Bash(gh *), Read
---

# Desktop Release

Cut a new desktop release for all platforms (Linux, macOS, Windows) via GitHub Actions.

This skill delegates to `scripts/desktop-release.py` which handles the entire flow:
version bump, git tag, push, and CI monitoring.

## Process

### Step 1: Check readiness

Run the script in dry-run mode first to show what will happen:

```bash
cd /home/pete/code/immerse-yourself && python3 scripts/desktop-release.py --dry-run
```

Show the user the current version and what the new version will be. If the user provided arguments (like `--minor`, `--major`, or `--version X.Y.Z`), pass them through.

### Step 2: Generate release notes

Before cutting the release, generate release notes from all commits since the last tag:

```bash
cd /home/pete/code/immerse-yourself && git log $(git describe --tags --abbrev=0)..HEAD --pretty=format:"- %s" --no-merges
```

Compose human-readable release notes from these commits:
- Group changes into categories: **Features**, **Fixes**, **Improvements**, **Other** (skip devlog-only and version-bump commits)
- Write a short summary sentence at the top
- Keep each bullet concise — rewrite commit messages into user-facing language (e.g., "Add About & Credits panel" becomes "New About & Credits panel with attribution tooltips")
- Show the draft release notes to the user for approval alongside the version bump

### Step 3: Confirm and execute

If the user approves, run the actual release. Pass through any arguments the user specified (e.g., `/desktop-release --minor`).

```bash
cd /home/pete/code/immerse-yourself && python3 scripts/desktop-release.py
```

The script will:
1. Verify clean main branch
2. Bump version in `tauri.conf.json`
3. Commit the version bump
4. Create an annotated git tag
5. Push commit + tag
6. Monitor the GitHub Actions workflow until all 3 platform builds complete
7. Report the release URL when the release job finishes

### Step 4: Attach release notes

Once the release is created on GitHub, update it with the release notes:

```bash
cd /home/pete/code/immerse-yourself && gh release edit vX.Y.Z --notes "RELEASE_NOTES_HERE"
```

Use a heredoc for multi-line notes:
```bash
gh release edit vX.Y.Z --notes "$(cat <<'EOF'
## What's New in vX.Y.Z

Summary sentence here.

### Features
- ...

### Fixes
- ...
EOF
)"
```

### Step 5: Report results

When the build finishes, report:
- Whether it succeeded or failed
- The release URL (e.g., `github.com/indubitablygregarious/immerse-yourself/releases/tag/vX.Y.Z`)
- The release notes that were attached
- If it failed, show the command to view logs: `gh run view <id> --log-failed`

## Options

Pass arguments from the user's invocation to the script:

| Argument | Effect |
|----------|--------|
| _(none)_ | Bump patch version (default) |
| `--minor` | Bump minor version |
| `--major` | Bump major version |
| `--version X.Y.Z` | Set explicit version |
| `--no-monitor` | Push and exit without waiting for CI |
| `--monitor-only vX.Y.Z` | Just watch an existing build |

## Troubleshooting

- **Not on main branch**: Switch to main first
- **Dirty working tree**: Commit or stash changes first
- **Tag already exists**: The version was already released; bump again
- **Build failed**: Run `gh run view <id> --log-failed` and report the error
