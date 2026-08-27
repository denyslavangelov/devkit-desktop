import { useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  Boxes,
  Check,
  ChevronRight,
  CircleDot,
  Code2,
  FolderGit2,
  Github,
  Laptop,
  Plus,
  RefreshCw,
  Settings,
  SlidersHorizontal,
  TerminalSquare,
  Trash2,
  Wrench,
  X,
} from "lucide-react";
import { NewProjectModal } from "./components/NewProjectModal";
import { DeleteProjectModal } from "./components/DeleteProjectModal";
import {
  checkTools,
  getSystemInfo,
  gitPull,
  listProjects,
  listTemplates,
  openInCursor,
  saveTemplates,
  type DevkitTemplate,
  type ProjectSummary,
  type SystemInfo,
  type ToolStatus,
} from "./lib/tauri";

type View = "projects" | "templates" | "settings";

const STORAGE_KEY = "devkit.projectsRoot";

function App() {
  const [view, setView] = useState<View>("projects");
  const [root, setRoot] = useState(() => localStorage.getItem(STORAGE_KEY) ?? "");
  const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [templates, setTemplates] = useState<DevkitTemplate[]>([]);
  const [tools, setTools] = useState<ToolStatus[]>([]);
  const [system, setSystem] = useState<SystemInfo | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [showNewProject, setShowNewProject] = useState(false);
  const [projectToDelete, setProjectToDelete] = useState<ProjectSummary | null>(null);
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

  async function refresh() {
    setBusy(true);
    setMessage(null);
    try {
      const [sys, doctor, nextTemplates] = await Promise.all([
        getSystemInfo(),
        checkTools(),
        listTemplates(),
      ]);
      setSystem(sys);
      setTools(doctor);
      setTemplates(nextTemplates);
      if (root) setProjects(await listProjects(root));
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }

  useEffect(() => {
    void refresh();
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
    setProjects(await listProjects(selected));
  }

  async function handleOpen(project: ProjectSummary) {
    try {
      await openInCursor(project.path);
    } catch (error) {
      setMessage(String(error));
    }
  }

  async function handlePull(project: ProjectSummary) {
    setBusy(true);
    try {
      const result = await gitPull(project.path);
      setMessage(result || "Project updated.");
      if (root) setProjects(await listProjects(root));
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
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

    setBusy(true);
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
      setBusy(false);
    }
  }

  async function handleRemoveTemplate(id: string) {
    setBusy(true);
    try {
      const next = await saveTemplates(templates.filter((template) => template.id !== id));
      setTemplates(next);
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }

  function openNewProject() {
    if (!root) {
      setMessage("Choose a projects folder before creating a project.");
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

      <main className="main">
        <header className="topbar">
          <div>
            <h1>
              {view === "projects" ? "Projects" : view === "templates" ? "Templates" : "Settings"}
            </h1>
            <p>
              {view === "projects"
                ? "Your local projects, Git state and launch controls."
                : view === "templates"
                  ? "GitHub template repositories used for New Project."
                  : "Machine setup and Devkit health."}
            </p>
          </div>
          <button className="icon-button" onClick={() => void refresh()} disabled={busy} title="Refresh">
            <RefreshCw size={17} className={busy ? "spin" : ""} />
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

        {view === "projects" && (
          <section>
            <div className="section-actions">
              <div className="path-box">
                <span className="muted">Projects folder</span>
                <strong>{root || "Not configured"}</strong>
              </div>
              <button className="secondary" onClick={() => void chooseRoot()}>
                <SlidersHorizontal size={16} /> Choose folder
              </button>
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
            ) : projects.length === 0 ? (
              <EmptyState
                title="No projects found"
                description="Create one from a GitHub template, or add repositories to this folder."
                action="New project"
                onAction={openNewProject}
              />
            ) : (
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
                      <div className={`status ${project.dirty ? "warn" : "good"}`}>
                        <CircleDot size={12} />
                        {project.dirty ? "Changes" : "Clean"}
                      </div>
                    </div>
                    <div className="meta-row">
                      <span>Branch</span>
                      <strong>{project.branch ?? "—"}</strong>
                    </div>
                    <div className="meta-row">
                      <span>Git</span>
                      <strong>{project.isGitRepo ? "Connected" : "No repo"}</strong>
                    </div>
                    <div className="card-actions">
                      <button className="primary grow" onClick={() => void handleOpen(project)}>
                        <Code2 size={16} /> Open Cursor
                      </button>
                      <button
                        className="secondary"
                        disabled={!project.isGitRepo || busy}
                        onClick={() => void handlePull(project)}
                      >
                        <RefreshCw size={16} /> Pull
                      </button>
                      <button
                        className="icon-button danger-icon"
                        title="Delete project"
                        disabled={busy}
                        onClick={() => setProjectToDelete(project)}
                      >
                        <Trash2 size={15} />
                      </button>
                    </div>
                  </article>
                ))}
              </div>
            )}
          </section>
        )}

        {view === "templates" && (
          <section className="templates-layout">
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
                <button className="primary" disabled={busy} onClick={() => void handleAddTemplate()}>
                  <Plus size={16} /> Save template
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
                        disabled={busy}
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
          <section className="settings-grid">
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
                  <TerminalSquare size={16} /> Missing:{" "}
                  {missingRequired.map((x) => x.tool).join(", ")}
                </div>
              )}
            </div>

            <div className="panel">
              <div className="panel-title">
                <Github size={18} />
                <div>
                  <h3>Project location</h3>
                  <p>Machine-specific. Not synced to Git.</p>
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
