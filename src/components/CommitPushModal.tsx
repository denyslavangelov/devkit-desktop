import { useEffect, useState } from "react";
import { LoaderCircle, Upload, X } from "lucide-react";
import { gitCommitPush, type ProjectSummary } from "../lib/tauri";

type Props = {
  open: boolean;
  project: ProjectSummary | null;
  onClose: () => void;
  onPushed: (message: string) => void;
  title?: string;
  description?: string;
  defaultMessage?: string;
};

export function CommitPushModal({
  open,
  project,
  onClose,
  onPushed,
  title = "Commit and push",
  description = "Save local changes to GitHub so your other machines can pull them.",
  defaultMessage = "",
}: Props) {
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setMessage(defaultMessage);
    setBusy(false);
    setError(null);
  }, [open, project?.path, defaultMessage]);

  if (!open || !project) return null;

  const canPush = message.trim().length > 0 && !busy;

  async function handlePush() {
    if (!project || !canPush) return;
    setBusy(true);
    setError(null);
    try {
      const result = await gitCommitPush({
        path: project.path,
        message: message.trim(),
      });
      onPushed(result || "Pushed to GitHub.");
      onClose();
    } catch (err) {
      setError(String(err));
      setBusy(false);
    }
  }

  return (
    <div className="modal-backdrop" role="presentation" onClick={() => !busy && onClose()}>
      <div
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="commit-push-title"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="modal-head">
          <div>
            <h2 id="commit-push-title">{title}</h2>
            <p>{description}</p>
          </div>
          <button className="icon-button" onClick={onClose} disabled={busy} aria-label="Close">
            <X size={16} />
          </button>
        </div>

        <div className="modal-body">
          <div className="push-context">
            <strong>{project.name}</strong>
            <span>{project.branch ?? "branch"}</span>
          </div>

          <div className="field">
            <label htmlFor="commit-message">Commit message</label>
            <textarea
              id="commit-message"
              className="commit-message-input"
              value={message}
              onChange={(event) => setMessage(event.target.value)}
              placeholder="Describe what you changed…"
              rows={4}
              autoFocus
              disabled={busy}
            />
          </div>

          <p className="commit-hint">
            Devkit will stage all changes, commit, and push to GitHub.
          </p>

          {error && <div className="modal-error">{error}</div>}
        </div>

        <div className="modal-foot">
          <button className="secondary" onClick={onClose} disabled={busy}>
            Cancel
          </button>
          <button className="primary" disabled={!canPush} onClick={() => void handlePush()}>
            {busy ? (
              <>
                <LoaderCircle size={16} className="spin" /> Pushing…
              </>
            ) : (
              <>
                <Upload size={16} /> Commit & push
              </>
            )}
          </button>
        </div>
      </div>
    </div>
  );
}
