# Cursor build brief — Devkit

## Product

Devkit is a cross-platform desktop app for Windows and macOS that removes terminal work from starting and moving development projects between machines.

The user should be able to:

- create a project from one of their GitHub template repositories
- clone an existing project onto the current machine
- install dependencies
- open a project in Cursor
- see Git status
- pull changes
- commit and push changes
- edit `.env.local`
- encrypt `.env.local` with SOPS + age into a committed encrypted file
- decrypt that file on another authorized computer

## Stack

- Tauri 2
- React
- TypeScript
- Vite
- Rust native command layer
- pnpm

## Architectural rule

Do not create a generic shell executor callable from the frontend.

Prefer narrow commands:

- `create_project(input)`
- `clone_project(input)`
- `git_pull(path)`
- `git_commit_push(path, message)`
- `open_in_cursor(path)`
- `install_dependencies(path)`
- `encrypt_env(path)`
- `decrypt_env(path)`

Validate paths and inputs in Rust.

## Milestone 2: New Project

Build the `+ New project` flow.

### UI

Modal fields:

- Project name
- Template selector
- GitHub owner
- Private/Public
- Local destination
- Install dependencies toggle
- Open in Cursor toggle

### Template model

Persist templates locally for now:

```ts
interface DevkitTemplate {
  id: string;
  name: string;
  repository: string; // owner/repo
  description?: string;
}
```

Start with a JSON config stored in the Tauri app config directory.

### Native creation flow

1. Validate project name.
2. Ensure destination does not exist.
3. Verify `gh auth status` succeeds.
4. Run GitHub CLI to create a private/public repo from the selected template.
5. Clone into the configured Projects folder.
6. Write `.devkit.json` into the project.
7. Detect package manager.
8. Install dependencies when selected.
9. Refresh the project list.
10. Open Cursor when selected.

Use GitHub CLI for V1; GitHub App/API auth can come later.

### UX

Show progress steps rather than freezing the modal:

- Creating GitHub repository
- Cloning project
- Configuring Devkit
- Installing dependencies
- Ready

Return useful errors to the UI.

## Milestone 3: ENV

Use SOPS + age.

Plaintext:

`.env.local` — ignored by Git

Encrypted:

`.env.enc` — committed to Git

Never display secrets unless the user explicitly toggles visibility.
