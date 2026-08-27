use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SystemInfo {
    os: String,
    arch: String,
    hostname: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolStatus {
    tool: String,
    available: bool,
    version: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectSummary {
    name: String,
    path: String,
    branch: Option<String>,
    dirty: bool,
    is_git_repo: bool,
    package_manager: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DevkitTemplate {
    id: String,
    name: String,
    repository: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateProjectInput {
    name: String,
    template_repository: String,
    owner: String,
    private: bool,
    destination_root: String,
    install_dependencies: bool,
    open_in_cursor: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateProjectProgress {
    step: String,
    status: String,
    message: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateProjectResult {
    path: String,
    repository: String,
    package_manager: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteProjectInput {
    path: String,
    projects_root: String,
    delete_remote: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeleteProjectResult {
    deleted_local: bool,
    deleted_remote: bool,
    remote: Option<String>,
}

const ALLOWED_TOOLS: &[&str] = &["git", "gh", "node", "pnpm", "sops", "age-keygen", "cursor"];
const CREATE_PROGRESS_EVENT: &str = "create-project-progress";

fn tool_search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        dirs.extend([
            home.join(".local/bin"),
            home.join("Library/pnpm"),
            home.join("Library/pnpm/bin"),
            home.join(".cargo/bin"),
        ]);
    }
    dirs.extend([
        PathBuf::from("/Applications/Cursor.app/Contents/Resources/app/bin"),
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/usr/bin"),
        PathBuf::from("/bin"),
    ]);
    dirs
}

fn resolve_tool(tool: &str) -> Option<PathBuf> {
    if !ALLOWED_TOOLS.contains(&tool) && tool != "npm" && tool != "yarn" && tool != "bun" {
        return None;
    }
    for dir in tool_search_dirs() {
        let candidate = dir.join(tool);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn tool_command(tool: &str) -> Result<Command, String> {
    let path = resolve_tool(tool).ok_or_else(|| {
        format!(
            "`{tool}` was not found. Install it and make sure it is available on your PATH."
        )
    })?;
    Ok(Command::new(path))
}

#[tauri::command]
fn system_info() -> SystemInfo {
    SystemInfo {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        hostname: hostname::get()
            .ok()
            .and_then(|value| value.into_string().ok())
            .unwrap_or_else(|| "This computer".to_string()),
    }
}

fn command_version(tool: &str) -> ToolStatus {
    if !ALLOWED_TOOLS.contains(&tool) {
        return ToolStatus {
            tool: tool.to_string(),
            available: false,
            version: None,
        };
    }

    let Some(path) = resolve_tool(tool) else {
        return ToolStatus {
            tool: tool.to_string(),
            available: false,
            version: None,
        };
    };

    match Command::new(path)
        .arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let text = if stdout.is_empty() { stderr } else { stdout };
            ToolStatus {
                tool: tool.to_string(),
                available: output.status.success() || !text.is_empty(),
                version: if text.is_empty() {
                    None
                } else {
                    Some(text.lines().next().unwrap_or_default().to_string())
                },
            }
        }
        Err(_) => ToolStatus {
            tool: tool.to_string(),
            available: false,
            version: None,
        },
    }
}

#[tauri::command]
fn check_tools() -> Vec<ToolStatus> {
    ALLOWED_TOOLS.iter().map(|tool| command_version(tool)).collect()
}

fn safe_existing_dir(path: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(path);
    if !path.exists() {
        return Err("Path does not exist.".to_string());
    }
    if !path.is_dir() {
        return Err("Path is not a directory.".to_string());
    }
    path.canonicalize()
        .map_err(|e| format!("Could not resolve path: {e}"))
}

fn ensure_project_under_root(path: &Path, root: &Path) -> Result<(), String> {
    if path == root {
        return Err("Refusing to delete the projects root folder.".into());
    }
    if !path.starts_with(root) {
        return Err("Project is outside the configured projects folder.".into());
    }
    // Only allow deleting direct children of the projects root.
    let relative = path
        .strip_prefix(root)
        .map_err(|_| "Project is outside the configured projects folder.".to_string())?;
    if relative.components().count() != 1 {
        return Err("Only direct project folders can be deleted.".into());
    }
    Ok(())
}

fn resolve_remote_repository(path: &Path) -> Option<String> {
    if let Ok(raw) = fs::read_to_string(path.join(".devkit.json")) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(repo) = value.get("repository").and_then(|v| v.as_str()) {
                let repo = repo.trim();
                if !repo.is_empty() && repo.contains('/') {
                    return Some(repo.to_string());
                }
            }
        }
    }

    if let Ok(raw) = run_checked(
        "gh",
        &["repo", "view", "--json", "nameWithOwner"],
        Some(path),
    ) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(repo) = value.get("nameWithOwner").and_then(|v| v.as_str()) {
                return Some(repo.to_string());
            }
        }
    }

    None
}

fn git_output(path: &Path, args: &[&str]) -> Option<String> {
    let mut cmd = tool_command("git").ok()?;
    let output = cmd.current_dir(path).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn detect_package_manager(path: &Path) -> Option<String> {
    if path.join("pnpm-lock.yaml").exists() {
        Some("pnpm".into())
    } else if path.join("yarn.lock").exists() {
        Some("yarn".into())
    } else if path.join("bun.lockb").exists() || path.join("bun.lock").exists() {
        Some("bun".into())
    } else if path.join("package-lock.json").exists() {
        Some("npm".into())
    } else if path.join("package.json").exists() {
        Some("node".into())
    } else {
        None
    }
}

fn project_summary(path: &Path) -> ProjectSummary {
    let is_git_repo = path.join(".git").exists();
    let branch = if is_git_repo {
        git_output(path, &["rev-parse", "--abbrev-ref", "HEAD"])
    } else {
        None
    };
    let dirty = if is_git_repo {
        git_output(path, &["status", "--porcelain"])
            .map(|s| !s.is_empty())
            .unwrap_or(false)
    } else {
        false
    };

    ProjectSummary {
        name: path
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or("Project")
            .to_string(),
        path: path.to_string_lossy().to_string(),
        branch,
        dirty,
        is_git_repo,
        package_manager: detect_package_manager(path),
    }
}

#[tauri::command]
fn list_projects(root: String) -> Result<Vec<ProjectSummary>, String> {
    let root = safe_existing_dir(&root)?;
    let mut projects = Vec::new();

    let entries = fs::read_dir(&root).map_err(|e| format!("Could not read projects folder: {e}"))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if path.join(".git").exists()
            || path.join("package.json").exists()
            || path.join(".devkit.json").exists()
        {
            projects.push(project_summary(&path));
        }
    }

    projects.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(projects)
}

#[tauri::command]
fn open_in_cursor(path: String) -> Result<(), String> {
    let path = safe_existing_dir(&path)?;
    tool_command("cursor")?
        .arg(path)
        .spawn()
        .map_err(|e| {
            format!("Could not launch Cursor. Make sure the `cursor` command is installed: {e}")
        })?;
    Ok(())
}

#[tauri::command]
fn git_pull(path: String) -> Result<String, String> {
    let path = safe_existing_dir(&path)?;
    if !path.join(".git").exists() {
        return Err("This folder is not a Git repository.".into());
    }

    let output = tool_command("git")?
        .current_dir(path)
        .args(["pull", "--ff-only"])
        .output()
        .map_err(|e| format!("Could not run git pull: {e}"))?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if error.is_empty() {
            "git pull failed.".into()
        } else {
            error
        });
    }

    let result = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(if result.is_empty() {
        "Already up to date.".into()
    } else {
        result
    })
}

fn templates_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("Could not resolve app config dir: {e}"))?;
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create config dir: {e}"))?;
    Ok(dir.join("templates.json"))
}

fn default_templates() -> Vec<DevkitTemplate> {
    vec![DevkitTemplate {
        id: "denyslavangelov-nextjs-template".into(),
        name: "Next.js template".into(),
        repository: "denyslavangelov/nextjs-template".into(),
        description: Some("Next.js starter from denyslavangelov/nextjs-template".into()),
    }]
}

fn read_templates(app: &AppHandle) -> Result<Vec<DevkitTemplate>, String> {
    let path = templates_path(app)?;
    if !path.exists() {
        let defaults = default_templates();
        write_templates(app, &defaults)?;
        return Ok(defaults);
    }
    let raw = fs::read_to_string(&path).map_err(|e| format!("Could not read templates: {e}"))?;
    let templates: Vec<DevkitTemplate> =
        serde_json::from_str(&raw).map_err(|e| format!("Invalid templates.json: {e}"))?;
    if templates.is_empty() {
        let defaults = default_templates();
        write_templates(app, &defaults)?;
        return Ok(defaults);
    }
    Ok(templates)
}

fn write_templates(app: &AppHandle, templates: &[DevkitTemplate]) -> Result<(), String> {
    let path = templates_path(app)?;
    let raw = serde_json::to_string_pretty(templates)
        .map_err(|e| format!("Could not serialize templates: {e}"))?;
    fs::write(&path, raw).map_err(|e| format!("Could not write templates: {e}"))
}

fn validate_template(template: &DevkitTemplate) -> Result<(), String> {
    if template.id.trim().is_empty() {
        return Err("Template id is required.".into());
    }
    if template.name.trim().is_empty() {
        return Err("Template name is required.".into());
    }
    validate_repository(&template.repository)?;
    Ok(())
}

fn validate_repository(repository: &str) -> Result<(), String> {
    let repository = repository.trim();
    let parts: Vec<&str> = repository.split('/').collect();
    if parts.len() != 2
        || parts[0].is_empty()
        || parts[1].is_empty()
        || !parts.iter().all(|part| {
            part.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        })
    {
        return Err("Repository must look like owner/repo.".into());
    }
    Ok(())
}

fn validate_project_name(name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Project name is required.".into());
    }
    if name.len() > 100 {
        return Err("Project name is too long.".into());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return Err("Project name may only contain letters, numbers, '.', '-' and '_'.".into());
    }
    Ok(())
}

fn validate_owner(owner: &str) -> Result<(), String> {
    let owner = owner.trim();
    if owner.is_empty() {
        return Ok(());
    }
    if !owner
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("GitHub owner may only contain letters, numbers, '-' and '_'.".into());
    }
    Ok(())
}

fn run_checked(command: &str, args: &[&str], cwd: Option<&Path>) -> Result<String, String> {
    let mut cmd = tool_command(command)?;
    cmd.args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("Could not run `{command}`: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if !output.status.success() {
        return Err(format_command_failure(command, &stdout, &stderr));
    }

    Ok(if stdout.is_empty() { stderr } else { stdout })
}

fn format_command_failure(command: &str, stdout: &str, stderr: &str) -> String {
    let mut parts = Vec::new();
    if !stderr.is_empty() {
        parts.push(stderr.to_string());
    }
    if !stdout.is_empty() {
        // Prefer the end of long install logs — that's where the real error usually is.
        let trimmed = tail_lines(stdout, 40);
        parts.push(trimmed);
    }
    if parts.is_empty() {
        format!("`{command}` failed.")
    } else {
        parts.join("\n\n")
    }
}

fn tail_lines(text: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= max_lines {
        return text.to_string();
    }
    let skipped = lines.len() - max_lines;
    let mut out = format!("… ({skipped} earlier lines omitted)\n");
    out.push_str(&lines[lines.len() - max_lines..].join("\n"));
    out
}

fn emit_progress(app: &AppHandle, step: &str, status: &str, message: Option<String>) {
    let _ = app.emit(
        CREATE_PROGRESS_EVENT,
        CreateProjectProgress {
            step: step.to_string(),
            status: status.to_string(),
            message,
        },
    );
}

fn install_dependencies(path: &Path) -> Result<Option<String>, String> {
    let Some(pm) = detect_package_manager(path) else {
        return Ok(None);
    };

    let result = match pm.as_str() {
        "pnpm" => run_checked("pnpm", &["install"], Some(path)),
        "yarn" => run_checked("yarn", &["install"], Some(path)),
        "bun" => run_checked("bun", &["install"], Some(path)),
        "npm" | "node" => run_checked("npm", &["install"], Some(path)),
        other => return Err(format!("Unsupported package manager: {other}")),
    };

    match result {
        Ok(_) => Ok(Some(pm)),
        Err(err) => {
            // pnpm can exit non-zero for ignored build scripts even when packages installed.
            if path.join("node_modules").is_dir() {
                Ok(Some(pm))
            } else {
                Err(err)
            }
        }
    }
}

fn now_iso() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

#[tauri::command]
fn list_templates(app: AppHandle) -> Result<Vec<DevkitTemplate>, String> {
    read_templates(&app)
}

#[tauri::command]
fn save_templates(app: AppHandle, templates: Vec<DevkitTemplate>) -> Result<Vec<DevkitTemplate>, String> {
    for template in &templates {
        validate_template(template)?;
    }
    write_templates(&app, &templates)?;
    Ok(templates)
}

#[tauri::command]
async fn create_project(
    app: AppHandle,
    input: CreateProjectInput,
) -> Result<CreateProjectResult, String> {
    tauri::async_runtime::spawn_blocking(move || create_project_inner(app, input))
        .await
        .map_err(|e| format!("Create task failed: {e}"))?
}

fn create_project_inner(
    app: AppHandle,
    input: CreateProjectInput,
) -> Result<CreateProjectResult, String> {
    let name = input.name.trim().to_string();
    let template_repository = input.template_repository.trim().to_string();
    let owner = input.owner.trim().to_string();

    validate_project_name(&name)?;
    validate_repository(&template_repository)?;
    validate_owner(&owner)?;

    let root = safe_existing_dir(&input.destination_root)?;
    let destination = root.join(&name);
    if destination.exists() {
        return Err(format!(
            "Destination already exists: {}",
            destination.display()
        ));
    }

    emit_progress(
        &app,
        "auth",
        "running",
        Some("Checking GitHub authentication…".into()),
    );
    run_checked("gh", &["auth", "status"], None).map_err(|e| {
        emit_progress(&app, "auth", "error", Some(e.clone()));
        if e.contains("not found") {
            format!("{e}\nInstall GitHub CLI, then run `gh auth login`.")
        } else {
            format!("GitHub CLI is not authenticated. Run `gh auth login` first.\n{e}")
        }
    })?;
    emit_progress(&app, "auth", "done", None);

    let repo_ref = if owner.is_empty() {
        name.clone()
    } else {
        format!("{owner}/{name}")
    };
    let visibility = if input.private {
        "--private"
    } else {
        "--public"
    };

    emit_progress(
        &app,
        "create",
        "running",
        Some(format!("Creating GitHub repository {repo_ref}…")),
    );
    run_checked(
        "gh",
        &[
            "repo",
            "create",
            &repo_ref,
            "--template",
            &template_repository,
            visibility,
            "--clone",
        ],
        Some(&root),
    )
    .map_err(|e| {
        emit_progress(&app, "create", "error", Some(e.clone()));
        e
    })?;
    emit_progress(&app, "create", "done", None);

    if !destination.exists() {
        let msg = format!(
            "Repository was created but not found at {}.",
            destination.display()
        );
        emit_progress(&app, "clone", "error", Some(msg.clone()));
        return Err(msg);
    }
    emit_progress(
        &app,
        "clone",
        "done",
        Some("Project cloned locally.".into()),
    );

    emit_progress(
        &app,
        "configure",
        "running",
        Some("Writing Devkit project metadata…".into()),
    );
    let remote = match run_checked(
        "gh",
        &["repo", "view", &repo_ref, "--json", "nameWithOwner"],
        Some(&destination),
    ) {
        Ok(raw) => serde_json::from_str::<serde_json::Value>(&raw)
            .ok()
            .and_then(|value| {
                value
                    .get("nameWithOwner")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| {
                if owner.is_empty() {
                    format!("unknown/{name}")
                } else {
                    format!("{owner}/{name}")
                }
            }),
        Err(_) => {
            if owner.is_empty() {
                format!("unknown/{name}")
            } else {
                format!("{owner}/{name}")
            }
        }
    };

    let meta = serde_json::json!({
        "name": name,
        "template": template_repository,
        "repository": remote,
        "createdAt": now_iso(),
    });
    fs::write(
        destination.join(".devkit.json"),
        serde_json::to_string_pretty(&meta).map_err(|e| e.to_string())?,
    )
    .map_err(|e| {
        let msg = format!("Could not write .devkit.json: {e}");
        emit_progress(&app, "configure", "error", Some(msg.clone()));
        msg
    })?;
    emit_progress(&app, "configure", "done", None);

    let mut package_manager = detect_package_manager(&destination);
    if input.install_dependencies {
        emit_progress(
            &app,
            "install",
            "running",
            Some("Installing dependencies…".into()),
        );
        match install_dependencies(&destination) {
            Ok(pm) => {
                if pm.is_some() {
                    package_manager = pm;
                }
                emit_progress(&app, "install", "done", None);
            }
            Err(e) => {
                emit_progress(&app, "install", "error", Some(e.clone()));
                return Err(e);
            }
        }
    } else {
        emit_progress(
            &app,
            "install",
            "done",
            Some("Skipped dependency install.".into()),
        );
    }

    if input.open_in_cursor {
        emit_progress(
            &app,
            "open",
            "running",
            Some("Opening project in Cursor…".into()),
        );
        match open_in_cursor(destination.to_string_lossy().to_string()) {
            Ok(()) => emit_progress(&app, "open", "done", None),
            Err(e) => {
                // Project was created successfully; opening is best-effort.
                emit_progress(
                    &app,
                    "open",
                    "done",
                    Some(format!("Skipped open in Cursor: {e}")),
                );
            }
        }
    }

    emit_progress(
        &app,
        "ready",
        "done",
        Some("Project is ready.".into()),
    );

    Ok(CreateProjectResult {
        path: destination.to_string_lossy().to_string(),
        repository: remote,
        package_manager,
    })
}

#[tauri::command]
fn delete_project(input: DeleteProjectInput) -> Result<DeleteProjectResult, String> {
    let root = safe_existing_dir(&input.projects_root)?;
    let path = safe_existing_dir(&input.path)?;
    ensure_project_under_root(&path, &root)?;

    let remote = resolve_remote_repository(&path);
    let mut deleted_remote = false;

    if input.delete_remote {
        let Some(repo) = remote.as_ref() else {
            return Err(
                "Could not determine the GitHub repository for this project. Delete locally only, or check `gh` auth."
                    .into(),
            );
        };
        run_checked("gh", &["repo", "delete", repo, "--yes"], None)?;
        deleted_remote = true;
    }

    fs::remove_dir_all(&path).map_err(|e| format!("Could not delete local project: {e}"))?;

    Ok(DeleteProjectResult {
        deleted_local: true,
        deleted_remote,
        remote,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            system_info,
            check_tools,
            list_projects,
            open_in_cursor,
            git_pull,
            list_templates,
            save_templates,
            create_project,
            delete_project,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Devkit");
}
