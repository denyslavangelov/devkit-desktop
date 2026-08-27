import { useEffect, useState } from "react";
import {
  Check,
  ExternalLink,
  Github,
  KeyRound,
  LoaderCircle,
  LogOut,
  RefreshCw,
} from "lucide-react";
import {
  GITHUB_TOKEN_DOCS_URL,
  getGitHubAuthStatus,
  githubAuthLogin,
  githubAuthLoginWithToken,
  githubAuthLogout,
  githubAuthRefresh,
  installGithubCli,
  openExternalUrl,
  type GitHubAuthStatus,
} from "../lib/tauri";

const REQUIRED_SCOPES = ["repo", "delete_repo", "read:org", "gist"];

type Props = {
  busy?: boolean;
  onBusyChange?: (busy: boolean) => void;
  onMessage?: (message: string) => void;
  onStatusChange?: (status: GitHubAuthStatus) => void;
};

export function GitHubAuthPanel({ busy = false, onBusyChange, onMessage, onStatusChange }: Props) {
  const [status, setStatus] = useState<GitHubAuthStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [showTokenForm, setShowTokenForm] = useState(false);
  const [token, setToken] = useState("");
  const [action, setAction] = useState<string | null>(null);

  const panelBusy = busy || loading || action !== null;

  async function refreshStatus() {
    setLoading(true);
    try {
      const next = await getGitHubAuthStatus();
      setStatus(next);
      onStatusChange?.(next);
    } catch (error) {
      onMessage?.(String(error));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void refreshStatus();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function runAction(
    label: string,
    fn: () => Promise<GitHubAuthStatus>,
    successMessage?: string,
  ) {
    setAction(label);
    onBusyChange?.(true);
    try {
      const next = await fn();
      setStatus(next);
      onStatusChange?.(next);
      if (successMessage) onMessage?.(successMessage);
    } catch (error) {
      onMessage?.(String(error));
    } finally {
      setAction(null);
      onBusyChange?.(false);
    }
  }

  const connected = status?.authenticated && status.missingScopes.length === 0;
  const needsScopes = status?.authenticated && status.missingScopes.length > 0;

  return (
    <div className="panel github-auth-panel">
      <div className="panel-title">
        <Github size={18} />
        <div>
          <h3>GitHub account</h3>
          <p>Sign in once per machine. Devkit opens GitHub in your browser — no terminal needed.</p>
        </div>
      </div>

      {loading && !status ? (
        <div className="inline-empty">
          <LoaderCircle size={16} className="spin" /> Checking GitHub…
        </div>
      ) : (
        <>
          <div className={`github-auth-status ${connected ? "connected" : needsScopes ? "warn" : ""}`}>
            <div className="github-auth-avatar">
              <Github size={18} />
            </div>
            <div>
              {status?.ghAvailable ? (
                status.authenticated ? (
                  <>
                    <strong>
                      {needsScopes ? "Connected — permissions needed" : "Connected"}
                    </strong>
                    <span>
                      {status.username ? `@${status.username}` : "Signed in"}
                      {status.gitProtocol ? ` · Git over ${status.gitProtocol}` : ""}
                    </span>
                  </>
                ) : (
                  <>
                    <strong>Not connected</strong>
                    <span>Sign in to create, clone, and sync projects.</span>
                  </>
                )
              ) : (
                <>
                  <strong>GitHub CLI not installed</strong>
                  <span>Install it once, then sign in below.</span>
                </>
              )}
              {status?.message && <p className="github-auth-message">{status.message}</p>}
            </div>
          </div>

          {status?.ghAvailable && status.authenticated && (
            <div className="scope-list">
              {REQUIRED_SCOPES.map((scope) => {
                const ok = !status.missingScopes.includes(scope);
                return (
                  <span className={`scope-pill ${ok ? "ok" : "missing"}`} key={scope}>
                    {ok ? <Check size={11} /> : null}
                    {scope}
                  </span>
                );
              })}
            </div>
          )}

          <div className="github-auth-actions">
            {!status?.ghAvailable && (
              <button
                className="primary"
                disabled={panelBusy}
                onClick={() => {
                  setAction("install");
                  onBusyChange?.(true);
                  void installGithubCli()
                    .then(() => onMessage?.("Opened GitHub CLI install page."))
                    .catch((error) => onMessage?.(String(error)))
                    .finally(() => {
                      setAction(null);
                      onBusyChange?.(false);
                    });
                }}
              >
                {action === "install" ? (
                  <LoaderCircle size={16} className="spin" />
                ) : (
                  <ExternalLink size={16} />
                )}
                Install GitHub CLI
              </button>
            )}

            {!status?.ghAvailable && (
              <button
                className="secondary"
                disabled={panelBusy}
                onClick={() => void refreshStatus()}
              >
                <RefreshCw size={16} className={loading ? "spin" : ""} />
                Check again
              </button>
            )}

            {status?.ghAvailable && !status.authenticated && (
              <button
                className="primary"
                disabled={panelBusy}
                onClick={() =>
                  void runAction(
                    "login",
                    githubAuthLogin,
                    "GitHub account connected.",
                  )
                }
              >
                {action === "login" ? (
                  <LoaderCircle size={16} className="spin" />
                ) : (
                  <Github size={16} />
                )}
                Sign in with GitHub
              </button>
            )}

            {status?.ghAvailable && needsScopes && (
              <button
                className="primary"
                disabled={panelBusy}
                onClick={() =>
                  void runAction(
                    "refresh",
                    githubAuthRefresh,
                    "GitHub permissions updated.",
                  )
                }
              >
                {action === "refresh" ? (
                  <LoaderCircle size={16} className="spin" />
                ) : (
                  <RefreshCw size={16} />
                )}
                Refresh permissions
              </button>
            )}

            {status?.ghAvailable && status.authenticated && (
              <button
                className="secondary"
                disabled={panelBusy}
                onClick={() =>
                  void runAction("logout", githubAuthLogout, "Signed out of GitHub.")
                }
              >
                {action === "logout" ? (
                  <LoaderCircle size={16} className="spin" />
                ) : (
                  <LogOut size={16} />
                )}
                Sign out
              </button>
            )}

            {status?.ghAvailable && (
              <button
                className="secondary"
                disabled={panelBusy}
                onClick={() => void refreshStatus()}
              >
                <RefreshCw size={16} className={loading ? "spin" : ""} />
                Check again
              </button>
            )}
            {status?.ghAvailable && (
              <button
                className="secondary"
                disabled={panelBusy}
                onClick={() => setShowTokenForm((current) => !current)}
              >
                <KeyRound size={16} />
                {showTokenForm ? "Hide token sign-in" : "Use access token"}
              </button>
            )}
          </div>

          {showTokenForm && status?.ghAvailable && (
            <div className="token-form">
              <p>
                Paste a classic personal access token with{" "}
                <code>repo</code>, <code>delete_repo</code>, <code>read:org</code>, and{" "}
                <code>gist</code> scopes.
              </p>
              <div className="field">
                <label htmlFor="github-token">Personal access token</label>
                <input
                  id="github-token"
                  type="password"
                  value={token}
                  onChange={(event) => setToken(event.target.value)}
                  placeholder="ghp_… or github_pat_…"
                  autoComplete="off"
                />
              </div>
              <div className="token-form-actions">
                <button
                  className="secondary"
                  disabled={panelBusy}
                  onClick={() => void openExternalUrl(GITHUB_TOKEN_DOCS_URL)}
                >
                  <ExternalLink size={15} /> Create token on GitHub
                </button>
                <button
                  className="primary"
                  disabled={panelBusy || !token.trim()}
                  onClick={() =>
                    void runAction(
                      "token",
                      () => githubAuthLoginWithToken(token),
                      "GitHub token saved.",
                    ).then(() => setToken(""))
                  }
                >
                  {action === "token" ? (
                    <LoaderCircle size={16} className="spin" />
                  ) : (
                    <KeyRound size={16} />
                  )}
                  Save token
                </button>
              </div>
            </div>
          )}

          {action === "login" || action === "refresh" ? (
            <p className="github-auth-hint">
              Complete the sign-in in your browser, then return here. This may take a few seconds.
            </p>
          ) : null}
        </>
      )}
    </div>
  );
}
