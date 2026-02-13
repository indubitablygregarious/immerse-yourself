# Bug Fix Skill
1. First, confirm which codebase is active (Tauri/TypeScript, NOT archived Python)
2. Read the relevant source files and reproduce the bug
3. Implement the fix in the CORRECT codebase
4. Run `cargo-1.89 build` for Rust changes or `npm run build` for TypeScript
5. If iOS-related: verify no read-only bundle paths, no subprocess dependencies (curl), and check framework linking
6. Run tests: `cargo-1.89 test` and/or the E2E suite
7. Commit with a descriptive message referencing the bug
