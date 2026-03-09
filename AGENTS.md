# Repository Guidelines

## Project Structure & Module Organization
`rust/` contains the core engine and WASM-facing API, with AI code under `rust/src/ai/` and integration tests in `rust/tests/`. `python/` holds training and model-export utilities, plus tests in `python/tests/`. `web/` is the React + TypeScript frontend; UI components live in `web/src/components/`, hooks in `web/src/hooks/`, workers in `web/src/workers/`, and browser tests in `web/e2e/`. Treat `docs/REQUIREMENTS.md`, `docs/DESIGN.md`, and `docs/TASKS.md` as the product and architecture source of truth.

## Build, Test, and Development Commands
- `cargo test --manifest-path rust/Cargo.toml`: run Rust unit and integration tests.
- `wasm-pack build rust --target web`: compile the Rust engine to WebAssembly.
- `pytest python/tests -q`: run Python training and export tests.
- `npm --prefix web run dev`: start the Vite frontend locally.
- `npm --prefix web run build`: build WASM, type-check, and create the production bundle.
- `npm --prefix web run test`: run Vitest component and hook tests.
- `npm --prefix web run test:e2e`: run Playwright app flows.
- `npm --prefix web run lint`: run ESLint for the frontend.

## Coding Style & Naming Conventions
Use `cargo fmt` defaults for Rust, `ruff format` for Python, and 2-space indentation in TypeScript/React. Prefer `snake_case` for Rust and Python functions/modules, `PascalCase` for Rust types and React components, and `camelCase` for TS functions and hooks such as `useGame`. Keep files aligned with responsibility boundaries in `docs/DESIGN.md`.

## Testing Guidelines
Name Python tests `test_*.py`, keep Rust integration tests in `rust/tests/`, and colocate web unit tests as `*.test.ts(x)`. Cover deterministic AI behavior, API flow (`init_game -> place_stone -> ai_move -> get_result`), and browser compatibility where applicable. Before opening a PR, run the relevant module tests plus any impacted lint/format checks.

## Commit & Pull Request Guidelines
Recent history uses Conventional Commits, for example `feat(ai): add v3 symmetric model and benchmark tooling` and `fix(wasm): align test tie-break symmetry helper`. Keep branches short-lived off `main`, preferably linked to an issue number. PRs should include scope, linked issue or task, commands run with results, and screenshots or GIFs for visible UI changes.

## Contributor Workflow Notes
Use `.pre-commit-config.yaml` as the baseline quality gate: Rust is checked with `cargo fmt`, Python with `ruff format` and `ruff check`, and web code with ESLint. Avoid committing generated WASM build output unless the change explicitly requires it.
