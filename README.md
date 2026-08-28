# Devkit Desktop

Cross-platform desktop utility for creating, cloning, syncing and opening development projects on Windows and macOS.

## Features

- Tauri 2 + React + TypeScript + Vite
- Native tool detection for `git`, `gh`, `node`, `pnpm`, `sops`, `age-keygen`, and `cursor`
- Projects-folder picker and local project scanning
- Git branch, dirty status, unpushed commit count
- Pull, commit + push, and sync all (pull then push unpushed)
- Encrypted env sync: edit `.env.local` in your editor — Devkit encrypts on push and decrypts on clone (no env UI)
- Cross-machine sync via GitHub — clone remote projects, list repos across machines
- GitHub sign-in from Settings (browser OAuth or token — no terminal)
- Create projects from GitHub templates
- Delete local projects (optional GitHub remote delete)
- Open project in Cursor

## Install from Releases

GitHub Actions builds installers for:

- Windows
- macOS Apple Silicon
- macOS Intel

See the [Releases](../../releases) page for downloads.

To publish a new version:

1. Bump `version` in `package.json`, `src-tauri/tauri.conf.json`, and `src-tauri/Cargo.toml`
2. Commit, then tag and push:

```bash
git tag v0.3.0
git push origin main v0.3.0
```

That creates a GitHub Release with the built installers attached.

## Open in Cursor

Open this folder in Cursor, then run:

```bash
pnpm install
pnpm tauri dev
```

### macOS prerequisites

You need Rust and Xcode command line tools for Tauri. If Rust is not installed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Then restart the terminal/Cursor.

### Windows prerequisites

Tauri requires Rust plus the Windows C++ build tools/WebView2 prerequisites. Follow the official Tauri prerequisites page if `pnpm tauri dev` reports a missing native build dependency.

## Cursor CLI

The **Open Cursor** button invokes the `cursor` executable. In Cursor, install its shell command if the Doctor screen shows `cursor` as missing.

## Recommended build order

1. Commit + Push UI
2. SOPS + age ENV manager
3. Cross-machine install flow
4. Dev server start/stop
5. Packaging/signing for Windows + macOS

## Security approach

Native operations are exposed as narrow Tauri commands. V1 does not expose a generic `run_shell(command)` endpoint. Keep it that way: add specific native commands for each operation and validate filesystem paths/arguments before execution.
