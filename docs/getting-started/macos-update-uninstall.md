# macOS Update and Uninstall Guide

This page documents supported update and uninstall procedures for MultiClaw on macOS (OS X).

Last verified: **February 22, 2026**.

## 1) Check current install method

```bash
which multiclaw
multiclaw --version
```

Typical locations:

- Homebrew: `/opt/homebrew/bin/multiclaw` (Apple Silicon) or `/usr/local/bin/multiclaw` (Intel)
- Cargo/bootstrap/manual: `~/.cargo/bin/multiclaw`

If both exist, your shell `PATH` order decides which one runs.

## 2) Update on macOS

### A) Homebrew install

```bash
brew update
brew upgrade multiclaw
multiclaw --version
```

### B) Clone + bootstrap install

From your local repository checkout:

```bash
git pull --ff-only
./bootstrap.sh --prefer-prebuilt
multiclaw --version
```

If you want source-only update:

```bash
git pull --ff-only
cargo install --path . --force --locked
multiclaw --version
```

### C) Manual prebuilt binary install

Re-run your download/install flow with the latest release asset, then verify:

```bash
multiclaw --version
```

## 3) Uninstall on macOS

### A) Stop and remove background service first

This prevents the daemon from continuing to run after binary removal.

```bash
multiclaw service stop || true
multiclaw service uninstall || true
```

Service artifacts removed by `service uninstall`:

- `~/Library/LaunchAgents/com.multiclaw.daemon.plist`

### B) Remove the binary by install method

Homebrew:

```bash
brew uninstall multiclaw
```

Cargo/bootstrap/manual (`~/.cargo/bin/multiclaw`):

```bash
cargo uninstall multiclaw || true
rm -f ~/.cargo/bin/multiclaw
```

### C) Optional: remove local runtime data

Only run this if you want a full cleanup of config, auth profiles, logs, and workspace state.

```bash
rm -rf ~/.multiclaw
```

## 4) Verify uninstall completed

```bash
command -v multiclaw || echo "multiclaw binary not found"
pgrep -fl multiclaw || echo "No running multiclaw process"
```

If `pgrep` still finds a process, stop it manually and re-check:

```bash
pkill -f multiclaw
```

## Related docs

- [One-Click Bootstrap](../one-click-bootstrap.md)
- [Commands Reference](../commands-reference.md)
- [Troubleshooting](../troubleshooting.md)
