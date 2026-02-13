---
name: ios-release
description: Bump the iOS app version, commit, push to main, and verify the GitHub Actions iOS build starts. Use when cutting a new iOS/TestFlight build.
allowed-tools: Bash(git *), Bash(gh *), Read, Edit
---

# iOS Release - Cut a New Build

Bump the version in `tauri.conf.json`, commit, push, and verify the GitHub Actions iOS build workflow starts.

## Process

### Step 1: Ensure correct branch and clean state

1. Check the current branch with `git branch --show-current`.
2. If not on `main`, **stop and tell the user** this skill must be run from the `main` branch (since the iOS build workflow only triggers on pushes to `main`).
3. Run `git status` to check for uncommitted changes.
4. If there are uncommitted changes, commit them first:
   a. Run `git diff --stat` and `git log --oneline -5` to understand the changes and commit style.
   b. Stage all modified/new files with `git add` (list specific files, not `-A`).
   c. Write a descriptive commit message summarizing the changes.
   d. Commit the changes.
5. Run `git pull` to make sure the branch is up to date with the remote.

### Step 2: Read current version

1. Read `rust/immerse-tauri/tauri.conf.json`.
2. Extract the current `"version"` value (e.g., `"0.3.1"`).
3. Display it to the user.

### Step 3: Bump the version

Increment the **patch** version by 1 (e.g., `0.3.1` -> `0.3.2`).

Use the Edit tool to update the `"version"` field in `rust/immerse-tauri/tauri.conf.json`.

### Step 4: Commit and push

1. Stage only the version file:
   ```bash
   git add rust/immerse-tauri/tauri.conf.json
   ```
2. Commit with a clear message:
   ```bash
   git commit -m "bump iOS version to X.Y.Z for TestFlight"
   ```
3. Push to main:
   ```bash
   git push
   ```

### Step 5: Verify the GitHub Actions workflow started

1. Wait 5 seconds for GitHub to register the push.
2. Run:
   ```bash
   gh run list --workflow=ios-build.yml --limit=1
   ```
3. Report the status and run URL to the user so they can monitor the build.
