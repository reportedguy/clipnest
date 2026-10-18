import { useEffect, useState } from "react";
import { Cpu, Download, Pause, Trash2 } from "lucide-react";
import { ipc, type AiStatus } from "../lib/ipc";
import { AI_STATUS, useTauriEvent } from "../lib/events";
import { formatBytes } from "../lib/format";
import { Button } from "./ui";
import { useToast } from "./Toast";

export function useAiStatus(): AiStatus | null {
  const [status, setStatus] = useState<AiStatus | null>(null);
  useEffect(() => {
    ipc.aiStatus().then(setStatus, () => {});
  }, []);
  useTauriEvent<AiStatus>(AI_STATUS, setStatus);
  return status;
}

/**
 * Set up / download progress / ready state for the local AI, in Settings (onboarding has its
 * own model picker). Downloading is explicit (this is the app's only network use),
 * resumable, and never blocks capturing.
 */
export function AiSetup({ showManage = false }: { showManage?: boolean }) {
  const status = useAiStatus();
  const toast = useToast();
  const fail = (e: unknown) => toast({ message: String(e), tone: "error" });
  if (!status) return null;

  const pct = status.totalBytes > 0 ? Math.floor((status.downloadedBytes / status.totalBytes) * 100) : 0;
  const remaining = formatBytes(Math.max(status.totalBytes - status.downloadedBytes, 0));

  return (
    <div className="flex flex-col gap-3 py-2">
      <div className="flex items-start gap-3">
        <Cpu size={18} className="mt-0.5 shrink-0 text-ink" aria-hidden />
        <div className="min-w-0 flex-1 text-sm">
          {status.state === "ready" ? (
            <>
              <p>
                <span className="font-medium">{status.modelName}</span> is ready and runs on this computer.
              </p>
              {
                <p className="text-xs text-ink-muted">
                  {status.engineRunning
                    ? "Working now."
                    : "Starts when there's something to file and stops after 2 idle minutes to free memory."}
                </p>
              }
            </>
          ) : status.state === "downloading" ? (
            <p>
              {status.visionDownloading ? "Downloading the image model" : `Downloading ${status.modelName}`}… {pct}%{" "}
              <span className="text-ink-muted">({remaining} left)</span>
            </p>
          ) : (
            <>
              <p>
                {status.state === "paused"
                  ? `Download paused at ${pct}%. It picks up where it left off.`
                  : `The AI that files your screenshots runs entirely on this computer. It needs a one-time download of about ${formatBytes(status.totalBytes)}.`}
              </p>
              {status.state === "notInstalled" && (
                <p className="text-xs text-ink-muted">You can keep capturing while it downloads; screenshots are filed once it's ready.</p>
              )}
            </>
          )}
          {status.error && (
            <p role="alert" className="mt-1 text-xs text-signature-coral dark:text-signature-peach">
              {status.error}
            </p>
          )}
        </div>
      </div>

      {(status.state === "downloading" || status.state === "paused" || (status.state === "error" && status.downloadedBytes > 0)) && (
        <div
          className="h-1.5 overflow-hidden rounded-full bg-line"
          role="progressbar"
          aria-valuenow={pct}
          aria-valuemin={0}
          aria-valuemax={100}
        >
          <div className="h-full rounded-full bg-primary transition-[width] duration-300" style={{ width: `${pct}%` }} />
        </div>
      )}

      <div className="flex flex-wrap gap-2">
        {status.state === "downloading" ? (
          <Button onClick={() => void ipc.aiPauseDownload().catch(fail)}>
            <Pause size={14} /> Pause
          </Button>
        ) : status.state !== "ready" ? (
          <Button variant="primary" onClick={() => void ipc.aiStartSetup().catch(fail)}>
            <Download size={14} />
            {status.state === "notInstalled" ? `Set up AI (${formatBytes(status.totalBytes)})` : "Resume download"}
          </Button>
        ) : null}
        {showManage && status.state === "ready" && !status.visionInstalled && !status.visionDownloading && (
          <Button onClick={() => void ipc.aiStartVisionSetup().catch(fail)}>
            <Download size={14} /> Add image understanding ({formatBytes(status.visionBytes)})
          </Button>
        )}
        {showManage && (status.state === "ready" || status.state === "paused" || status.state === "error") && (
          <Button
            variant="ghost"
            onClick={() => void ipc.aiDelete().then(() => toast({ message: "AI files deleted. Screenshots are kept." }), fail)}
          >
            <Trash2 size={14} /> Delete AI files
          </Button>
        )}
      </div>
    </div>
  );
}
