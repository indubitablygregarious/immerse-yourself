---
name: build-check
description: Run autonomous cross-platform build verification with 3 parallel agents (desktop build/test, iOS static analysis, CI workflow validation). Aggregates results into a release-readiness report.
allowed-tools: Task, Read, Bash(tail:*)
---

# Cross-Platform Build Verification

Run 3 parallel sub-agents to verify build readiness across desktop, iOS, and CI. Each agent iterates up to 5 times on failures. Results are aggregated into a single release-readiness report.

## Process

### Step 1: Launch all 3 agents in parallel

Use the Task tool to launch all 3 agents simultaneously with `run_in_background: true`. All 3 must be launched in a single message (parallel tool calls).

#### Agent 1: Desktop Build & Test (subagent_type: Bash)

Prompt the agent with:

> You are a desktop build verification agent for a Tauri (Rust + React/TypeScript) application at ~/iye/immerse-yourself.
>
> CRITICAL: Never run `cargo` directly. Always use the Makefile targets which use the correct Rust 1.89 wrapper.
>
> Your task: Run the full desktop build and test pipeline, iterating up to 5 times if there are failures. For each iteration, identify the root cause, fix it, and retry.
>
> Steps:
> 1. Run `make check` from ~/iye/immerse-yourself to verify Rust compilation
> 2. Run `make test` to run Rust tests
> 3. Run `npx tsc --noEmit` in ~/iye/immerse-yourself/rust/immerse-tauri/ui/ for TypeScript type checking
> 4. If any step fails, analyze the error, attempt a fix, and retry (up to 5 iterations total)
>
> For each iteration, report:
> - What was run
> - Whether it passed or failed
> - If failed: root cause and fix applied
>
> At the end, provide a summary report with:
> - Total iterations needed
> - All issues found and fixes applied
> - Final pass/fail status for: Rust compilation, Rust tests, TypeScript types
> - Any warnings worth noting

#### Agent 2: iOS Pipeline Validation (subagent_type: general-purpose)

Prompt the agent with:

> You are an iOS build verification agent for a Tauri 2.x application at ~/iye/immerse-yourself.
>
> Since we're on Linux, you cannot run actual iOS builds. Instead, perform static analysis of the iOS build pipeline to find potential issues BEFORE they hit CI/macOS.
>
> Your tasks (iterate up to 5 passes looking for issues):
>
> 1. **Framework linking check**: Search all Rust source files (especially in rust/immerse-core/src/ffi.rs and any iOS-specific code) for incorrect framework references, missing `#[link]` attributes, and any `std::process::Command` that calls `curl` (not available on iOS).
>
> 2. **Writable path audit**: Search ALL Rust source files for file path patterns that might target read-only bundle directories on iOS. Look for hardcoded paths, check that file writes use proper iOS-compatible directories (cache/documents, not app bundle), look for `std::env::current_dir()` usage, and check download_queue.rs for how freesound downloads are cached.
>
> 3. **Tauri iOS config check**: Read tauri.conf.json and Cargo.toml files. Check iOS bundle identifier is set, required Tauri iOS plugins are listed, and no desktop-only dependencies are unconditionally compiled for iOS.
>
> 4. **FFI layer validation**: Read rust/immerse-core/src/ffi.rs. Verify functions are properly marked with `#[no_mangle]` and `extern "C"`, types are C-compatible, and memory management is correct.
>
> 5. **Build script / CI consistency**: Read the Makefile iOS targets and .github/workflows/ios-build.yml. Verify they're consistent.
>
> For each issue found, report: file and line number, description, severity (critical/warning/info), and suggested fix.
>
> This is a READ-ONLY research task -- do NOT modify any files. Provide a final summary categorized by severity.

#### Agent 3: CI Workflow Validation (subagent_type: general-purpose)

Prompt the agent with:

> You are a CI/CD validation agent for the project at ~/iye/immerse-yourself.
>
> Validate ALL GitHub Actions workflow files for correctness, security, and best practices. Iterate up to 5 passes looking for different categories of issues.
>
> Steps:
>
> 1. **Find all workflow files**: Glob for .github/workflows/*.yml and .github/workflows/*.yaml
>
> 2. **Action reference validation**: For every `uses:` line, check that the action exists and is real (e.g., `actions/checkout@v4` is valid, `rust-action/setup@v1` is NOT). Check for deprecated actions like `actions-rs/toolchain`. Verify version pins use tags not branches.
>
> 3. **Concurrency guards**: Every workflow should have `concurrency:` with `group:` and `cancel-in-progress: true`. Flag any missing.
>
> 4. **macOS runner costs**: Check for `macos-*` runners. Flag any job using macOS that could run on ubuntu instead.
>
> 5. **Caching**: Check that workflows with npm/cargo steps have proper caching. Flag missing cache steps.
>
> 6. **Security**: No secrets in plain text, proper `${{ secrets.* }}` usage, no unfiltered `pull_request_target`, scoped permissions.
>
> 7. **General correctness**: Valid YAML, descriptive step names, correct triggers, properly quoted env vars.
>
> For each issue: report workflow file and line, description, severity, and recommended fix.
>
> This is a READ-ONLY research task -- do NOT modify any files. Provide a final categorized summary.

### Step 2: Wait for all agents to complete

Monitor agent completion. You will be notified as each finishes. If any agent takes unusually long, you can check progress by reading its output file.

### Step 3: Aggregate into a release-readiness report

Once all 3 agents have completed, compile their results into a single structured report with these sections:

```
# Build Readiness Report

## 1. Desktop Build & Test
- Table: Check | Status | Details
- Iterations needed
- Compiler warnings (if any)

## 2. iOS Pipeline (Static Analysis)
- Critical issues table (will cause runtime failures)
- Warning issues table
- Verified-clean areas

## 3. CI Workflow Validation
- Issues table by severity
- Overall assessment

## Release Readiness Summary
- Table: Area | Verdict (GO / HOLD / GO with caveats)
- Recommendation paragraph
```

Verdicts:
- **GO**: All checks pass, no critical issues
- **GO with caveats**: Functional but has non-blocking warnings worth addressing
- **HOLD**: Critical issues that must be fixed before release
