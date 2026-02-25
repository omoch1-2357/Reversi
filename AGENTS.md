# Repository Guidelines

## Project Structure & Module Organization
This repository is currently documentation-first. Use these files as the source of truth:
- `docs/REQUIREMENTS.md`: product and technical requirements
- `docs/DESIGN.md`: architecture and module design
- `docs/TASKS.md`: implementation phases and master checklist; execute and track work as GitHub Issues

Planned implementation layout:
- `rust/`: game engine and WASM API (`src/lib.rs`, `src/ai/search.rs`)
- `python/`: training pipeline (`train.py`, `td_lambda.py`, `export_model.py`)
- `web/`: React + TypeScript UI (`src/components`, `src/workers`, `src/wasm`)
- `.github/workflows/`: CI/CD and deployment jobs

## Build, Test, and Development Commands
Run commands from repository root (once each module exists):
- `cargo test --manifest-path rust/Cargo.toml` - run Rust unit/integration tests.
- `wasm-pack build rust --target web` - compile Rust engine to WebAssembly.
- `pytest python/tests -q` - run Python training/format tests.
- `npm --prefix web ci` - install frontend dependencies.
- `npm --prefix web run dev` - start local frontend dev server.
- `npm --prefix web run build` - create production frontend build.
- `npx --prefix web playwright test` - run browser integration/E2E checks.
- `gh issue create --title "..." --milestone "..."` - create and track Issues from `docs/TASKS.md`.
- `gh pr create --fill --body "Closes #<issue>"` - open PRs linked to Issues.

## Coding Style & Naming Conventions
- Rust: follow `rustfmt` defaults; `snake_case` for functions/modules, `PascalCase` for types.
- TypeScript/React: 2-space indentation, `PascalCase` components (`Board.tsx`), `camelCase` hooks/functions (`useGame.ts`).
- Python: PEP 8, `snake_case`, and type hints for public interfaces.
- Keep filenames aligned with responsibilities in `docs/DESIGN.md` (for example, search logic in `search.rs`).

## Testing Guidelines
- Rust tests: `rust/src/*` unit tests plus `rust/tests/` integration tests.
- Python tests: `python/tests/test_*.py`.
- Web tests: component/integration tests under `web/` plus Playwright E2E.
- Prioritize deterministic AI behavior checks (same state + level -> same move), full API flow checks (`init_game -> place_stone -> ai_move -> get_result`), and performance targets documented in `docs/REQUIREMENTS.md`.

## Commit & Pull Request Guidelines
Use GitHub Flow as the Git/branch strategy:
- branch from `main` with short-lived branches
- open a PR back to `main` for review and CI before merge
- link related Issues in PRs with `Closes #<issue>`

Issue-based workflow (Milestone-driven):
1. Create a Milestone for each phase.
2. Create Issues from `docs/TASKS.md` and assign them to that Milestone.
3. Start a branch from `main` using the Issue number (for example, `chore/#1-project-setup`).
4. Open a PR with `Closes #<issue>` and merge after CI passes.
5. Move to the next Issue.

There is no established commit history yet. Use Conventional Commits:
- `feat(rust): add legal move bitboard generation`
- `fix(web): handle ai_move worker timeout`
- `docs: update level-depth table`

PRs should include:
- concise summary and scope
- linked task(s) from `docs/TASKS.md`
- test evidence (commands run and key results)
- screenshots/GIFs for UI-impacting changes
- doc updates when behavior, APIs, or file layout changes
