use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
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
    ahead: u32,
    is_git_repo: bool,
    package_manager: Option<String>,
    env_out_of_sync: bool,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CloneProjectInput {
    repository: String,
    destination_root: String,
    install_dependencies: bool,
    open_in_cursor: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CloneProjectResult {
    path: String,
    repository: String,
    package_manager: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteProjectSummary {
    name: String,
    repository: String,
    updated_at: Option<String>,
    cloned_locally: bool,
    local_path: Option<String>,
    branch: Option<String>,
    dirty: bool,
    package_manager: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncProjectResult {
    name: String,
    path: String,
    ok: bool,
    message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GitHubAuthStatus {
    gh_available: bool,
    authenticated: bool,
    username: Option<String>,
    scopes: Vec<String>,
    missing_scopes: Vec<String>,
    git_protocol: Option<String>,
    message: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgeKeyStatus {
    has_key: bool,
    public_key: Option<String>,
    sops_available: bool,
    age_keygen_available: bool,
    message: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EnvStatus {
    has_local: bool,
    has_encrypted: bool,
    out_of_sync: bool,
    can_encrypt: bool,
    can_decrypt: bool,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WriteEnvLocalInput {
    path: String,
    content: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportAgeKeyInput {
    secret: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallEnvToolsResult {
    ok: bool,
    message: String,
    sops_available: bool,
    age_keygen_available: bool,
}

const ALLOWED_TOOLS: &[&str] = &["git", "gh", "node", "pnpm", "sops", "age-keygen", "cursor"];
const CREATE_PROGRESS_EVENT: &str = "create-project-progress";
const DEVKIT_GITHUB_TOPIC: &str = "devkit";
const GITHUB_HOST: &str = "github.com";
const REQUIRED_GITHUB_SCOPES: &[&str] = &["repo", "delete_repo", "read:org", "gist"];
const GITHUB_CLI_INSTALL_URL: &str = "https://cli.github.com/";
const ENV_LOCAL: &str = ".env.local";
const ENV_ENC: &str = ".env.enc";
const SOPS_YAML: &str = ".sops.yaml";
const SOPS_INSTALL_URL: &str = "https://github.com/getsops/sops/releases";

fn open_url(url: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let result = Command::new("open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let result = Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let result = Command::new("xdg-open").arg(url).spawn();

    result.map_err(|e| format!("Could not open link: {e}"))?;
    Ok(())
}

fn github_auth_help() -> &'static str {
    "Connect GitHub in Settings to continue."
}

fn format_github_auth_error(raw: &str) -> String {
    if raw.contains("not found") {
        return format!(
            "GitHub CLI is not installed. Open Settings and choose Install GitHub CLI.\n{raw}"
        );
    }
    if raw.contains("not logged in")
        || raw.contains("not authenticated")
        || raw.contains("To re-authenticate")
    {
        return format!("{}\n{}", github_auth_help(), raw);
    }
    raw.to_string()
}

fn parse_github_scopes(status_text: &str) -> Vec<String> {
    for line in status_text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("- Token scopes:") {
            return rest
                .split(',')
                .map(|scope| scope.trim().trim_matches('\'').trim_matches('"').to_string())
                .filter(|scope| !scope.is_empty())
                .collect();
        }
    }
    Vec::new()
}

fn parse_github_protocol(status_text: &str) -> Option<String> {
    for line in status_text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("- Git operations protocol:") {
            return Some(rest.trim().to_string());
        }
    }
    None
}

fn missing_github_scopes(scopes: &[String]) -> Vec<String> {
    REQUIRED_GITHUB_SCOPES
        .iter()
        .filter(|required| !scopes.iter().any(|scope| scope == *required))
        .map(|scope| scope.to_string())
        .collect()
}

fn github_auth_status_inner() -> GitHubAuthStatus {
    let gh_available = resolve_tool("gh").is_some();
    if !gh_available {
        return GitHubAuthStatus {
            gh_available: false,
            authenticated: false,
            username: None,
            scopes: Vec::new(),
            missing_scopes: REQUIRED_GITHUB_SCOPES.iter().map(|s| s.to_string()).collect(),
            git_protocol: None,
            message: Some("GitHub CLI is not installed on this computer.".into()),
        };
    }

    let status_output = tool_command("gh")
        .and_then(|mut cmd| {
            cmd.args(["auth", "status", "-h", GITHUB_HOST])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .map_err(|e| e.to_string())
        });

    let (status_text, authenticated) = match status_output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let text = format!("{stdout}{stderr}");
            (text, output.status.success())
        }
        Err(err) => (err, false),
    };

    let scopes = if authenticated {
        parse_github_scopes(&status_text)
    } else {
        Vec::new()
    };
    let missing_scopes = if authenticated {
        missing_github_scopes(&scopes)
    } else {
        REQUIRED_GITHUB_SCOPES.iter().map(|s| s.to_string()).collect()
    };
    let git_protocol = if authenticated {
        parse_github_protocol(&status_text)
    } else {
        None
    };

    let username = if authenticated {
        run_checked("gh", &["api", "user", "-q", ".login"], None).ok()
    } else {
        None
    };

    let message = if !authenticated {
        Some("Sign in with GitHub to create, clone, and sync projects.".into())
    } else if !missing_scopes.is_empty() {
        Some("Some GitHub permissions are missing. Refresh access to enable all Devkit features.".into())
    } else {
        None
    };

    GitHubAuthStatus {
        gh_available: true,
        authenticated,
        username,
        scopes,
        missing_scopes,
        git_protocol,
        message,
    }
}

#[tauri::command]
fn github_auth_status() -> GitHubAuthStatus {
    github_auth_status_inner()
}

#[tauri::command]
fn open_external_url(url: String) -> Result<(), String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("URL is required.".into());
    }
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err("Only http(s) links are allowed.".into());
    }
    open_url(url)
}

#[tauri::command]
fn install_github_cli() -> Result<(), String> {
    open_url(GITHUB_CLI_INSTALL_URL)
}

#[tauri::command]
async fn github_auth_login() -> Result<GitHubAuthStatus, String> {
    tauri::async_runtime::spawn_blocking(github_auth_login_inner)
        .await
        .map_err(|e| format!("GitHub sign-in failed: {e}"))?
}

fn github_auth_login_inner() -> Result<GitHubAuthStatus, String> {
    if resolve_tool("gh").is_none() {
        return Err("GitHub CLI is not installed. Use Install GitHub CLI in Settings.".into());
    }

    let output = tool_command("gh")?
        .args([
            "auth",
            "login",
            "-h",
            GITHUB_HOST,
            "-p",
            "https",
            "-w",
            "--skip-ssh-key",
            "-s",
            "repo",
            "-s",
            "delete_repo",
            "-s",
            "read:org",
            "-s",
            "gist",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Could not start GitHub sign-in: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Err(format_github_auth_error(if stderr.is_empty() {
            &stdout
        } else {
            &stderr
        }));
    }

    Ok(github_auth_status_inner())
}

#[tauri::command]
fn github_auth_login_with_token(token: String) -> Result<GitHubAuthStatus, String> {
    if resolve_tool("gh").is_none() {
        return Err("GitHub CLI is not installed. Use Install GitHub CLI in Settings.".into());
    }

    let token = token.trim();
    if token.is_empty() {
        return Err("Paste a GitHub personal access token.".into());
    }

    let mut child = tool_command("gh")?
        .args(["auth", "login", "--with-token"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not save GitHub token: {e}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin
            .write_all(token.as_bytes())
            .map_err(|e| format!("Could not send token to GitHub CLI: {e}"))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("GitHub token sign-in failed: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "GitHub rejected that token. Check scopes and try again.".into()
        } else {
            stderr
        });
    }

    Ok(github_auth_status_inner())
}

#[tauri::command]
async fn github_auth_refresh() -> Result<GitHubAuthStatus, String> {
    tauri::async_runtime::spawn_blocking(github_auth_refresh_inner)
        .await
        .map_err(|e| format!("GitHub permission refresh failed: {e}"))?
}

fn github_auth_refresh_inner() -> Result<GitHubAuthStatus, String> {
    if resolve_tool("gh").is_none() {
        return Err("GitHub CLI is not installed. Use Install GitHub CLI in Settings.".into());
    }

    let output = tool_command("gh")?
        .args([
            "auth",
            "refresh",
            "-h",
            GITHUB_HOST,
            "-s",
            "repo",
            "-s",
            "delete_repo",
            "-s",
            "read:org",
            "-s",
            "gist",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Could not refresh GitHub permissions: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "Could not refresh GitHub permissions.".into()
        } else {
            stderr
        });
    }

    Ok(github_auth_status_inner())
}

#[tauri::command]
fn github_auth_logout() -> Result<GitHubAuthStatus, String> {
    if resolve_tool("gh").is_none() {
        return Ok(github_auth_status_inner());
    }

    let _ = tool_command("gh")
        .and_then(|mut cmd| {
            cmd.args(["auth", "logout", "-h", GITHUB_HOST, "--yes"])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .map_err(|e| e.to_string())
        });

    Ok(github_auth_status_inner())
}

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
            home.join("scoop/shims"),
        ]);
    }
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        let local = PathBuf::from(local_app_data);
        dirs.push(local.join("Microsoft/WindowsApps"));
        dirs.push(local.join("Programs/SOPS"));
    }
    dirs.extend([
        PathBuf::from("/Applications/Cursor.app/Contents/Resources/app/bin"),
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("C:/Program Files/SOPS"),
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

fn local_projects_by_repository(root: &Path) -> HashMap<String, PathBuf> {
    let mut map = HashMap::new();
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if let Some(repo) = resolve_remote_repository(&path) {
                map.insert(repo, path);
            }
        }
    }
    map
}

fn github_repo_has_devkit_topic(value: &serde_json::Value) -> bool {
    value
        .get("repositoryTopics")
        .and_then(|topics| topics.as_array())
        .map(|topics| {
            topics.iter().any(|topic| {
                topic
                    .get("name")
                    .and_then(|name| name.as_str())
                    .map(|name| name.eq_ignore_ascii_case(DEVKIT_GITHUB_TOPIC))
                    .unwrap_or(false)
                    || topic
                        .as_str()
                        .map(|name| name.eq_ignore_ascii_case(DEVKIT_GITHUB_TOPIC))
                        .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn register_github_project(repository: &str, path: Option<&Path>) {
    let _ = run_checked(
        "gh",
        &["repo", "edit", repository, "--add-topic", DEVKIT_GITHUB_TOPIC],
        None,
    );

    if let Some(path) = path {
        if !path.join(".devkit.json").exists() {
            return;
        }
        let dirty = git_output(path, &["status", "--porcelain", ".devkit.json"])
            .map(|status| !status.is_empty())
            .unwrap_or(false);
        if !dirty {
            return;
        }
        let _ = run_checked("git", &["add", ".devkit.json"], Some(path));
        let _ = run_checked(
            "git",
            &["commit", "-m", "chore: add Devkit project metadata"],
            Some(path),
        );
        let _ = run_checked("git", &["push"], Some(path));
    }
}

fn fetch_github_repo(repository: &str) -> Option<serde_json::Value> {
    run_checked(
        "gh",
        &[
            "repo",
            "view",
            repository,
            "--json",
            "nameWithOwner,name,updatedAt,isArchived,repositoryTopics",
        ],
        None,
    )
    .ok()
    .and_then(|raw| serde_json::from_str(&raw).ok())
}

fn remote_summary_from_json(
    value: &serde_json::Value,
    local_by_repo: &HashMap<String, PathBuf>,
) -> Option<RemoteProjectSummary> {
    if value
        .get("isArchived")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        return None;
    }

    let repository = value
        .get("nameWithOwner")
        .and_then(|v| v.as_str())
        .map(str::to_string)?;
    let name = value
        .get("name")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| repository.split('/').nth(1).unwrap_or(&repository).to_string());
    let updated_at = value
        .get("updatedAt")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let local_path = local_by_repo.get(&repository);
    let (branch, dirty, package_manager) = if let Some(path) = local_path {
        let summary = project_summary(path);
        (
            summary.branch,
            summary.dirty,
            summary.package_manager,
        )
    } else {
        (None, false, None)
    };

    Some(RemoteProjectSummary {
        name,
        repository,
        updated_at,
        cloned_locally: local_path.is_some(),
        local_path: local_path.map(|path| path.to_string_lossy().to_string()),
        branch,
        dirty,
        package_manager,
    })
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

fn unpushed_commit_count(path: &Path) -> u32 {
    let has_upstream = tool_command("git")
        .ok()
        .and_then(|mut cmd| {
            cmd.current_dir(path)
                .args(["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .ok()
        })
        .map(|output| output.status.success())
        .unwrap_or(false);

    if !has_upstream {
        return 0;
    }

    git_output(path, &["rev-list", "--count", "@{u}..HEAD"])
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

fn working_tree_dirty(path: &Path) -> bool {
    git_output(path, &["status", "--porcelain"])
        .map(|status| !status.is_empty())
        .unwrap_or(false)
}

fn run_git_push(path: &Path) -> Result<String, String> {
    let output = tool_command("git")?
        .current_dir(path)
        .args(["push"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Could not run git push: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "git push failed.".into()
        } else {
            stderr
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(if stdout.is_empty() {
        "Pushed to GitHub.".into()
    } else {
        stdout
    })
}

fn age_key_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("Could not resolve app config dir: {e}"))?
        .join("age");
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create age key dir: {e}"))?;
    Ok(dir.join("keys.txt"))
}

fn read_age_public_key(key_file: &Path) -> Result<String, String> {
    if !key_file.is_file() {
        return Err("No age key on this machine.".into());
    }
  let from_tool = resolve_tool("age-keygen").and_then(|path| {
        Command::new(path)
            .arg("-y")
            .arg(key_file)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
    });
    if let Some(public_key) = from_tool.filter(|key| key.starts_with("age1")) {
        return Ok(public_key);
    }

    let raw = fs::read_to_string(key_file).map_err(|e| format!("Could not read age key: {e}"))?;
    for line in raw.lines() {
        let line = line.trim();
        if line.starts_with("# public key:") {
            return Ok(line.replace("# public key:", "").trim().to_string());
        }
        if line.starts_with("age1") {
            return Ok(line.to_string());
        }
    }

    Err("Could not read the public key from your age key file.".into())
}

fn age_key_status_inner(app: &AppHandle) -> AgeKeyStatus {
    let sops_available = resolve_tool("sops").is_some();
    let age_keygen_available = resolve_tool("age-keygen").is_some();
    let key_path = age_key_path(app);
    let has_key = key_path
        .as_ref()
        .map(|path| path.is_file())
        .unwrap_or(false);
    let public_key = if has_key {
        key_path
            .as_ref()
            .ok()
            .and_then(|path| read_age_public_key(path).ok())
    } else {
        None
    };

    let message = if !sops_available || !age_keygen_available {
        Some("Install sops + age below, or use Manual download.".into())
    } else if !has_key {
        Some("Generate or import an age key to encrypt and decrypt .env files.".into())
    } else {
        None
    };

    AgeKeyStatus {
        has_key,
        public_key,
        sops_available,
        age_keygen_available,
        message,
    }
}

fn run_sops(key_file: &Path, args: &[&str], cwd: &Path) -> Result<String, String> {
    let mut cmd = tool_command("sops")?;
    cmd.env("SOPS_AGE_KEY_FILE", key_file);
    cmd.current_dir(cwd)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = cmd
        .output()
        .map_err(|e| format!("Could not run sops: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if !output.status.success() {
        return Err(if stderr.is_empty() {
            "sops command failed.".into()
        } else {
            stderr
        });
    }

    Ok(stdout)
}

fn sops_yaml_content(public_key: &str) -> String {
    format!(
        "creation_rules:\n  - path_regex: \\.env\\.local$\n    age: '{public_key}'\n  - path_regex: \\.env\\.enc$\n    age: '{public_key}'\n"
    )
}

fn ensure_sops_yaml(project_path: &Path, public_key: &str) -> Result<(), String> {
    let yaml_path = project_path.join(SOPS_YAML);
    let content = sops_yaml_content(public_key);

    if yaml_path.exists() {
        let raw = fs::read_to_string(&yaml_path)
            .map_err(|e| format!("Could not read {SOPS_YAML}: {e}"))?;
        if raw.contains(public_key) && raw.contains(".env\\.local") && raw.contains(".env\\.enc") {
            return Ok(());
        }
    }

    fs::write(&yaml_path, content).map_err(|e| format!("Could not write {SOPS_YAML}: {e}"))?;
    Ok(())
}

fn env_paths_out_of_sync(project_path: &Path) -> bool {
    let local = project_path.join(ENV_LOCAL);
    let enc = project_path.join(ENV_ENC);
    if local.exists() && !enc.exists() {
        return true;
    }
    if !local.exists() || !enc.exists() {
        return false;
    }
    let local_mtime = fs::metadata(&local).and_then(|m| m.modified()).ok();
    let enc_mtime = fs::metadata(&enc).and_then(|m| m.modified()).ok();
    match (local_mtime, enc_mtime) {
        (Some(local_ts), Some(enc_ts)) => local_ts > enc_ts,
        _ => false,
    }
}

fn env_status_inner(app: &AppHandle, project_path: &Path) -> EnvStatus {
    let key_status = age_key_status_inner(app);
    let has_local = project_path.join(ENV_LOCAL).is_file();
    let has_encrypted = project_path.join(ENV_ENC).is_file();
    let out_of_sync = env_paths_out_of_sync(project_path);
    let tools_ready = key_status.sops_available && key_status.age_keygen_available;
    let can_encrypt = tools_ready && key_status.has_key && has_local;
    let can_decrypt = tools_ready && key_status.has_key && has_encrypted;

    let message = if !tools_ready {
        Some("Install sops and age-keygen from Settings → ENV keys.".into())
    } else if !key_status.has_key {
        Some("Set up your age key in Settings → ENV keys.".into())
    } else if has_encrypted && !has_local {
        Some("Encrypted env found. Decrypt to create .env.local on this machine.".into())
    } else if out_of_sync {
        Some("Local .env.local changed — Devkit will encrypt automatically when you push.".into())
    } else if has_local && !has_encrypted {
        Some("Encrypt .env.local to sync secrets through GitHub.".into())
    } else {
        None
    };

    EnvStatus {
        has_local,
        has_encrypted,
        out_of_sync,
        can_encrypt,
        can_decrypt,
        message,
    }
}

fn encrypt_env_inner(app: &AppHandle, path: &Path) -> Result<String, String> {
    if !path.join(ENV_LOCAL).is_file() {
        return Err(format!("{ENV_LOCAL} was not found in this project."));
    }

    let key_file = age_key_path(app)?;
    if !key_file.is_file() {
        return Err("No age key on this machine. Set one up in Settings → ENV keys.".into());
    }

    let public_key = read_age_public_key(&key_file)?;
    ensure_sops_yaml(path, &public_key)?;

    let encrypted = run_sops(
        &key_file,
        &[
            "--encrypt",
            "--input-type",
            "dotenv",
            "--output-type",
            "dotenv",
            "--filename-override",
            ENV_ENC,
            ENV_LOCAL,
        ],
        path,
    )?;

    fs::write(path.join(ENV_ENC), encrypted)
        .map_err(|e| format!("Could not write {ENV_ENC}: {e}"))?;

    Ok(format!("Encrypted {ENV_LOCAL} → {ENV_ENC}."))
}

fn encrypt_env_if_needed(app: &AppHandle, path: &Path) -> Result<Option<String>, String> {
    if !env_paths_out_of_sync(path) || !path.join(ENV_LOCAL).is_file() {
        return Ok(None);
    }
    encrypt_env_inner(app, path).map(Some)
}

fn commit_and_push_project(
    app: &AppHandle,
    path: &Path,
    message: Option<String>,
) -> Result<String, String> {
    let encrypted = encrypt_env_if_needed(app, path)?;
    let env_was_encrypted = encrypted.is_some();
    let dirty = working_tree_dirty(path);
    let mut notes: Vec<String> = encrypted.into_iter().collect();

    if dirty {
        let trimmed = message.unwrap_or_default().trim().to_string();
        let commit_message = if trimmed.is_empty() {
            if env_was_encrypted {
                "chore: update encrypted env".to_string()
            } else {
                return Err("Add a commit message for your changes.".into());
            }
        } else {
            trimmed
        };
        if commit_message.len() > 500 {
            return Err("Commit message is too long (max 500 characters).".into());
        }
        run_checked("git", &["add", "-A"], Some(path))?;
        run_checked("git", &["commit", "-m", &commit_message], Some(path))?;
    } else if unpushed_commit_count(path) == 0 {
        return Ok(if notes.is_empty() {
            "Nothing to push.".into()
        } else {
            notes.join(" ")
        });
    }

    notes.push(run_git_push(path)?);
    Ok(notes.join(" "))
}

fn decrypt_env_inner(app: &AppHandle, path: &Path) -> Result<String, String> {
    if !path.join(ENV_ENC).is_file() {
        return Err(format!("{ENV_ENC} was not found in this project."));
    }

    let key_file = age_key_path(app)?;
    if !key_file.is_file() {
        return Err("No age key on this machine. Import the same key from your other computer.".into());
    }

    let decrypted = run_sops(
        &key_file,
        &[
            "--decrypt",
            "--input-type",
            "dotenv",
            "--output-type",
            "dotenv",
            "--filename-override",
            ENV_ENC,
            ENV_ENC,
        ],
        path,
    )?;

    fs::write(path.join(ENV_LOCAL), decrypted)
        .map_err(|e| format!("Could not write {ENV_LOCAL}: {e}"))?;

    Ok(format!("Decrypted {ENV_ENC} → {ENV_LOCAL}."))
}

#[tauri::command]
fn age_key_status(app: AppHandle) -> AgeKeyStatus {
    age_key_status_inner(&app)
}

#[tauri::command]
fn generate_age_key(app: AppHandle) -> Result<AgeKeyStatus, String> {
    if resolve_tool("age-keygen").is_none() {
        return Err("age-keygen is not installed. Install sops and age from Settings → ENV keys.".into());
    }

    let key_file = age_key_path(&app)?;
    if key_file.exists() {
        return Err("An age key already exists on this machine.".into());
    }

    run_checked(
        "age-keygen",
        &["-o", key_file.to_string_lossy().as_ref()],
        None,
    )?;

    Ok(age_key_status_inner(&app))
}

#[tauri::command]
fn import_age_key(app: AppHandle, input: ImportAgeKeyInput) -> Result<AgeKeyStatus, String> {
    let secret = input.secret.trim();
    if !secret.starts_with("AGE-SECRET-KEY-") {
        return Err("Paste a full age private key (starts with AGE-SECRET-KEY-).".into());
    }

    let key_file = age_key_path(&app)?;
    fs::write(&key_file, format!("{secret}\n"))
        .map_err(|e| format!("Could not save age key: {e}"))?;
    let public_key = read_age_public_key(&key_file)?;
    let body = format!(
        "# Devkit age key — keep private\n# public key: {public_key}\n{secret}\n"
    );
    fs::write(&key_file, body).map_err(|e| format!("Could not save age key: {e}"))?;
    Ok(age_key_status_inner(&app))
}

#[tauri::command]
fn export_age_key(app: AppHandle) -> Result<String, String> {
    let key_file = age_key_path(&app)?;
    if !key_file.is_file() {
        return Err("No age key to export.".into());
    }
    fs::read_to_string(&key_file).map_err(|e| format!("Could not read age key: {e}"))
}

#[tauri::command]
fn open_sops_install_page() -> Result<(), String> {
    open_url(SOPS_INSTALL_URL)
}

fn resolve_brew() -> Result<PathBuf, String> {
    for candidate in ["/opt/homebrew/bin/brew", "/usr/local/bin/brew"] {
        let path = PathBuf::from(candidate);
        if path.is_file() {
            return Ok(path);
        }
    }
    for dir in tool_search_dirs() {
        let candidate = dir.join("brew");
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(
        "Homebrew was not found. Install Homebrew first, or use Manual download."
            .into(),
    )
}

fn run_program(program: &Path, args: &[&str], cwd: Option<&Path>) -> Result<String, String> {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("Could not run `{}`: {e}", program.display()))?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if !output.status.success() {
        return Err(format_command_failure(
            program.to_string_lossy().as_ref(),
            &stdout,
            &stderr,
        ));
    }

    Ok(if stdout.is_empty() { stderr } else { stdout })
}

fn install_env_tools_macos() -> Result<String, String> {
    let brew = resolve_brew()?;
    let need_sops = resolve_tool("sops").is_none();
    let need_age = resolve_tool("age-keygen").is_none();

    if !need_sops && !need_age {
        return Ok("sops and age-keygen are already installed.".into());
    }

  let mut args = vec!["install"];
    if need_sops {
        args.push("sops");
    }
    if need_age {
        args.push("age");
    }

    run_program(&brew, &args, None)?;
    Ok("Installed sops and age via Homebrew.".into())
}

fn install_env_tools_windows() -> Result<String, String> {
    let need_sops = resolve_tool("sops").is_none();
    let need_age = resolve_tool("age-keygen").is_none();

    if !need_sops && !need_age {
        return Ok("sops and age-keygen are already installed.".into());
    }

    if let Ok(msg) = install_env_tools_windows_winget(need_sops, need_age) {
        if resolve_tool("sops").is_some() && resolve_tool("age-keygen").is_some() {
            return Ok(msg);
        }
    }

    if let Ok(msg) = install_env_tools_windows_choco(need_sops, need_age) {
        if resolve_tool("sops").is_some() && resolve_tool("age-keygen").is_some() {
            return Ok(msg);
        }
    }

    let sops_ok = resolve_tool("sops").is_some();
    let age_ok = resolve_tool("age-keygen").is_some();
    if sops_ok && age_ok {
        return Ok("Installed sops and age.".into());
    }

    let mut missing = Vec::new();
    if !sops_ok {
        missing.push("sops");
    }
    if !age_ok {
        missing.push("age-keygen");
    }
    Err(format!(
        "Could not install {} automatically. Try Manual download or install winget/Chocolatey.",
        missing.join(" and ")
    ))
}

fn install_env_tools_windows_winget(need_sops: bool, need_age: bool) -> Result<String, String> {
    let winget = PathBuf::from("winget");
    let accept = [
        "--accept-package-agreements",
        "--accept-source-agreements",
    ];

    if need_sops && resolve_tool("sops").is_none() {
        run_program(
            &winget,
            &[
                "install",
                "-e",
                "--id",
                "Mozilla.SOPS",
                accept[0],
                accept[1],
            ],
            None,
        )?;
    }

    if need_age && resolve_tool("age-keygen").is_none() {
        let age_ids = ["FiloSottile.age", "jamesog.age", "age.age"];
        let mut installed = false;
        for id in age_ids {
            if run_program(
                &winget,
                &["install", "-e", "--id", id, accept[0], accept[1]],
                None,
            )
            .is_ok()
                && resolve_tool("age-keygen").is_some()
            {
                installed = true;
                break;
            }
        }
        if !installed && resolve_tool("age-keygen").is_none() {
            return Err("winget could not install age.".into());
        }
    }

    Ok("Installed via winget.".into())
}

fn install_env_tools_windows_choco(need_sops: bool, need_age: bool) -> Result<String, String> {
    let choco = PathBuf::from("choco");
    if need_sops && resolve_tool("sops").is_none() {
        run_program(&choco, &["install", "sops", "-y"], None)?;
    }
    if need_age && resolve_tool("age-keygen").is_none() {
        run_program(&choco, &["install", "age", "-y"], None)?;
    }
    Ok("Installed via Chocolatey.".into())
}

fn install_env_tools_inner() -> Result<InstallEnvToolsResult, String> {
    let before_sops = resolve_tool("sops").is_some();
    let before_age = resolve_tool("age-keygen").is_some();

    if before_sops && before_age {
        return Ok(InstallEnvToolsResult {
            ok: true,
            message: "sops and age-keygen are already installed.".into(),
            sops_available: true,
            age_keygen_available: true,
        });
    }

    let install_message = match std::env::consts::OS {
        "macos" => install_env_tools_macos(),
        "windows" => install_env_tools_windows(),
        other => Err(format!(
            "Automatic install is not supported on {other}. Use Manual download."
        )),
    };

    let sops_available = resolve_tool("sops").is_some();
    let age_keygen_available = resolve_tool("age-keygen").is_some();

    match install_message {
        Ok(message) => {
            let mut parts = vec![message];
            if sops_available && !before_sops {
                parts.push("sops is ready.".into());
            }
            if age_keygen_available && !before_age {
                parts.push("age-keygen is ready.".into());
            }
            if !sops_available || !age_keygen_available {
                parts.push(
                    "Some tools are still missing. Try Check again or Manual download.".into(),
                );
            }
            Ok(InstallEnvToolsResult {
                ok: sops_available && age_keygen_available,
                message: parts.join(" "),
                sops_available,
                age_keygen_available,
            })
        }
        Err(err) => {
            if sops_available && age_keygen_available {
                Ok(InstallEnvToolsResult {
                    ok: true,
                    message: "Tools are now available.".into(),
                    sops_available: true,
                    age_keygen_available: true,
                })
            } else {
                Err(err)
            }
        }
    }
}

#[tauri::command]
async fn install_env_tools() -> Result<InstallEnvToolsResult, String> {
    tauri::async_runtime::spawn_blocking(install_env_tools_inner)
        .await
        .map_err(|e| format!("Install failed: {e}"))?
}

#[tauri::command]
fn install_sops() -> Result<(), String> {
    open_url(SOPS_INSTALL_URL)
}

#[tauri::command]
fn env_status(app: AppHandle, path: String) -> Result<EnvStatus, String> {
    let path = safe_existing_dir(&path)?;
    Ok(env_status_inner(&app, &path))
}

#[tauri::command]
fn read_env_local(path: String) -> Result<String, String> {
    let path = safe_existing_dir(&path)?;
    let env_path = path.join(ENV_LOCAL);
    if !env_path.is_file() {
        return Ok(String::new());
    }
    fs::read_to_string(&env_path).map_err(|e| format!("Could not read {ENV_LOCAL}: {e}"))
}

#[tauri::command]
fn write_env_local(input: WriteEnvLocalInput) -> Result<(), String> {
    let path = safe_existing_dir(&input.path)?;
    fs::write(path.join(ENV_LOCAL), &input.content)
        .map_err(|e| format!("Could not write {ENV_LOCAL}: {e}"))?;
    Ok(())
}

#[tauri::command]
fn encrypt_env(app: AppHandle, path: String) -> Result<String, String> {
    let path = safe_existing_dir(&path)?;
    encrypt_env_inner(&app, &path)
}

#[tauri::command]
fn decrypt_env(app: AppHandle, path: String) -> Result<String, String> {
    let path = safe_existing_dir(&path)?;
    decrypt_env_inner(&app, &path)
}

fn project_summary(path: &Path) -> ProjectSummary {
    let is_git_repo = path.join(".git").exists();
    let branch = if is_git_repo {
        git_output(path, &["rev-parse", "--abbrev-ref", "HEAD"])
    } else {
        None
    };
    let dirty = if is_git_repo {
        working_tree_dirty(path)
    } else {
        false
    };
    let ahead = if is_git_repo {
        unpushed_commit_count(path)
    } else {
        0
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
        ahead,
        is_git_repo,
        package_manager: detect_package_manager(path),
        env_out_of_sync: env_paths_out_of_sync(path),
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
fn list_github_projects(root: String) -> Result<Vec<RemoteProjectSummary>, String> {
    let root = safe_existing_dir(&root)?;
    run_checked("gh", &["auth", "status"], None).map_err(|e| {
        if e.contains("not found") {
            format!("{e}\nInstall GitHub CLI from Settings.")
        } else {
            format_github_auth_error(&e)
        }
    })?;

    let local_by_repo = local_projects_by_repository(&root);

    let raw = run_checked(
        "gh",
        &[
            "repo",
            "list",
            "--limit",
            "200",
            "--json",
            "nameWithOwner,name,updatedAt,isArchived,repositoryTopics",
        ],
        None,
    )?;

    let repos: Vec<serde_json::Value> =
        serde_json::from_str(&raw).map_err(|e| format!("Could not parse GitHub repo list: {e}"))?;

    let mut seen = HashSet::new();
    let mut projects = Vec::new();

    for value in repos {
        if !github_repo_has_devkit_topic(&value) {
            continue;
        }
        if let Some(summary) = remote_summary_from_json(&value, &local_by_repo) {
            seen.insert(summary.repository.clone());
            projects.push(summary);
        }
    }

    for (repository, path) in &local_by_repo {
        if seen.contains(repository) {
            continue;
        }
        if let Some(value) = fetch_github_repo(repository) {
            if let Some(summary) = remote_summary_from_json(&value, &local_by_repo) {
                seen.insert(summary.repository.clone());
                projects.push(summary);
                continue;
            }
        }
        let name = repository
            .split('/')
            .nth(1)
            .unwrap_or(repository)
            .to_string();
        let summary = project_summary(path);
        projects.push(RemoteProjectSummary {
            name,
            repository: repository.clone(),
            updated_at: None,
            cloned_locally: true,
            local_path: Some(path.to_string_lossy().to_string()),
            branch: summary.branch,
            dirty: summary.dirty,
            package_manager: summary.package_manager,
        });
    }

    projects.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(projects)
}

#[tauri::command]
fn sync_all_projects(app: AppHandle, root: String) -> Result<Vec<SyncProjectResult>, String> {
    let projects = list_projects(root)?;
    let mut results = Vec::new();

    for project in projects {
        if !project.is_git_repo {
            results.push(SyncProjectResult {
                name: project.name.clone(),
                path: project.path.clone(),
                ok: true,
                message: "Skipped — not a Git repository.".into(),
            });
            continue;
        }

        let path = PathBuf::from(&project.path);
        let pull = tool_command("git")
            .and_then(|mut cmd| {
                cmd.current_dir(&path)
                    .args(["pull", "--ff-only"])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
                    .map_err(|e| format!("Could not run git pull: {e}"))
            });

        match pull {
            Ok(output) => {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    let pull_msg = if stdout.is_empty() {
                        "Already up to date.".to_string()
                    } else {
                        stdout
                    };

                    let dirty = working_tree_dirty(&path);
                    let message = if dirty {
                        format!("{pull_msg} Push skipped — commit your changes first.")
                    } else {
                        match commit_and_push_project(&app, &path, None) {
                            Ok(push_msg) if push_msg == "Nothing to push." => pull_msg,
                            Ok(push_msg) => format!("{pull_msg} {push_msg}"),
                            Err(err) => format!("{pull_msg} Push failed: {err}"),
                        }
                    };

                    results.push(SyncProjectResult {
                        name: project.name.clone(),
                        path: project.path.clone(),
                        ok: true,
                        message,
                    });
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    results.push(SyncProjectResult {
                        name: project.name.clone(),
                        path: project.path.clone(),
                        ok: false,
                        message: if stderr.is_empty() {
                            "git pull failed.".into()
                        } else {
                            stderr
                        },
                    });
                }
            }
            Err(err) => results.push(SyncProjectResult {
                name: project.name.clone(),
                path: project.path.clone(),
                ok: false,
                message: err,
            }),
        }
    }

    Ok(results)
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitCommitPushInput {
    path: String,
    message: Option<String>,
}

#[tauri::command]
fn git_commit_push(app: AppHandle, input: GitCommitPushInput) -> Result<String, String> {
    let path = safe_existing_dir(&input.path)?;
    if !path.join(".git").exists() {
        return Err("This folder is not a Git repository.".into());
    }

    commit_and_push_project(&app, &path, input.message)
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
        format_github_auth_error(&e)
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

    register_github_project(&remote, Some(&destination));

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
async fn clone_project(
    app: AppHandle,
    input: CloneProjectInput,
) -> Result<CloneProjectResult, String> {
    tauri::async_runtime::spawn_blocking(move || clone_project_inner(app, input))
        .await
        .map_err(|e| format!("Clone task failed: {e}"))?
}

fn clone_project_inner(app: AppHandle, input: CloneProjectInput) -> Result<CloneProjectResult, String> {
    let repository = input.repository.trim().to_string();
    validate_repository(&repository)?;

    let root = safe_existing_dir(&input.destination_root)?;
    let name = repository
        .split('/')
        .nth(1)
        .ok_or_else(|| "Repository must look like owner/repo.".to_string())?;
    validate_project_name(name)?;

    let destination = root.join(name);
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
        format_github_auth_error(&e)
    })?;
    emit_progress(&app, "auth", "done", None);

    emit_progress(
        &app,
        "clone",
        "running",
        Some(format!("Cloning {repository}…")),
    );
    run_checked(
        "gh",
        &["repo", "clone", &repository, name],
        Some(&root),
    )
    .map_err(|e| {
        emit_progress(&app, "clone", "error", Some(e.clone()));
        e
    })?;
    emit_progress(
        &app,
        "clone",
        "done",
        Some("Project cloned locally.".into()),
    );

    if !destination.exists() {
        let msg = format!(
            "Repository was cloned but not found at {}.",
            destination.display()
        );
        emit_progress(&app, "configure", "error", Some(msg.clone()));
        return Err(msg);
    }

    emit_progress(
        &app,
        "configure",
        "running",
        Some("Configuring Devkit metadata…".into()),
    );

    if !destination.join(".devkit.json").exists() {
        let meta = serde_json::json!({
            "name": name,
            "repository": repository,
            "createdAt": now_iso(),
            "clonedAt": now_iso(),
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
    }

    register_github_project(&repository, Some(&destination));
    emit_progress(&app, "configure", "done", None);

    if destination.join(ENV_ENC).exists() {
        emit_progress(
            &app,
            "env",
            "running",
            Some("Decrypting environment file…".into()),
        );
        match decrypt_env_inner(&app, &destination) {
            Ok(msg) => emit_progress(&app, "env", "done", Some(msg)),
            Err(e) => emit_progress(
                &app,
                "env",
                "done",
                Some(format!("Skipped env decrypt: {e}")),
            ),
        }
    }

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
        Some("Project is ready on this machine.".into()),
    );

    Ok(CloneProjectResult {
        path: destination.to_string_lossy().to_string(),
        repository,
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
            github_auth_status,
            github_auth_login,
            github_auth_login_with_token,
            github_auth_refresh,
            github_auth_logout,
            open_external_url,
            install_github_cli,
            age_key_status,
            generate_age_key,
            import_age_key,
            export_age_key,
            install_env_tools,
            open_sops_install_page,
            install_sops,
            env_status,
            read_env_local,
            write_env_local,
            encrypt_env,
            decrypt_env,
            list_projects,
            list_github_projects,
            sync_all_projects,
            open_in_cursor,
            git_pull,
            git_commit_push,
            list_templates,
            save_templates,
            create_project,
            clone_project,
            delete_project,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Devkit");
}
