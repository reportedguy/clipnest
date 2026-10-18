import { useEffect, useState } from "react";
import { ImageIcon } from "lucide-react";
import { ipc, type QueueStatus } from "../lib/ipc";
import { QUEUE_STATUS, useTauriEvent } from "../lib/events";
import { formatBytes } from "../lib/format";
import { useAiStatus } from "./AiSetup";
import { Button } from "./ui";
import { useToast } from "./Toast";

/**
 * One-time consent for the image model, shown only once a screenshot actually needs it
 * (Image mode, or auto-upgraded because it had almost no text). Spec 3.2: lazy download.
 */
export function VisionPrompt() {
  const ai = useAiStatus();
  const toast = useToast();
  const [queue, setQueue] = useState<QueueStatus | null>(null);
  useEffect(() => {
    ipc.queueStatus().then(setQueue, () => {});
  }, []);
  useTauriEvent<QueueStatus>(QUEUE_STATUS, setQueue);

  if (!ai || !queue || ai.state !== "ready" || ai.visionInstalled || ai.visionDownloading || queue.waitingForVision === 0) {
    return null;
  }
  const n = queue.waitingForVision;
  return (
    <div className="flex flex-wrap items-center gap-3 rounded-[var(--radius-card)] border border-line bg-surface-raised p-3 text-sm">
      <ImageIcon size={18} className="shrink-0 text-ink" aria-hidden />
      <span className="flex-1">
        {n === 1 ? "1 screenshot is" : `${n} screenshots are`} mostly visual (charts, designs, palettes). To understand pictures, the AI
        needs a one-time {formatBytes(ai.visionBytes)} add-on.
      </span>
      <Button variant="primary" onClick={() => void ipc.aiStartVisionSetup().catch((e) => toast({ message: String(e), tone: "error" }))}>
        Download
      </Button>
    </div>
  );
}
