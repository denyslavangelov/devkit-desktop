import { useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  Boxes,
  Check,
  ChevronRight,
  CircleDot,
  Code2,
  Download,
  FolderGit2,
  Github,
  Laptop,
  Plus,
  RefreshCw,
  Settings,
  SlidersHorizontal,
  Trash2,
  Upload,
  Wrench,
  X,
} from "lucide-react";
import { AgeKeyPanel } from "./components/AgeKeyPanel";
import { Button } from "./components/Button";
import { CommitPushModal } from "./components/CommitPushModal";
import { CloneProjectModal } from "./components/CloneProjectModal";
import { GitHubAuthPanel } from "./components/GitHubAuthPanel";
import { LoadingScreen, ProjectGridSkeleton } from "./components/LoadingScreen";
import { NewProjectModal } from "./components/NewProjectModal";
import { DeleteProjectModal } from "./components/DeleteProjectModal";
import {
  checkTools,
  getGitHubAuthStatus,
  getSystemInfo,
  gitCommitPush,
  gitPull,
  listGithubProjects,
  listProjects,
  listTemplates,
  openInCursor,
  saveTemplates,
  syncAllProjects,
  type DevkitTemplate,
  type GitHubAuthStatus,
  type ProjectSummary,
  type RemoteProjectSummary,
  type SystemInfo,
  type ToolStatus,
} from "./lib/tauri";

type View = "projects" | "templates" | "settings";

const STORAGE_KEY = "devkit.projectsRoot";

function App() {
  const [view, setView] = useState<View>("projects");
  const [root, setRoot] = useState(() => localStorage.getItem(STORAGE_KEY) ?? "");
  const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [githubProjects, setGithubProjects] = useState<RemoteProjectSummary[]>([]);
  const [templates, setTemplates] = useState<DevkitTemplate[]>([]);
  const [githubAuth, setGithubAuth] = useState<GitHubAuthStatus | null>(null);
  const [tools, setTools] = useState<ToolStatus[]>([]);
  const [system, setSystem] = useState<SystemInfo | null>(null);
  const [booting, setBooting] = useState(true);
  const [bootPhase, setBootPhase] = useState("Starting Devkit");
  const [projectsLoading, setProjectsLoading] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [pullingPath, setPullingPath] = useState<string | null>(null);
  const [pushingPath, setPushingPath] = useState<string | null>(null);
  const [projectToPush, setProjectToPush] = useState<ProjectSummary | null>(null);
  const [syncingAll, setSyncingAll] = useState(false);
  const [templateSaving, setTemplateSaving] = useState(false);
  const [authBusy, setAuthBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [showNewProject, setShowNewProject] = useState(false);
  const [projectToDelete, setProjectToDelete] = useState<ProjectSummary | null>(null);
  const [projectToClone, setProjectToClone] = useState<RemoteProjectSummary | null>(null);
  const [templateDraft, setTemplateDraft] = useState({
    name: "",
    repository: "",
    description: "",
  });

  const missingRequired = useMemo(
    () =>
      tools.filter(
        (tool) =>
          ["git", "gh", "node", "pnpm", "sops", "age-keygen", "cursor"].includes(tool.tool) &&
          !tool.available,
      ),
    [tools],
  );

  const remoteOnlyProjects = useMemo(
    () => githubProjects.filter((project) => !project.clonedLocally),
    [githubProjects],
  );

  const githubReady =
    githubAuth?.ghAvailable &&
    githubAuth.authenticated &&
    githubAuth.missingScopes.length === 0;

  async function loadProjects(options?: { skeleton?: boolean }) {
    if (!root) {
      setProjects([]);
      setGithubProjects([]);
      return;
    }

    if (options?.skeleton) setProjectsLoading(true);
    try {
      const [local, remote] = await Promise.all([
        listProjects(root),
        listGithubProjects(root),
      ]);
      setProjects(local);
      setGithubProjects(remote);
    } catch (error) {
      setMessage(String(error));
    } finally {
      setProjectsLoading(false);
    }
  }

  async function refresh(options?: { skeleton?: boolean }) {
    setRefreshing(true);
    setMessage(null);
    try {
      setBootPhase("Checking tools and GitHub");
      const [sys, doctor, nextTemplates, auth] = await Promise.all([
        getSystemInfo(),
        checkTools(),
        listTemplates(),
        getGitHubAuthStatus(),
      ]);
      setSystem(sys);
      setTools(doctor);
      setTemplates(nextTemplates);
      setGithubAuth(auth);
      setBootPhase("Loading projects");
      await loadProjects({ skeleton: options?.skeleton });
    } catch (error) {
      setMessage(String(error));
    } finally {
      setRefreshing(false);
    }
  }

  useEffect(() => {
    void (async () => {
      setBootPhase("Starting Devkit");
      await refresh({ skeleton: Boolean(root) });
      setBooting(false);
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function chooseRoot() {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Choose your projects folder",
    });
    if (typeof selected !== "string") return;
    localStorage.setItem(STORAGE_KEY, selected);
    setRoot(selected);
    setProjectsLoading(true);
    try {
      const [local, remote] = await Promise.all([
        listProjects(selected),
        listGithubProjects(selected),
      ]);
      setProjects(local);
      setGithubProjects(remote);
    } catch (error) {
      setMessage(String(error));
    } finally {
      setProjectsLoading(false);
    }
  }

  async function handleOpen(project: ProjectSummary) {
    try {
      await openInCursor(project.path);
    } catch (error) {
      setMessage(String(error));
    }
  }

  async function handleSyncAll() {
    if (!root) return;
    setSyncingAll(true);
    try {
      const results = await syncAllProjects(root);
      const failed = results.filter((result) => !result.ok);
      if (failed.length > 0) {
        setMessage(
          `Synced ${results.length - failed.length}/${results.length}. ${failed[0].name}: ${failed[0].message}`,
        );
      } else if (results.length === 0) {
        setMessage("No local projects to sync.");
      } else {
        setMessage(`Pulled latest changes for ${results.length} project(s).`);
      }
      await loadProjects();
    } catch (error) {
      setMessage(String(error));
    } finally {
      setSyncingAll(false);
    }
  }

  async function handlePush(project: ProjectSummary) {
    if (project.dirty) {
      setProjectToPush(project);
      return;
    }

    if (project.ahead === 0 && !project.envOutOfSync) {
      setMessage("Nothing to push.");
      return;
    }

    setPushingPath(project.path);
    try {
      const result = await gitCommitPush({ path: project.path });
      setMessage(result || "Pushed to GitHub.");
      await loadProjects();
    } catch (error) {
      setMessage(String(error));
    } finally {
      setPushingPath(null);
    }
  }

  async function handlePull(project: ProjectSummary) {
    setPullingPath(project.path);
    try {
      const result = await gitPull(project.path);
      setMessage(result || "Project updated.");
      await loadProjects();
    } catch (error) {
      setMessage(String(error));
    } finally {
      setPullingPath(null);
    }
  }

  async function handleAddTemplate() {
    const name = templateDraft.name.trim();
    const repository = templateDraft.repository.trim();
    const description = templateDraft.description.trim();
    if (!name || !repository) {
      setMessage("Template name and repository are required.");
      return;
    }

    setTemplateSaving(true);
    try {
      const next = await saveTemplates([
        ...templates,
        {
          id: crypto.randomUUID(),
          name,
          repository,
          ...(description ? { description } : {}),
        },
      ]);
      setTemplates(next);
      setTemplateDraft({ name: "", repository: "", description: "" });
      setMessage(`Saved template ${name}.`);
    } catch (error) {
      setMessage(String(error));
    } finally {
      setTemplateSaving(false);
    }
  }

  async function handleRemoveTemplate(id: string) {
    setTemplateSaving(true);
    try {
      const next = await saveTemplates(templates.filter((template) => template.id !== id));
      setTemplates(next);
    } catch (error) {
      setMessage(String(error));
    } finally {
      setTemplateSaving(false);
    }
  }

  function openNewProject() {
    if (!root) {
      setMessage("Choose a projects folder before creating a project.");
      return;
    }
    if (!githubReady) {
      setView("settings");
      setMessage("Connect GitHub in Settings first.");
      return;
    }
    if (templates.length === 0) {
      setView("templates");
      setMessage("Add a GitHub template repository first.");
      return;
    }
    setShowNewProject(true);
  }

  return (
    <div className="app-shell">
      {booting && (
        <LoadingScreen
          fullScreen
          title={bootPhase}
          subtitle="Getting your workspace ready"
        />
      )}

      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark">
            <Code2 size={16} />
          </div>
          <div className="brand-copy">
            <span>Devkit</span>
            <small>Desktop</small>
          </div>
        </div>
        <nav>
          <NavItem
            icon={<FolderGit2 size={18} />}
            label="Projects"
            active={view === "projects"}
            onClick={() => setView("projects")}
          />
          <NavItem
            icon={<Boxes size={18} />}
            label="Templates"
            active={view === "templates"}
            onClick={() => setView("templates")}
          />
          <NavItem
            icon={<Settings size={18} />}
            label="Settings"
            active={view === "settings"}
            onClick={() => setView("settings")}
          />
        </nav>
        <div className="sidebar-footer">
          <div className="machine-pill">
            <Laptop size={15} />
            <span>{system?.hostname ?? "This computer"}</span>
            <span className="online-dot" />
          </div>
          <span className="muted tiny">
            {system ? `${system.os} · ${system.arch}` : "Detecting system…"}
          </span>
        </div>
      </aside>

      <main className={`main ${booting ? "main-booting" : ""}`}>
        {refreshing && !booting && <div className="refresh-strip" aria-hidden />}
        <header className="topbar">
          <div>
            <h1>
              {view === "projects" ? "Projects" : view === "templates" ? "Templates" : "Settings"}
            </h1>
            <p>
              {view === "projects"
                ? "Local projects, GitHub sync, and launch controls."
                : view === "templates"
                  ? "GitHub template repositories used for New Project."
                  : "Machine setup and Devkit health."}
            </p>
          </div>
          <button
            className="icon-button"
            onClick={() => void refresh()}
            disabled={refreshing || booting}
            title="Refresh"
          >
            <RefreshCw size={17} className={refreshing ? "spin" : ""} />
          </button>
        </header>

        {message && (
          <div className="notice">
            {message}
            <button onClick={() => setMessage(null)}>
              <X size={14} />
            </button>
          </div>
        )}

        {view === "projects" && !githubReady && (
          <div className="notice github-connect-banner">
            <Github size={16} />
            <span>
              {githubAuth?.ghAvailable
                ? githubAuth.authenticated
                  ? "GitHub needs extra permissions to create and delete repos."
                  : "Connect GitHub to create projects and sync across your machines."
                : "Install GitHub CLI and sign in to unlock create, clone, and sync."}
            </span>
            <button className="secondary" onClick={() => setView("settings")}>
              Open Settings
            </button>
          </div>
        )}

        {view === "projects" && (
          <section className="view-panel view-active">
            <div className="section-actions">
              <div className="path-box">
                <span className="muted">Projects folder</span>
                <strong>{root || "Not configured"}</strong>
              </div>
              <button className="secondary" onClick={() => void chooseRoot()}>
                <SlidersHorizontal size={16} /> Choose folder
              </button>
              <Button
                variant="secondary"
                icon={<RefreshCw size={16} />}
                loading={syncingAll}
                disabled={!root || projects.length === 0 || projectsLoading}
                onClick={() => void handleSyncAll()}
              >
                Sync all
              </Button>
              <button className="primary" onClick={openNewProject}>
                <Plus size={16} /> New project
              </button>
            </div>

            {!root ? (
              <EmptyState
                title="Choose your Projects folder"
                description="Devkit will scan it for Git and Node projects. Nothing is uploaded anywhere."
                action="Choose folder"
                onAction={() => void chooseRoot()}
              />
            ) : projectsLoading ? (
              <div className="projects-section">
                <ProjectGridSkeleton count={3} />
              </div>
            ) : projects.length === 0 && remoteOnlyProjects.length === 0 ? (
              <EmptyState
                title="No projects found"
                description="Create one from a GitHub template, or add repositories to this folder."
                action="New project"
                onAction={openNewProject}
              />
            ) : (
              <>
                <div className="projects-section">
                  <div className="section-label">
                    <Laptop size={15} />
                    <span>On this machine</span>
                    <span className="section-count">{projects.length}</span>
                  </div>
                  <div className="project-grid">
                    {projects.map((project) => (
                      <article className="project-card" key={project.path}>
                        <div className="project-card-head">
                          <div className="project-icon">
                            <FolderGit2 size={20} />
                          </div>
                          <div className="project-title">
                            <h3>{project.name}</h3>
                            <span>{project.packageManager ?? "Project"}</span>
                          </div>
                          <div className={`status ${project.dirty ? "warn" : project.ahead > 0 ? "warn" : "good"}`}>
                            <CircleDot size={12} />
                            {project.dirty
                              ? "Changes"
                              : project.ahead > 0
                                ? `${project.ahead} unpushed`
                                : "Clean"}
                          </div>
                        </div>
                        <div className="meta-row">
                          <span>Branch</span>
                          <strong>{project.branch ?? "—"}</strong>
                        </div>
                        <div className="meta-row">
                          <span>Git</span>
                          <strong>
                            {project.isGitRepo
                              ? project.ahead > 0
                                ? `${project.ahead} commit(s) to push`
                                : "Connected"
                              : "No repo"}
                          </strong>
                        </div>
                        <div className="card-actions card-actions-git">
                          <button className="primary grow" onClick={() => void handleOpen(project)}>
                            <Code2 size={16} /> Open Cursor
                          </button>
                          <button
                            className="secondary git-action"
                            disabled={!project.isGitRepo || pullingPath === project.path}
                            onClick={() => void handlePull(project)}
                            title="Pull from GitHub"
                          >
                            <RefreshCw
                              size={16}
                              className={pullingPath === project.path ? "spin" : ""}
                            />
                            Pull
                          </button>
                          <button
                            className="secondary git-action"
                            disabled={
                              !project.isGitRepo ||
                              pushingPath === project.path ||
                              (!project.dirty && project.ahead === 0 && !project.envOutOfSync)
                            }
                            onClick={() => void handlePush(project)}
                            title={
                              project.dirty
                                ? "Commit and push changes"
                                : "Push to GitHub"
                            }
                          >
                            <Upload
                              size={16}
                              className={pushingPath === project.path ? "spin" : ""}
                            />
                            Push
                          </button>
                          <button
                            className="icon-button danger-icon"
                            title="Delete project"
                            disabled={pullingPath !== null || pushingPath !== null || syncingAll}
                            onClick={() => setProjectToDelete(project)}
                          >
                            <Trash2 size={15} />
                          </button>
                        </div>
                      </article>
                    ))}
                  </div>
                </div>

                {remoteOnlyProjects.length > 0 && (
                  <div className="projects-section remote-section">
                    <div className="section-label">
                      <Github size={15} />
                      <span>On GitHub — not on this machine</span>
                      <span className="section-count">{remoteOnlyProjects.length}</span>
                    </div>
                    <p className="section-hint">
                      Created on another computer. Clone here, then Pull and Push to stay in sync.
                    </p>
                    <div className="project-grid">
                      {remoteOnlyProjects.map((project) => (
                        <article className="project-card remote-only" key={project.repository}>
                          <div className="project-card-head">
                            <div className="project-icon remote">
                              <Github size={18} />
                            </div>
                            <div className="project-title">
                              <h3>{project.name}</h3>
                              <span>{project.repository}</span>
                            </div>
                            <div className="status remote">
                              <CircleDot size={12} />
                              Remote
                            </div>
                          </div>
                          <div className="meta-row">
                            <span>Updated</span>
                            <strong>
                              {project.updatedAt
                                ? new Date(project.updatedAt).toLocaleDateString()
                                : "—"}
                            </strong>
                          </div>
                          <div className="card-actions">
                            <button
                              className="primary grow"
                              disabled={authBusy || syncingAll}
                              onClick={() => setProjectToClone(project)}
                            >
                              <Download size={16} /> Clone here
                            </button>
                          </div>
                        </article>
                      ))}
                    </div>
                  </div>
                )}
              </>
            )}
          </section>
        )}

        {view === "templates" && (
          <section className="templates-layout view-panel view-active">
            <div className="panel">
              <div className="panel-title">
                <Github size={18} />
                <div>
                  <h3>Add template</h3>
                  <p>Must be a GitHub template repository (`owner/repo`).</p>
                </div>
              </div>
              <div className="form-grid compact">
                <div className="field">
                  <label htmlFor="template-name">Name</label>
                  <input
                    id="template-name"
                    value={templateDraft.name}
                    onChange={(event) =>
                      setTemplateDraft((current) => ({ ...current, name: event.target.value }))
                    }
                    placeholder="Next.js starter"
                  />
                </div>
                <div className="field">
                  <label htmlFor="template-repo">Repository</label>
                  <input
                    id="template-repo"
                    value={templateDraft.repository}
                    onChange={(event) =>
                      setTemplateDraft((current) => ({
                        ...current,
                        repository: event.target.value,
                      }))
                    }
                    placeholder="owner/repo"
                  />
                </div>
                <div className="field">
                  <label htmlFor="template-description">Description</label>
                  <input
                    id="template-description"
                    value={templateDraft.description}
                    onChange={(event) =>
                      setTemplateDraft((current) => ({
                        ...current,
                        description: event.target.value,
                      }))
                    }
                    placeholder="Optional"
                  />
                </div>
                <button
                  className="primary"
                  disabled={templateSaving}
                  onClick={() => void handleAddTemplate()}
                >
                  {templateSaving ? (
                    <>
                      <RefreshCw size={16} className="spin" /> Saving…
                    </>
                  ) : (
                    <>
                      <Plus size={16} /> Save template
                    </>
                  )}
                </button>
              </div>
            </div>

            <div className="panel">
              <div className="panel-title">
                <Boxes size={18} />
                <div>
                  <h3>Saved templates</h3>
                  <p>Stored in this machine’s Devkit config folder.</p>
                </div>
              </div>
              {templates.length === 0 ? (
                <div className="inline-empty">No templates yet. Add one to unlock New Project.</div>
              ) : (
                <div className="template-list">
                  {templates.map((template) => (
                    <div className="template-row" key={template.id}>
                      <div>
                        <strong>{template.name}</strong>
                        <span>{template.repository}</span>
                        {template.description && <p>{template.description}</p>}
                      </div>
                      <button
                        className="icon-button"
                        title="Remove template"
                        disabled={templateSaving}
                        onClick={() => void handleRemoveTemplate(template.id)}
                      >
                        <Trash2 size={15} />
                      </button>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </section>
        )}

        {view === "settings" && (
          <section className="settings-grid view-panel view-active">
            <GitHubAuthPanel
              busy={authBusy}
              onBusyChange={setAuthBusy}
              onMessage={setMessage}
              onStatusChange={setGithubAuth}
            />

            <AgeKeyPanel
              onMessage={setMessage}
              onToolsInstalled={() => void checkTools().then(setTools)}
            />

            <div className="panel">
              <div className="panel-title">
                <Wrench size={18} />
                <div>
                  <h3>Doctor</h3>
                  <p>Tools Devkit expects on this computer.</p>
                </div>
              </div>
              <div className="tool-list">
                {tools.map((tool) => (
                  <div className="tool-row" key={tool.tool}>
                    <div className={tool.available ? "tool-icon ok" : "tool-icon missing"}>
                      {tool.available ? <Check size={13} /> : <X size={13} />}
                    </div>
                    <strong>{tool.tool}</strong>
                    <span>{tool.version ?? "Not found"}</span>
                  </div>
                ))}
              </div>
              {missingRequired.length > 0 && (
                <div className="doctor-warning">
                  Missing tools: {missingRequired.map((x) => x.tool).join(", ")}. Install them from
                  their websites — Devkit opens the links for GitHub CLI in the panel above.
                </div>
              )}
            </div>

            <div className="panel">
              <div className="panel-title">
                <Github size={18} />
                <div>
                  <h3>Project location</h3>
                  <p>Projects sync across machines via GitHub. Use Clone and Pull.</p>
                </div>
              </div>
              <div className="field">
                <label>Projects folder</label>
                <div className="field-action">
                  <input value={root} readOnly placeholder="Choose a folder" />
                  <button onClick={() => void chooseRoot()}>Browse</button>
                </div>
              </div>
            </div>
          </section>
        )}
      </main>

      <NewProjectModal
        open={showNewProject}
        templates={templates}
        destinationRoot={root}
        onClose={() => setShowNewProject(false)}
        onCreated={(text) => {
          setMessage(text);
          void refresh();
        }}
      />
      <CommitPushModal
        open={!!projectToPush}
        project={projectToPush}
        onClose={() => setProjectToPush(null)}
        onPushed={(text) => {
          setMessage(text);
          void loadProjects();
        }}
      />
      <CloneProjectModal
        open={!!projectToClone}
        repository={projectToClone?.repository ?? null}
        destinationRoot={root}
        onClose={() => setProjectToClone(null)}
        onCloned={(text) => {
          setMessage(text);
          void refresh();
        }}
      />
      <DeleteProjectModal
        open={!!projectToDelete}
        project={projectToDelete}
        projectsRoot={root}
        onClose={() => setProjectToDelete(null)}
        onDeleted={(text) => {
          setMessage(text);
          void refresh();
        }}
      />
    </div>
  );
}

function NavItem({
  icon,
  label,
  active,
  onClick,
}: {
  icon: React.ReactNode;
  label: string;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button className={`nav-item ${active ? "active" : ""}`} onClick={onClick}>
      {icon}
      <span>{label}</span>
      <ChevronRight size={14} />
    </button>
  );
}

function EmptyState({
  title,
  description,
  action,
  onAction,
}: {
  title: string;
  description: string;
  action: string;
  onAction: () => void;
}) {
  return (
    <div className="empty">
      <div className="empty-icon">
        <FolderGit2 size={25} />
      </div>
      <h2>{title}</h2>
      <p>{description}</p>
      <button className="primary" onClick={onAction}>
        {action}
      </button>
    </div>
  );
}

export default App;
