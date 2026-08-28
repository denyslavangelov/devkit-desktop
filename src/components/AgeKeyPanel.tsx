import { useEffect, useState } from "react";
import {
  Check,
  Copy,
  Download,
  ExternalLink,
  KeyRound,
  LoaderCircle,
  RefreshCw,
  Shield,
} from "lucide-react";
import {
  exportAgeKey,
  generateAgeKey,
  getAgeKeyStatus,
  importAgeKey,
  installEnvTools,
  openSopsInstallPage,
  type AgeKeyStatus,
} from "../lib/tauri";

type Props = {
  onMessage?: (message: string) => void;
  onToolsInstalled?: () => void;
};

export function AgeKeyPanel({ onMessage, onToolsInstalled }: Props) {
  const [status, setStatus] = useState<AgeKeyStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [action, setAction] = useState<string | null>(null);
  const [showImport, setShowImport] = useState(false);
  const [importText, setImportText] = useState("");
  const [installHint, setInstallHint] = useState<string | null>(null);

  async function refresh() {
    setLoading(true);
    try {
      setStatus(await getAgeKeyStatus());
    } catch (error) {
      onMessage?.(String(error));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const toolsReady = status?.sopsAvailable && status?.ageKeygenAvailable;

  return (
    <div className="panel age-key-panel">
      <div className="panel-title">
        <Shield size={18} />
        <div>
          <h3>ENV keys</h3>
          <p>
            One age key per machine. Devkit can install sops + age for you, then import the same key
            on Mac and Windows.
          </p>
        </div>
      </div>

      {loading && !status ? (
        <div className="inline-empty">
          <LoaderCircle size={16} className="spin" /> Checking keys…
        </div>
      ) : (
        <>
          <div
            className={`age-key-status ${status?.hasKey ? "ready" : ""} ${!toolsReady ? "warn" : ""}`}
          >
            <strong>
              {status?.hasKey
                ? "Encryption key ready"
                : toolsReady
                  ? "No encryption key yet"
                  : "Tools missing"}
            </strong>
            {status?.publicKey && (
              <code className="age-public-key">{status.publicKey}</code>
            )}
            {status?.message && <p>{status.message}</p>}
            {installHint && <p className="env-install-hint">{installHint}</p>}
          </div>

          <div className="github-auth-actions">
            {!toolsReady && (
              <>
                <button
                  className="primary"
                  disabled={action !== null}
                  onClick={() => {
                    setAction("install");
                    setInstallHint(
                      "Installing sops and age… This may take a minute. You may see a system prompt.",
                    );
                    void installEnvTools()
                      .then(async (result) => {
                        setStatus(await getAgeKeyStatus());
                        onMessage?.(result.message);
                        onToolsInstalled?.();
                        if (!result.ok) {
                          setInstallHint(
                            "Install finished but some tools are still missing. Try Check again.",
                          );
                        } else {
                          setInstallHint(null);
                        }
                      })
                      .catch((e) => {
                        onMessage?.(String(e));
                        setInstallHint("Automatic install failed. Try Manual download.");
                      })
                      .finally(() => setAction(null));
                  }}
                >
                  {action === "install" ? (
                    <LoaderCircle size={16} className="spin" />
                  ) : (
                    <Download size={16} />
                  )}
                  Install sops + age
                </button>
                <button
                  className="secondary"
                  disabled={action !== null}
                  onClick={() => {
                    setAction("manual");
                    void openSopsInstallPage()
                      .then(() => onMessage?.("Opened manual install page."))
                      .catch((e) => onMessage?.(String(e)))
                      .finally(() => setAction(null));
                  }}
                >
                  <ExternalLink size={16} /> Manual download
                </button>
              </>
            )}

            {toolsReady && !status?.hasKey && (
              <button
                className="primary"
                disabled={action !== null}
                onClick={() => {
                  setAction("generate");
                  void generateAgeKey()
                    .then((next) => {
                      setStatus(next);
                      onMessage?.("Age key generated on this machine.");
                    })
                    .catch((e) => onMessage?.(String(e)))
                    .finally(() => setAction(null));
                }}
              >
                {action === "generate" ? (
                  <LoaderCircle size={16} className="spin" />
                ) : (
                  <KeyRound size={16} />
                )}
                Generate key
              </button>
            )}

            {toolsReady && (
              <button
                className="secondary"
                disabled={action !== null}
                onClick={() => setShowImport((v) => !v)}
              >
                <KeyRound size={16} />
                {showImport ? "Hide import" : "Import key"}
              </button>
            )}

            {status?.hasKey && (
              <button
                className="secondary"
                disabled={action !== null}
                onClick={() => {
                  setAction("export");
                  void exportAgeKey()
                    .then((secret) => {
                      void navigator.clipboard.writeText(secret.trim());
                      onMessage?.("Private key copied. Paste it on your other computer.");
                    })
                    .catch((e) => onMessage?.(String(e)))
                    .finally(() => setAction(null));
                }}
              >
                {action === "export" ? (
                  <LoaderCircle size={16} className="spin" />
                ) : (
                  <Copy size={16} />
                )}
                Copy private key
              </button>
            )}

            <button className="secondary" disabled={loading || action !== null} onClick={() => void refresh()}>
              <RefreshCw size={16} className={loading ? "spin" : ""} />
              Check again
            </button>
          </div>

          {showImport && toolsReady && (
            <div className="token-form">
              <p>
                Paste the private key from your other machine (starts with{" "}
                <code>AGE-SECRET-KEY-</code>).
              </p>
              <div className="field">
                <label htmlFor="age-import">Private key</label>
                <textarea
                  id="age-import"
                  className="commit-message-input"
                  value={importText}
                  onChange={(e) => setImportText(e.target.value)}
                  placeholder="AGE-SECRET-KEY-…"
                  rows={4}
                />
              </div>
              <button
                className="primary"
                disabled={!importText.trim() || action !== null}
                onClick={() => {
                  setAction("import");
                  void importAgeKey(importText)
                    .then((next) => {
                      setStatus(next);
                      setImportText("");
                      setShowImport(false);
                      onMessage?.("Age key imported.");
                    })
                    .catch((e) => onMessage?.(String(e)))
                    .finally(() => setAction(null));
                }}
              >
                {action === "import" ? (
                  <LoaderCircle size={16} className="spin" />
                ) : (
                  <Check size={16} />
                )}
                Save imported key
              </button>
            </div>
          )}
        </>
      )}
    </div>
  );
}
