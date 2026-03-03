---
name: sonarfix
description: Pull SonarCloud issues via MCP, create a tracking ticket linked to the parent issue, and fix them. Use when the user says "fix sonar issues", "sonarfix", or "clean up sonarqube findings".
allowed-tools: mcp__sonarqube__*, Bash(gh *), Bash(git *), Bash(make *), Read, Edit, Write, Glob, Grep
---

# SonarQube Issue Fixer

Fix SonarCloud findings for the `immerse-yourself` public repo. Each run picks a batch of related issues, creates a tracking ticket, fixes them, and pushes.

## Context

- **SonarCloud project**: `indubitablygregarious_immerse-yourself`
- **Parent tracking issue**: `indubitablygregarious/immerse-yourself-strategy#51`
- **Issue repo**: `indubitablygregarious/immerse-yourself-strategy` (NOT the public repo)
- **Labels**: `sonarqube`, plus category labels: `sonarqube:accessibility`, `sonarqube:complexity`, `sonarqube:code-style`, `sonarqube:css`, `sonarqube:react`, `sonarqube:html-semantics`

## Process

### Step 1: Pull current SonarCloud issues

Use the MCP tool `search_sonar_issues_in_projects` with:
- `projects`: `["indubitablygregarious_immerse-yourself"]`
- `issueStatuses`: `["OPEN"]`

Paginate (page 1 at ps=100, then page 2, etc.) to get all issues.

### Step 2: Categorize and pick a batch

Group issues by file and category. If the user specified a file or category, use that. Otherwise, pick the highest-impact batch using this priority:

1. **React anti-patterns** (real bugs/perf issues) — `sonarqube:react`
2. **Accessibility + HTML semantics** (user-facing quality) — `sonarqube:accessibility`, `sonarqube:html-semantics`
3. **CSS duplicates** (maintainability) — `sonarqube:css`
4. **Code style** (low priority, batch fix by file) — `sonarqube:code-style`
5. **Complexity** (refactoring, case by case) — `sonarqube:complexity`

Prefer fixing all issues in a single file at once rather than one issue across many files.

### Step 3: Create a child ticket

Create a GitHub issue in the **strategy repo** (`indubitablygregarious/immerse-yourself-strategy`) with:
- Title describing the fix (e.g., "Fix accessibility issues in SettingsDialog.tsx")
- Label: `sonarqube` + the relevant category label
- Body that includes:
  - `## Parent\nTracked by #51`
  - List of specific SonarCloud findings being addressed
  - Brief description of the fix approach

### Step 4: Fix the issues

1. Read the affected file(s)
2. Apply fixes for all selected SonarCloud findings
3. **MANDATORY: Verify changes compile before committing**:
   - For TypeScript changes: run `cd rust/immerse-tauri/ui && npx -p typescript tsc --noEmit`
   - For Rust changes: run `make check` from the repo root
   - Do NOT commit, push, or close issues until verification passes
4. Be careful not to change behavior — these are quality fixes, not feature changes

### Step 5: Commit and push

Commit with a message that:
- Summarizes the fixes
- References the child ticket: `Resolves indubitablygregarious/immerse-yourself-strategy#NN`
- Includes `Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>`

Push to `origin main`.

### Step 6: Report

Print a summary:
- Which issues were fixed (file, line, rule)
- Link to the closed ticket
- How many SonarCloud issues remain
