import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from "react";

interface ToastData {
  id: number;
  message: string;
  action?: { label: string; run: () => void };
  tone?: "info" | "error";
}

type Show = (t: Omit<ToastData, "id">) => void;

const ToastContext = createContext<Show>(() => {});

/** How long a toast (and its Undo) stays up. */
const TOAST_MS = 6000;

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toast, setToast] = useState<ToastData | null>(null);
  const timer = useRef<number | undefined>(undefined);
  const nextId = useRef(1);

  const show = useCallback<Show>((t) => {
    window.clearTimeout(timer.current);
    const id = nextId.current++;
    setToast({ ...t, id });
    timer.current = window.setTimeout(() => setToast((cur) => (cur?.id === id ? null : cur)), TOAST_MS);
  }, []);

  useEffect(() => () => window.clearTimeout(timer.current), []);

  return (
    <ToastContext.Provider value={show}>
      {children}
      <div aria-live="polite" className="pointer-events-none fixed inset-x-0 bottom-5 z-50 flex justify-center px-4">
        {toast && (
          <div
            role={toast.tone === "error" ? "alert" : "status"}
            className={`pointer-events-auto flex items-center gap-4 rounded-[var(--radius-action)] px-5 py-3 text-sm ${
              toast.tone === "error" ? "bg-signature-coral text-white" : "bg-primary text-on-primary"
            }`}
          >
            <span>{toast.message}</span>
            {toast.action && (
              <button
                type="button"
                className="font-medium underline underline-offset-2"
                onClick={() => {
                  toast.action!.run();
                  setToast(null);
                }}
              >
                {toast.action.label}
              </button>
            )}
          </div>
        )}
      </div>
    </ToastContext.Provider>
  );
}

export function useToast(): Show {
  return useContext(ToastContext);
}
