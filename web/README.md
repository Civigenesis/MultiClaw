# MultiClaw Web Dashboard

The gateway embeds this Vite app at **compile time** via `rust-embed` (`src/gateway/static_files.rs`), reading `web/dist/`.

## Important: not built by `cargo` alone

Running `cargo build` **does not** run `npm`. Whatever is already in `web/dist/` when Rust compiles is what gets embedded. To refresh the UI inside the binary:

```bash
./scripts/build-web.sh
cargo build --release
```

Or manually:

```bash
cd web && npm ci && npm run build && cd .. && cargo build --release
```

If `web/dist/` is missing, `cargo` emits a **warning** (see root `build.rs`).

CI and `dev/ci.sh` run `scripts/ci/web_build_gate.sh` before `cargo` so release binaries always embed a fresh dashboard. The root `Dockerfile` builds `web/dist` inside the image (Node 20 + `npm ci` + `npm run build`) before the final `cargo build`.

## Development

```bash
cd web && npm ci && npm run dev
```

Point the dev server at a running gateway as documented in the main repo (same-origin `/api` expectations may require proxy config).
