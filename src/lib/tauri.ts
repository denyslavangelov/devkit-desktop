import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type SystemInfo = {
  os: string;
  arch: string;
  hostname: string;
};

export type ToolStatus = {
  tool: string;
  available: boolean;
  version: string | null;
};

export type ProjectSummary = {
  name: string;
  path: string;
  branch: string | null;
  dirty: boolean;
  ahead: number;
  isGitRepo: boolean;
  packageManager: string | null;
  envOutOfSync: boolean;
};

export type DevkitTemplate = {
  id: string;
  name: string;
  repository: string;
  description?: string;
};

export type CreateProjectInput = {
  name: string;
  templateRepository: string;
  owner: string;
  private: boolean;
  destinationRoot: string;
  installDependencies: boolean;
  openInCursor: boolean;
};

export type CreateProjectProgress = {
  step: string;
  status: "running" | "done" | "error" | string;
  message: string | null;
};

export type CreateProjectResult = {
  path: string;
  repository: string;
  packageManager: string | null;
};

export type DeleteProjectInput = {
  path: string;
  projectsRoot: string;
  deleteRemote: boolean;
};

export type DeleteProjectResult = {
  deletedLocal: boolean;
  deletedRemote: boolean;
  remote: string | null;
};

export type RemoteProjectSummary = {
  name: string;
  repository: string;
  updatedAt: string | null;
  clonedLocally: boolean;
  localPath: string | null;
  branch: string | null;
  dirty: boolean;
  packageManager: string | null;
};

export type CloneProjectInput = {
  repository: string;
  destinationRoot: string;
  installDependencies: boolean;
  openInCursor: boolean;
};

export type CloneProjectResult = {
  path: string;
  repository: string;
  packageManager: string | null;
};

export type SyncProjectResult = {
  name: string;
  path: string;
  ok: boolean;
  message: string;
};

export type GitCommitPushInput = {
  path: string;
  message?: string;
};

export type GitHubAuthStatus = {
  ghAvailable: boolean;
  authenticated: boolean;
  username: string | null;
  scopes: string[];
  missingScopes: string[];
  gitProtocol: string | null;
  message: string | null;
};

export const GITHUB_TOKEN_DOCS_URL =
  "https://github.com/settings/tokens/new?scopes=repo,delete_repo,read:org,gist&description=Devkit+Desktop";

export type AgeKeyStatus = {
  hasKey: boolean;
  publicKey: string | null;
  sopsAvailable: boolean;
  ageKeygenAvailable: boolean;
  message: string | null;
};

export async function getAgeKeyStatus() {
  return invoke<AgeKeyStatus>("age_key_status");
}

export async function generateAgeKey() {
  return invoke<AgeKeyStatus>("generate_age_key");
}

export async function importAgeKey(secret: string) {
  return invoke<AgeKeyStatus>("import_age_key", { input: { secret } });
}

export async function exportAgeKey() {
  return invoke<string>("export_age_key");
}

export async function installEnvTools() {
  return invoke<InstallEnvToolsResult>("install_env_tools");
}

export async function openSopsInstallPage() {
  return invoke<void>("open_sops_install_page");
}

export type InstallEnvToolsResult = {
  ok: boolean;
  message: string;
  sopsAvailable: boolean;
  ageKeygenAvailable: boolean;
};

export async function getSystemInfo() {
  return invoke<SystemInfo>("system_info");
}

export async function getGitHubAuthStatus() {
  return invoke<GitHubAuthStatus>("github_auth_status");
}

export async function githubAuthLogin() {
  return invoke<GitHubAuthStatus>("github_auth_login");
}

export async function githubAuthLoginWithToken(token: string) {
  return invoke<GitHubAuthStatus>("github_auth_login_with_token", { token });
}

export async function githubAuthRefresh() {
  return invoke<GitHubAuthStatus>("github_auth_refresh");
}

export async function githubAuthLogout() {
  return invoke<GitHubAuthStatus>("github_auth_logout");
}

export async function openExternalUrl(url: string) {
  return invoke<void>("open_external_url", { url });
}

export async function installGithubCli() {
  return invoke<void>("install_github_cli");
}

export async function checkTools() {
  return invoke<ToolStatus[]>("check_tools");
}

export async function listProjects(root: string) {
  return invoke<ProjectSummary[]>("list_projects", { root });
}

export async function listGithubProjects(root: string) {
  return invoke<RemoteProjectSummary[]>("list_github_projects", { root });
}

export async function syncAllProjects(root: string) {
  return invoke<SyncProjectResult[]>("sync_all_projects", { root });
}

export async function openInCursor(path: string) {
  return invoke<void>("open_in_cursor", { path });
}

export async function gitPull(path: string) {
  return invoke<string>("git_pull", { path });
}

export async function gitCommitPush(input: GitCommitPushInput) {
  return invoke<string>("git_commit_push", { input });
}

export async function listTemplates() {
  return invoke<DevkitTemplate[]>("list_templates");
}

export async function saveTemplates(templates: DevkitTemplate[]) {
  return invoke<DevkitTemplate[]>("save_templates", { templates });
}

export async function createProject(input: CreateProjectInput) {
  return invoke<CreateProjectResult>("create_project", { input });
}

export async function cloneProject(input: CloneProjectInput) {
  return invoke<CloneProjectResult>("clone_project", { input });
}

export async function deleteProject(input: DeleteProjectInput) {
  return invoke<DeleteProjectResult>("delete_project", { input });
}

export function onCreateProjectProgress(
  handler: (progress: CreateProjectProgress) => void,
): Promise<UnlistenFn> {
  return listen<CreateProjectProgress>("create-project-progress", (event) => {
    handler(event.payload);
  });
}
