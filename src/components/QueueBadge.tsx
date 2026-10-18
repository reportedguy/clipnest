import { useEffect, useState } from "react";
import { Loader2 } from "lucide-react";
import { ipc, type QueueStatus } from "../lib/ipc";
import { QUEUE_STATUS, useTauriEvent } from "../lib/events";

/** Subtle header indicator: "Processing 3 screenshots", or why work is waiting. */
export function QueueBadge() {
  const [status, setStatus] = useState<QueueStatus | null>(null);
  useEffect(() => {
    ipc.queueStatus().then(setStatus, () => {});
  }, []);
  useTauriEvent<QueueStatus>(QUEUE_STATUS, setStatus);

  if (!status || status.pending === 0) return null;
  const n = status.pending;
  const noun = n === 1 ? "screenshot" : "screenshots";
  const waitingOnModel = !status.aiReady && status.waitingForAi === n;
  const waitingOnVision = status.aiReady && status.waitingForVision === n;

  const [text, busy] = status.batterySaver
    ? [`AI paused for battery saver · ${n} waiting`, false]
    : status.paused
      ? [`AI paused · ${n} waiting`, false]
      : waitingOnModel
        ? [`${n} ${noun} waiting for the AI model`, false]
        : waitingOnVision
          ? [`${n} ${noun} waiting for the image model`, false]
          : [`Processing ${n} ${noun}`, true];

  return (
    <span
      role="status"
      className="inline-flex items-center gap-1.5 rounded-[var(--radius-input)] bg-surface px-3 py-1.5 text-[13px] text-body"
    >
      {busy && <Loader2 size={12} className="animate-spin" aria-hidden />}
      {text}
    </span>
  );
}
