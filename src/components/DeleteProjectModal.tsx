import { useEffect, useState } from "react";
import { Checkbox } from "./Checkbox";
import { AlertTriangle, LoaderCircle, X } from "lucide-react";
import { deleteProject, type ProjectSummary } from "../lib/tauri";

type Props = {
  open: boolean;
  project: ProjectSummary | null;
  projectsRoot: string;
  onClose: () => void;
  onDeleted: (message: string) => void;
};

export function DeleteProjectModal({ open, project, projectsRoot, onClose, onDeleted }: Props) {
  const [deleteRemote, setDeleteRemote] = useState(false);
  const [confirmName, setConfirmName] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setDeleteRemote(false);
    setConfirmName("");
    setBusy(false);
    setError(null);
  }, [open, project?.path]);

  if (!open || !project) return null;

  const canDelete = confirmName.trim() === project.name && !busy;

  async function handleDelete() {
    if (!project || !canDelete) return;
    const target = project;
    setBusy(true);
    setError(null);
    try {
      const result = await deleteProject({
        path: target.path,
        projectsRoot,
        deleteRemote,
      });
      const parts = [`Deleted local project “${target.name}”.`];
      if (result.deletedRemote && result.remote) {
        parts.push(`Also deleted GitHub repo ${result.remote}.`);
      }
      onDeleted(parts.join(" "));
      onClose();
    } catch (err) {
      setError(String(err));
      setBusy(false);
    }
  }

  return (
    <div className="modal-backdrop" role="presentation" onClick={() => !busy && onClose()}>
      <div
        className="modal modal-danger"
        role="dialog"
        aria-modal="true"
        aria-labelledby="delete-project-title"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="modal-head">
          <div>
            <h2 id="delete-project-title">Delete project</h2>
            <p>This removes the local folder from your projects directory.</p>
          </div>
          <button className="icon-button" onClick={onClose} disabled={busy} aria-label="Close">
            <X size={16} />
          </button>
        </div>

        <div className="modal-body">
          <div className="danger-banner">
            <AlertTriangle size={16} />
            <div>
              <strong>{project.name}</strong>
              <p>{project.path}</p>
            </div>
          </div>

          <div className="toggle-list">
            <Checkbox
              checked={deleteRemote}
              onChange={setDeleteRemote}
              disabled={!project.isGitRepo || busy}
              label="Also delete the GitHub repository"
              description={
                project.isGitRepo
                  ? "Permanent — the remote repo cannot be recovered."
                  : "No git repo detected for this folder."
              }
            />
          </div>

          {deleteRemote && (
            <div className="danger-note">
              Remote delete is permanent. The GitHub repo cannot be recovered from Devkit.
            </div>
          )}

          <div className="field">
            <label htmlFor="confirm-delete-name">
              Type <strong>{project.name}</strong> to confirm
            </label>
            <input
              id="confirm-delete-name"
              value={confirmName}
              onChange={(event) => setConfirmName(event.target.value)}
              placeholder={project.name}
              autoFocus
              disabled={busy}
            />
          </div>

          {error && <div className="modal-error">{error}</div>}
        </div>

        <div className="modal-foot">
          <button className="secondary" onClick={onClose} disabled={busy}>
            Cancel
          </button>
          <button className="danger" disabled={!canDelete} onClick={() => void handleDelete()}>
            {busy ? (
              <>
                <LoaderCircle size={16} className="spin" /> Deleting…
              </>
            ) : deleteRemote ? (
              "Delete local + GitHub"
            ) : (
              "Delete local project"
            )}
          </button>
        </div>
      </div>
    </div>
  );
}
