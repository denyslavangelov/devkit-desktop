import { useEffect, useMemo, useRef, useState } from "react";
import { Check, FolderGit2, LoaderCircle, Sparkles, X } from "lucide-react";
import {
  createProject,
  onCreateProjectProgress,
  type CreateProjectProgress,
  type DevkitTemplate,
} from "../lib/tauri";

const STEPS = [
  { id: "auth", label: "Checking GitHub auth" },
  { id: "create", label: "Creating GitHub repository" },
  { id: "clone", label: "Cloning project" },
  { id: "configure", label: "Configuring Devkit" },
  { id: "install", label: "Installing dependencies" },
  { id: "open", label: "Opening in Cursor" },
  { id: "ready", label: "Ready" },
] as const;

type StepId = (typeof STEPS)[number]["id"];

type Props = {
  open: boolean;
  templates: DevkitTemplate[];
  destinationRoot: string;
  onClose: () => void;
  onCreated: (message: string) => void;
};

async function waitForPaint() {
  await new Promise<void>((resolve) => {
    requestAnimationFrame(() => {
      requestAnimationFrame(() => resolve());
    });
  });
}

export function NewProjectModal({ open, templates, destinationRoot, onClose, onCreated }: Props) {
  const [name, setName] = useState("");
  const [templateId, setTemplateId] = useState("");
  const [owner, setOwner] = useState("");
  const [isPrivate, setIsPrivate] = useState(true);
  const [installDependencies, setInstallDependencies] = useState(true);
  const [openInCursor, setOpenInCursor] = useState(true);
  const [phase, setPhase] = useState<"form" | "creating" | "success" | "error">("form");
  const [error, setError] = useState<string | null>(null);
  const [progress, setProgress] = useState<Partial<Record<StepId, CreateProjectProgress>>>({});
  const wasOpen = useRef(false);

  const selectedTemplate = useMemo(
    () => templates.find((template) => template.id === templateId) ?? null,
    [templateId, templates],
  );

  // Reset only when the modal opens, not when templates refresh mid-create.
  useEffect(() => {
    if (open && !wasOpen.current) {
      setName("");
      setOwner("");
      setIsPrivate(true);
      setInstallDependencies(true);
      setOpenInCursor(true);
      setPhase("form");
      setError(null);
      setProgress({});
      setTemplateId(templates[0]?.id ?? "");
    }
    if (!open) {
      setPhase("form");
      setError(null);
      setProgress({});
    }
    wasOpen.current = open;
  }, [open, templates]);

  useEffect(() => {
    if (!open || phase !== "creating") return;
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void onCreateProjectProgress((event) => {
      if (cancelled) return;
      setProgress((current) => ({
        ...current,
        [event.step]: event,
      }));
    }).then((fn) => {
      if (cancelled) {
        fn();
        return;
      }
      unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [open, phase]);

  if (!open) return null;

  const busy = phase === "creating" || phase === "success";
  const showProgress = phase !== "form";

  const canSubmit =
    phase === "form" &&
    name.trim().length > 0 &&
    !!selectedTemplate &&
    destinationRoot.trim().length > 0;

  const visibleSteps = STEPS.filter((step) => {
    if (step.id === "install" && !installDependencies && !progress.install) return false;
    if (step.id === "open" && !openInCursor && !progress.open) return false;
    return true;
  });

  const doneCount = visibleSteps.filter((step) => progress[step.id]?.status === "done").length;
  const progressRatio = visibleSteps.length
    ? Math.min(1, doneCount / visibleSteps.length)
    : 0;
  const activeStep =
    visibleSteps.find((step) => progress[step.id]?.status === "running") ??
    visibleSteps.find((step) => !progress[step.id] || progress[step.id]?.status === "pending");

  async function handleCreate() {
    if (!selectedTemplate || !canSubmit) return;

    setPhase("creating");
    setError(null);
    setProgress({
      auth: {
        step: "auth",
        status: "running",
        message: "Starting project setup…",
      },
    });

    // Paint the loading screen before the long native create call begins.
    await waitForPaint();

    try {
      const result = await createProject({
        name: name.trim(),
        templateRepository: selectedTemplate.repository,
        owner: owner.trim(),
        private: isPrivate,
        destinationRoot,
        installDependencies,
        openInCursor,
      });
      setPhase("success");
      setProgress((current) => ({
        ...current,
        ready: { step: "ready", status: "done", message: "Project is ready." },
      }));
      await new Promise((resolve) => setTimeout(resolve, 1000));
      onCreated(`Created ${result.repository} at ${result.path}`);
      onClose();
    } catch (err) {
      setError(String(err));
      setPhase("error");
    }
  }

  return (
    <div className="modal-backdrop" role="presentation" onClick={() => !busy && onClose()}>
      <div
        className={`modal ${showProgress ? "modal-creating" : ""} ${phase === "success" ? "modal-success" : ""} ${phase === "error" ? "modal-danger" : ""}`}
        role="dialog"
        aria-modal="true"
        aria-labelledby="new-project-title"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="modal-head">
          <div>
            <h2 id="new-project-title">
              {phase === "success"
                ? "Project ready"
                : phase === "creating"
                  ? "Creating project"
                  : phase === "error"
                    ? "Create failed"
                    : "New project"}
            </h2>
            <p>
              {phase === "success"
                ? "Handing off to your workspace…"
                : phase === "creating"
                  ? "Repo, clone, install, and local setup in progress."
                  : phase === "error"
                    ? "Something went wrong during creation."
                    : "Create a GitHub repo from a template and clone it locally."}
            </p>
          </div>
          <button className="icon-button" onClick={onClose} disabled={busy} aria-label="Close">
            <X size={16} />
          </button>
        </div>

        {!showProgress ? (
          <div className="modal-body">
            {templates.length === 0 ? (
              <div className="modal-empty">
                Add at least one template in the Templates tab before creating a project.
              </div>
            ) : (
              <div className="form-grid">
                <div className="field">
                  <label htmlFor="project-name">Project name</label>
                  <input
                    id="project-name"
                    value={name}
                    onChange={(event) => setName(event.target.value)}
                    placeholder="my-app"
                    autoFocus
                  />
                </div>

                <div className="field">
                  <label htmlFor="project-template">Template</label>
                  <select
                    id="project-template"
                    value={templateId}
                    onChange={(event) => setTemplateId(event.target.value)}
                  >
                    {templates.map((template) => (
                      <option key={template.id} value={template.id}>
                        {template.name} ({template.repository})
                      </option>
                    ))}
                  </select>
                </div>

                <div className="field">
                  <label htmlFor="project-owner">GitHub owner</label>
                  <input
                    id="project-owner"
                    value={owner}
                    onChange={(event) => setOwner(event.target.value)}
                    placeholder="Your user or org (optional)"
                  />
                </div>

                <div className="field">
                  <label>Visibility</label>
                  <div className="choice-row">
                    <button
                      type="button"
                      className={`choice ${isPrivate ? "active" : ""}`}
                      onClick={() => setIsPrivate(true)}
                    >
                      Private
                    </button>
                    <button
                      type="button"
                      className={`choice ${!isPrivate ? "active" : ""}`}
                      onClick={() => setIsPrivate(false)}
                    >
                      Public
                    </button>
                  </div>
                </div>

                <div className="field">
                  <label>Local destination</label>
                  <input value={destinationRoot || "Choose a projects folder first"} readOnly />
                </div>

                <div className="toggle-list">
                  <label className="toggle-row">
                    <input
                      type="checkbox"
                      checked={installDependencies}
                      onChange={(event) => setInstallDependencies(event.target.checked)}
                    />
                    <span>Install dependencies after clone</span>
                  </label>
                  <label className="toggle-row">
                    <input
                      type="checkbox"
                      checked={openInCursor}
                      onChange={(event) => setOpenInCursor(event.target.checked)}
                    />
                    <span>Open in Cursor when ready</span>
                  </label>
                </div>
              </div>
            )}
          </div>
        ) : (
          <div className="modal-body create-stage" aria-live="polite">
            <div
              className={`create-hero ${phase === "success" ? "is-success" : ""} ${phase === "error" ? "is-error" : ""}`}
            >
              <div className="create-orbit create-orbit-a" />
              <div className="create-orbit create-orbit-b" />
              <div className="create-orbit create-orbit-c" />
              <div className="create-core">
                {phase === "success" ? (
                  <Check size={28} />
                ) : phase === "error" ? (
                  <X size={28} />
                ) : (
                  <FolderGit2 size={26} className="create-core-icon" />
                )}
              </div>
              <div className="create-spark create-spark-1">
                <Sparkles size={12} />
              </div>
              <div className="create-spark create-spark-2">
                <Sparkles size={10} />
              </div>
            </div>

            <div className="create-status-line">
              <LoaderCircle
                size={14}
                className={phase === "creating" ? "spin" : "create-status-idle"}
              />
              <span>
                {phase === "success"
                  ? "Finished"
                  : phase === "error"
                    ? "Stopped"
                    : (activeStep?.label ?? "Starting…")}
              </span>
            </div>

            <div className="create-meter">
              <div className="create-meter-track">
                <div
                  className="create-meter-fill"
                  style={{
                    transform: `scaleX(${
                      phase === "error"
                        ? Math.max(progressRatio, 0.08)
                        : phase === "success"
                          ? 1
                          : Math.max(progressRatio, 0.12)
                    })`,
                  }}
                />
              </div>
              <div className="create-meter-meta">
                <span>{phase === "creating" ? "In progress" : phase}</span>
                <strong>
                  {Math.round((phase === "success" ? 1 : Math.max(progressRatio, 0.12)) * 100)}%
                </strong>
              </div>
            </div>

            <div className="progress-list">
              {visibleSteps.map((step, index) => {
                const state = progress[step.id];
                const status = state?.status ?? "pending";
                return (
                  <div
                    className={`progress-row ${status}`}
                    key={step.id}
                    style={{ animationDelay: `${index * 55}ms` }}
                  >
                    <div className="progress-icon">
                      {status === "done" ? (
                        <Check size={13} />
                      ) : status === "running" ? (
                        <LoaderCircle size={13} className="spin" />
                      ) : status === "error" ? (
                        <X size={13} />
                      ) : (
                        <span className="progress-dot" />
                      )}
                    </div>
                    <div>
                      <strong>{step.label}</strong>
                      {state?.message && <p>{state.message}</p>}
                    </div>
                    {status === "running" && <span className="progress-scan" aria-hidden />}
                  </div>
                );
              })}
            </div>

            {error && (
              <div className="error-enter">
                <div className="modal-error" ref={(node) => node?.scrollTo(0, node.scrollHeight)}>
                  {error}
                </div>
                <div className="modal-error-actions">
                  <button
                    type="button"
                    onClick={() => {
                      void navigator.clipboard.writeText(error);
                    }}
                  >
                    Copy error
                  </button>
                </div>
              </div>
            )}
          </div>
        )}

        <div className="modal-foot">
          <button className="secondary" onClick={onClose} disabled={busy}>
            {phase === "error" ? "Close" : "Cancel"}
          </button>
          {phase === "form" && (
            <button className="primary" disabled={!canSubmit} onClick={() => void handleCreate()}>
              Create project
            </button>
          )}
          {phase === "creating" && (
            <button className="primary" disabled>
              Creating…
            </button>
          )}
          {phase === "error" && (
            <button
              className="primary"
              onClick={() => {
                setError(null);
                setProgress({});
                setPhase("form");
              }}
            >
              Try again
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
