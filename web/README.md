# Reversi Web Frontend

## Project-specific setup

### 1. Base path for GitHub Pages

Set `base: '/Reversi/'` in `vite.config.ts`.

```ts
export default defineConfig({
  base: '/Reversi/',
})
```

This ensures built assets are resolved under the repository subpath.

### 2. Build WASM artifacts

Generate WASM from Rust into `web/src/wasm/pkg` before TypeScript/Vite build.

```bash
npm --prefix web run build:wasm
```

The command runs:

```bash
node ./scripts/build-wasm.mjs
```

To embed a specific model at build time:

```bash
npm --prefix web run build:wasm -- --model-path=../models/custom.weights.bin
```

`--model-path` is resolved relative to `web/` and is forwarded to Rust via
`REVERSI_MODEL_PATH`.

Expected generated files:

- `web/src/wasm/pkg/reversi.js`
- `web/src/wasm/pkg/reversi_bg.wasm`
- `web/src/wasm/pkg/reversi.d.ts`

### 3. Development and production placement

- Development: app imports from `src/wasm/pkg` via `src/wasm/index.ts`.
- Production: Vite bundles/copies the wasm binary under `dist/assets/*.wasm`.

Build all artifacts:

```bash
npm --prefix web run build
```

You can pass the same option through the full production build:

```bash
npm --prefix web run build -- --model-path=../models/custom.weights.bin
```

Expected output example:

- `web/dist/index.html`
- `web/dist/assets/reversi_bg-*.wasm`

### 4. Serving and path expectations

Use a static server that serves the built directory as `/Reversi/` on GitHub Pages.
For local preview:

```bash
npm --prefix web run preview
```

If deploying under another subpath, update `vite.config.ts` `base` accordingly.
