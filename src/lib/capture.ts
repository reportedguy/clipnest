// Used by the capture popup. Rust injects `window.__RS_CAPTURE__` before the page loads
// (see capture/session.rs).
import { convertFileSrc, invoke } from "@tauri-apps/api/core";

export interface CaptureInit {
  session: number;
}

declare global {
  interface Window {
    __RS_CAPTURE__?: CaptureInit;
  }
}

export function captureInit(): CaptureInit {
  const init = window.__RS_CAPTURE__;
  if (!init) throw new Error("capture page opened without session info");
  return init;
}

/** The selection's thumbnail, served from memory by the Rust `rsmedia` protocol. */
export function thumbUrl(session: number): string {
  return convertFileSrc(`thumb-${session}`, "rsmedia");
}

/** Emitted to the popup when the user has finished selecting a region. */
export const SELECTED_EVENT = "capture-selected";

export type Mode = "text" | "image";

export const captureIpc = {
  /** Returns true if the region was already selected when the page attached. */
  popupAttach: (session: number) => invoke<boolean>("popup_attach", { session }),
  popupReady: (session: number) => invoke<void>("popup_ready", { session }),
  save: (session: number, request: { note: string; mode: Mode; categoryIds: number[] }) =>
    invoke<number>("save_capture", { session, request }),
  cancel: (session: number) => invoke<void>("cancel_capture", { session }),
  closePopup: (session: number) => invoke<void>("close_popup", { session }),
};
