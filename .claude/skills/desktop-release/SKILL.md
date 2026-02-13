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

### Step 2: Confirm and execute

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

### Step 3: Report results

When the build finishes, report:
- Whether it succeeded or failed
- The release URL (e.g., `github.com/indubitablygregarious/immerse-yourself/releases/tag/vX.Y.Z`)
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
