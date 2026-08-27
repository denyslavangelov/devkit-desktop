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
  isGitRepo: boolean;
  packageManager: string | null;
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

export async function getSystemInfo() {
  return invoke<SystemInfo>("system_info");
}

export async function checkTools() {
  return invoke<ToolStatus[]>("check_tools");
}

export async function listProjects(root: string) {
  return invoke<ProjectSummary[]>("list_projects", { root });
}

export async function openInCursor(path: string) {
  return invoke<void>("open_in_cursor", { path });
}

export async function gitPull(path: string) {
  return invoke<string>("git_pull", { path });
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
