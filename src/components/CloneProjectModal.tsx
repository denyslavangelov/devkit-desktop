import { useEffect, useRef, useState } from "react";
import { Check, FolderGit2, LoaderCircle, Sparkles, X } from "lucide-react";
import { Checkbox } from "./Checkbox";
import {
  cloneProject,
  onCreateProjectProgress,
  type CreateProjectProgress,
} from "../lib/tauri";

const STEPS = [
  { id: "auth", label: "Checking GitHub auth" },
  { id: "clone", label: "Cloning project" },
  { id: "configure", label: "Configuring Devkit" },
  { id: "install", label: "Installing dependencies" },
  { id: "open", label: "Opening in Cursor" },
  { id: "ready", label: "Ready" },
] as const;

type StepId = (typeof STEPS)[number]["id"];

type Props = {
  open: boolean;
  repository: string | null;
  destinationRoot: string;
  onClose: () => void;
  onCloned: (message: string) => void;
};

async function waitForPaint() {
  await new Promise<void>((resolve) => {
    requestAnimationFrame(() => {
      requestAnimationFrame(() => resolve());
    });
  });
}

export function CloneProjectModal({
  open,
  repository,
  destinationRoot,
  onClose,
  onCloned,
}: Props) {
  const [installDependencies, setInstallDependencies] = useState(true);
  const [openInCursor, setOpenInCursor] = useState(true);
  const [phase, setPhase] = useState<"form" | "cloning" | "success" | "error">("form");
  const [error, setError] = useState<string | null>(null);
  const [progress, setProgress] = useState<Partial<Record<StepId, CreateProjectProgress>>>({});
  const wasOpen = useRef(false);

  useEffect(() => {
    if (open && !wasOpen.current) {
      setInstallDependencies(true);
      setOpenInCursor(true);
      setPhase("form");
      setError(null);
      setProgress({});
    }
    if (!open) {
      setPhase("form");
      setError(null);
      setProgress({});
    }
    wasOpen.current = open;
  }, [open]);

  useEffect(() => {
    if (!open || phase !== "cloning") return;
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

  if (!open || !repository) return null;

  const busy = phase === "cloning" || phase === "success";
  const showProgress = phase !== "form";
  const canSubmit = phase === "form" && destinationRoot.trim().length > 0;

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

  async function handleClone() {
    if (!canSubmit || !repository) return;

    setPhase("cloning");
    setError(null);
    setProgress({
      auth: {
        step: "auth",
        status: "running",
        message: "Starting clone…",
      },
    });

    await waitForPaint();

    try {
      const result = await cloneProject({
        repository,
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
      onCloned(`Cloned ${result.repository} to ${result.path}`);
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
        aria-labelledby="clone-project-title"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="modal-head">
          <div>
            <h2 id="clone-project-title">
              {phase === "success"
                ? "Project ready"
                : phase === "cloning"
                  ? "Cloning project"
                  : phase === "error"
                    ? "Clone failed"
                    : "Clone to this machine"}
            </h2>
            <p>
              {phase === "success"
                ? "Handing off to your workspace…"
                : phase === "cloning"
                  ? "Pulling the repo from GitHub onto this computer."
                  : phase === "error"
                    ? "Something went wrong during clone."
                    : `Download ${repository} into your projects folder.`}
            </p>
          </div>
          <button className="icon-button" onClick={onClose} disabled={busy} aria-label="Close">
            <X size={16} />
          </button>
        </div>

        {!showProgress ? (
          <div className="modal-body">
            <div className="form-grid">
              <div className="field">
                <label>Repository</label>
                <input value={repository} readOnly />
              </div>
              <div className="field">
                <label>Local destination</label>
                <input value={destinationRoot || "Choose a projects folder first"} readOnly />
              </div>
              <div className="toggle-list">
                <Checkbox
                  checked={installDependencies}
                  onChange={setInstallDependencies}
                  label="Install dependencies after clone"
                  description="Runs pnpm, npm, or yarn based on the project."
                />
                <Checkbox
                  checked={openInCursor}
                  onChange={setOpenInCursor}
                  label="Open in Cursor when ready"
                  description="Launch the editor after setup finishes."
                />
              </div>
            </div>
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
                className={phase === "cloning" ? "spin" : "create-status-idle"}
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
                <span>{phase === "cloning" ? "In progress" : phase}</span>
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
            <button className="primary" disabled={!canSubmit} onClick={() => void handleClone()}>
              Clone project
            </button>
          )}
          {phase === "cloning" && (
            <button className="primary" disabled>
              Cloning…
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
