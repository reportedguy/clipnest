import { useEffect, useLayoutEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";

/** Fired by Rust whenever screenshots or categories change (saves, moves, AI results). */
export const LIBRARY_CHANGED = "library-changed";
export const SETTINGS_CHANGED = "settings-changed";
export const QUEUE_STATUS = "queue-status";
export const AI_STATUS = "ai-status";

/** Subscribes to a Tauri event for the component's lifetime, always calling the latest handler. */
export function useTauriEvent<T>(event: string, handler: (payload: T) => void) {
  const latest = useRef(handler);
  useLayoutEffect(() => {
    latest.current = handler;
  });
  useEffect(() => {
    const off = listen<T>(event, (e) => latest.current(e.payload));
    return () => void off.then((f) => f());
  }, [event]);
}
