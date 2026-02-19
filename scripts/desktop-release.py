#!/usr/bin/env python3
"""Cut a release: bump version, tag, push, and monitor CI build.

Supports both desktop (full release with tag) and iOS-only (TestFlight) releases.

Usage:
    python3 scripts/desktop-release.py              # bump patch, tag, push (desktop + iOS)
    python3 scripts/desktop-release.py --minor      # bump minor version
    python3 scripts/desktop-release.py --major      # bump major version
    python3 scripts/desktop-release.py --version 1.0.0  # explicit version
    python3 scripts/desktop-release.py --dry-run    # show what would happen
    python3 scripts/desktop-release.py --ios-only   # iOS TestFlight only (no tag, no desktop build)
    python3 scripts/desktop-release.py --monitor-only v0.3.17  # just watch an existing run
"""

import argparse
import json
import os
import subprocess
import sys
import time

TAURI_CONF = "rust/immerse-tauri/tauri.conf.json"
WORKFLOW = "desktop-build.yml"
IOS_WORKFLOW = "ios-build.yml"
POLL_INTERVAL = 30  # seconds between status checks
POLL_TIMEOUT = 40 * 60  # 40 minutes max wait


def run(cmd, check=True, capture=True):
    """Run a shell command, return stdout."""
    result = subprocess.run(
        cmd, shell=True, capture_output=capture, text=True, check=check
    )
    return result.stdout.strip() if capture else None


def get_repo_root():
    """Find the git repo root."""
    return run("git rev-parse --show-toplevel")


def read_version(repo_root):
    """Read current version from tauri.conf.json."""
    conf_path = os.path.join(repo_root, TAURI_CONF)
    with open(conf_path) as f:
        conf = json.load(f)
    return conf["version"]


def write_version(repo_root, new_version):
    """Write new version to tauri.conf.json."""
    conf_path = os.path.join(repo_root, TAURI_CONF)
    with open(conf_path) as f:
        conf = json.load(f)
    conf["version"] = new_version
    with open(conf_path, "w") as f:
        json.dump(conf, f, indent=2)
        f.write("\n")


def bump_version(current, part="patch"):
    """Bump a semver version string."""
    major, minor, patch = (int(x) for x in current.split("."))
    if part == "major":
        return f"{major + 1}.0.0"
    elif part == "minor":
        return f"{major}.{minor + 1}.0"
    else:
        return f"{major}.{minor}.{patch + 1}"


def check_prerequisites(repo_root):
    """Verify we're in a clean state on the main branch."""
    errors = []

    branch = run("git branch --show-current")
    if branch != "main":
        errors.append(f"Not on main branch (currently on '{branch}')")

    status = run("git status --porcelain")
    if status:
        errors.append(f"Working tree is not clean:\n{status}")

    # Check remote is up to date
    run("git fetch origin main", check=False)
    behind = run("git rev-list HEAD..origin/main --count")
    if behind and int(behind) > 0:
        errors.append(f"Local main is {behind} commit(s) behind origin/main — run git pull")

    # Check gh CLI is available
    try:
        run("gh --version")
    except (subprocess.CalledProcessError, FileNotFoundError):
        errors.append("GitHub CLI (gh) not found — install from https://cli.github.com")

    return errors


def check_tag_exists(tag):
    """Check if a git tag already exists locally or remotely."""
    local = run(f"git tag -l {tag}")
    if local:
        return f"Tag {tag} already exists locally"
    remote = run(f"git ls-remote --tags origin refs/tags/{tag}")
    if remote:
        return f"Tag {tag} already exists on remote"
    return None


def create_release(repo_root, new_version, dry_run=False, ios_only=False):
    """Bump version, commit, tag, and push.

    If ios_only=True, skips tag creation and pushes without --follow-tags.
    The push to main triggers ios-build.yml without triggering desktop-build.yml.
    """
    tag = f"v{new_version}"

    if not ios_only:
        existing = check_tag_exists(tag)
        if existing:
            print(f"ERROR: {existing}")
            sys.exit(1)

    if ios_only:
        commit_msg = f"release {tag}: iOS TestFlight"
    else:
        commit_msg = f"release {tag}: desktop build"

    if dry_run:
        print(f"[DRY RUN] Would update {TAURI_CONF}: version -> {new_version}")
        print(f"[DRY RUN] Would commit: '{commit_msg}'")
        if not ios_only:
            print(f"[DRY RUN] Would create tag: {tag}")
            print("[DRY RUN] Would push commit + tag to origin")
        else:
            print("[DRY RUN] Would push commit to origin (no tag)")
        return tag

    # Update version
    write_version(repo_root, new_version)
    print(f"Updated {TAURI_CONF} to {new_version}")

    # Commit
    run(f"git add {TAURI_CONF}")
    run(f'git commit -m "{commit_msg}"')
    print("Committed version bump")

    if not ios_only:
        # Tag
        run(f'git tag -a {tag} -m "Desktop release {tag}"')
        print(f"Created tag {tag}")

        # Push commit and tag together
        run("git push origin main --follow-tags")
        print("Pushed commit + tag to origin")
    else:
        # Push commit only (no tag — triggers ios-build.yml but not desktop-build.yml)
        run("git push origin main")
        print("Pushed commit to origin (iOS only, no tag)")

    return tag


def monitor_build(tag, timeout=POLL_TIMEOUT, workflow=WORKFLOW, branch=None):
    """Poll GitHub Actions until the workflow completes or times out.

    branch: which branch to search for runs. Defaults to tag for desktop
    builds, "main" for iOS builds.
    """
    label = "iOS" if workflow == IOS_WORKFLOW else "desktop"
    if branch is None:
        branch = "main" if workflow == IOS_WORKFLOW else tag
    print(f"\nMonitoring {label} build for {tag}...")
    print(f"(checking every {POLL_INTERVAL}s, timeout {timeout // 60}min)\n")

    # Wait for the run to appear
    run_id = None
    for attempt in range(6):
        time.sleep(10 if attempt == 0 else POLL_INTERVAL)
        result = run(
            f"gh run list --workflow={workflow} --branch={branch} --limit=1 --json databaseId,status,conclusion,headBranch",
            check=False,
        )
        if not result or result == "[]":
            # Also try matching by recent runs
            result = run(
                f"gh run list --workflow={workflow} --limit=5 --json databaseId,status,conclusion,headBranch,event",
                check=False,
            )
            if result:
                runs = json.loads(result)
                for r in runs:
                    if r.get("headBranch") == branch or r.get("status") in ("queued", "in_progress"):
                        run_id = r["databaseId"]
                        break
            if run_id:
                break
            print(f"  Waiting for workflow to start... (attempt {attempt + 1})")
            continue
        runs = json.loads(result)
        if runs:
            run_id = runs[0]["databaseId"]
            break

    if not run_id:
        print("WARNING: Could not find workflow run. Check manually:")
        print(f"  gh run list --workflow={workflow}")
        run("gh browse --no-browser -n 2>/dev/null || echo ''", check=False)
        return False

    # Show the run URL
    run_url = run(f"gh run view {run_id} --json url --jq .url", check=False)
    if run_url:
        print(f"Workflow run: {run_url}")

    # Poll until completion
    start = time.time()
    last_status = None
    while time.time() - start < timeout:
        result = run(
            f"gh run view {run_id} --json status,conclusion,jobs",
            check=False,
        )
        if not result:
            time.sleep(POLL_INTERVAL)
            continue

        data = json.loads(result)
        status = data.get("status", "unknown")
        conclusion = data.get("conclusion", "")

        # Print job-level status
        jobs = data.get("jobs", [])
        job_summary = ", ".join(
            f"{j['name']}: {j.get('conclusion') or j.get('status', '?')}"
            for j in jobs
        )
        status_line = f"  [{status}] {job_summary}"
        if status_line != last_status:
            elapsed = int(time.time() - start)
            print(f"  {elapsed // 60}m{elapsed % 60:02d}s - {status} | {job_summary}")
            last_status = status_line

        if status == "completed":
            print()
            if conclusion == "success":
                print("BUILD SUCCEEDED")
                if workflow != IOS_WORKFLOW:
                    release_url = run(
                        f"gh release view {tag} --json url --jq .url 2>/dev/null",
                        check=False,
                    )
                    if release_url:
                        print(f"Release: {release_url}")
                return True
            else:
                print(f"BUILD FAILED (conclusion: {conclusion})")
                print(f"Debug: gh run view {run_id} --log-failed")
                return False

        time.sleep(POLL_INTERVAL)

    print(f"\nTIMEOUT after {timeout // 60} minutes. Build still running.")
    print(f"Monitor manually: gh run watch {run_id}")
    return False


def main():
    parser = argparse.ArgumentParser(description="Cut a release (desktop + iOS, or iOS only)")
    bump_group = parser.add_mutually_exclusive_group()
    bump_group.add_argument("--patch", action="store_true", default=True, help="Bump patch version (default)")
    bump_group.add_argument("--minor", action="store_true", help="Bump minor version")
    bump_group.add_argument("--major", action="store_true", help="Bump major version")
    bump_group.add_argument("--version", type=str, help="Set explicit version (e.g., 1.0.0)")
    parser.add_argument("--dry-run", action="store_true", help="Show what would happen without doing it")
    parser.add_argument("--no-monitor", action="store_true", help="Don't wait for CI build to finish")
    parser.add_argument("--monitor-only", type=str, metavar="TAG", help="Just monitor an existing tag's build (e.g., v0.3.17)")
    parser.add_argument("--ios-only", action="store_true", help="iOS TestFlight release only (no tag, no desktop build)")

    args = parser.parse_args()

    # Monitor-only mode
    if args.monitor_only:
        success = monitor_build(args.monitor_only)
        sys.exit(0 if success else 1)

    repo_root = get_repo_root()
    os.chdir(repo_root)

    # Preflight checks
    errors = check_prerequisites(repo_root)
    if errors and not args.dry_run:
        print("Cannot proceed:")
        for e in errors:
            print(f"  - {e}")
        sys.exit(1)

    # Determine new version
    current = read_version(repo_root)
    if args.version:
        new_version = args.version
    elif args.major:
        new_version = bump_version(current, "major")
    elif args.minor:
        new_version = bump_version(current, "minor")
    else:
        new_version = bump_version(current, "patch")

    tag = f"v{new_version}"
    mode = "iOS TestFlight" if args.ios_only else "desktop + iOS"
    print(f"Current version: {current}")
    print(f"New version:     {new_version} ({tag})")
    print(f"Mode:            {mode}")
    print()

    if not args.dry_run:
        if args.ios_only:
            confirm = input(f"Push iOS release {tag} to main (no tag)? [y/N] ").strip().lower()
        else:
            confirm = input(f"Create release {tag}? [y/N] ").strip().lower()
        if confirm != "y":
            print("Aborted.")
            sys.exit(0)

    tag = create_release(repo_root, new_version, dry_run=args.dry_run, ios_only=args.ios_only)

    if args.dry_run:
        print("\nDry run complete. No changes made.")
        return

    workflow = IOS_WORKFLOW if args.ios_only else WORKFLOW

    if args.no_monitor:
        print(f"\nRelease {tag} pushed. Monitor with:")
        print(f"  python3 scripts/desktop-release.py --monitor-only {tag}")
        print(f"  gh run list --workflow={workflow}")
        return

    success = monitor_build(tag, workflow=workflow)
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()
