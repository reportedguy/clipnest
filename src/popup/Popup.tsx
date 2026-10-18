import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { Popover } from "radix-ui";
import { ArrowUp, Check, ChevronDown } from "lucide-react";
import { CategoryPicker } from "../components/CategoryPicker";
import { nextColor } from "../lib/palette";
import { captureInit, captureIpc, SELECTED_EVENT, thumbUrl, type Mode } from "../lib/capture";
import { ipc, type AiStatus, type Category } from "../lib/ipc";
import { formatBytes } from "../lib/format";
import { setTheme } from "../lib/theme";

/** How long "Saved" stays visible before the popup closes (spec: under 800 ms). */
const SAVED_MS = 550;

/**
 * Note popup shown after a region is selected. Enter saves, Shift+Enter adds a line,
 * Esc discards the capture. Focus lands in the note box so the user can type at once.
 */
export function Popup() {
  const { session } = captureInit();
  const [note, setNote] = useState("");
  const [mode, setMode] = useState<Mode>("text");
  // Empty = Auto: the AI picks. Picking here locks the choice.
  const [categoryIds, setCategoryIds] = useState<number[]>([]);
  const [pickerOpen, setPickerOpen] = useState(false);
  const [categories, setCategories] = useState<Category[]>([]);
  const [state, setState] = useState<"editing" | "saving" | "saved">("editing");
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState(false);
  const [ai, setAi] = useState<AiStatus | null>(null);
  const noteRef = useRef<HTMLTextAreaElement>(null);
  const readySent = useRef(false);

  useEffect(() => {
    ipc
      .getSettings()
      .then((s) => setTheme(s.theme))
      .catch(() => {});
    ipc
      .listCategories()
      .then(setCategories)
      .catch(() => {});
    ipc
      .aiStatus()
      .then(setAi)
      .catch(() => {});
    const refocus = () => noteRef.current?.focus();
    window.addEventListener("focus", refocus);

    // This window is created (hidden) when the hotkey fires, so the user is usually still
    // dragging. Listen first, then ask, so the selection can't slip between the two.
    const off = getCurrentWebviewWindow().listen(SELECTED_EVENT, () => setSelected(true));
    void off
      .then(() => captureIpc.popupAttach(session))
      .then(
        (done) => done && setSelected(true),
        () => {},
      );
    return () => {
      window.removeEventListener("focus", refocus);
      void off.then((f) => f());
    };
  }, [session]);

  function ready() {
    if (readySent.current) return;
    readySent.current = true;
    requestAnimationFrame(() =>
      captureIpc.popupReady(session).then(
        () => noteRef.current?.focus(),
        () => {},
      ),
    );
  }

  async function save() {
    if (state !== "editing") return;
    setState("saving");
    setError(null);
    try {
      await captureIpc.save(session, { note, mode, categoryIds });
      setState("saved");
      setTimeout(() => void captureIpc.closePopup(session), SAVED_MS);
    } catch (e) {
      // Nothing is lost: the capture is still pending, so the user can retry or discard.
      setState("editing");
      setError(String(e));
    }
  }

  function onKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      if (pickerOpen) return; // Esc closes the open category picker, not the capture
      e.preventDefault();
      void captureIpc.cancel(session);
    } else if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing && e.target === noteRef.current) {
      e.preventDefault();
      void save();
    }
  }

  return (
    <div className="h-screen p-3">
      <div
        onKeyDown={onKeyDown}
        className="flex h-full flex-col gap-3 overflow-hidden rounded-[var(--radius-action)] border border-line bg-canvas p-4 text-ink shadow-[0_2px_10px_rgba(24,29,38,0.10)]"
      >
        <div className="flex h-[104px] shrink-0 items-center justify-center overflow-hidden rounded-[var(--radius-card)] bg-surface">
          {selected && (
            <img
              src={thumbUrl(session)}
              onLoad={ready}
              onError={ready}
              alt="Captured region"
              className="max-h-full max-w-full object-contain"
              draggable={false}
            />
          )}
        </div>

        <div className="relative flex min-h-0 flex-1">
          <textarea
            ref={noteRef}
            autoFocus
            rows={3}
            value={note}
            onChange={(e) => setNote(e.target.value)}
            disabled={state !== "editing"}
            placeholder="Add a note (optional)"
            aria-label="Note"
            className="min-h-0 flex-1 resize-none rounded-[var(--radius-input)] border border-line bg-canvas py-2.5 pr-12 pl-3 text-sm leading-[1.4] text-ink placeholder:text-ink-muted focus:border-focus focus:outline-none"
          />
          <button
            type="button"
            onClick={() => void save()}
            disabled={state !== "editing"}
            aria-label="Save screenshot"
            title="Save (Enter)"
            className="absolute right-2 bottom-2 inline-flex h-8 w-8 items-center justify-center rounded-full bg-brand text-white transition-colors duration-100 active:bg-brand-active disabled:opacity-40"
          >
            {state === "saved" ? <Check size={16} strokeWidth={2.5} /> : <ArrowUp size={16} strokeWidth={2.5} />}
          </button>
        </div>

        <div className="flex items-center gap-2">
          <div
            role="radiogroup"
            aria-label="Processing mode"
            className="flex rounded-[var(--radius-input)] border border-line p-0.5 text-[13px]"
          >
            {(["text", "image"] as const).map((m) => (
              <button
                key={m}
                type="button"
                role="radio"
                aria-checked={mode === m}
                title={m === "text" ? "Read the text (fast)" : "Look at the picture: charts, designs, palettes (slower)"}
                onClick={() => setMode(m)}
                className={`rounded-[var(--radius-input)] px-2.5 py-1 capitalize transition-colors duration-100 ${
                  mode === m ? "bg-primary text-on-primary" : "text-ink-muted"
                }`}
              >
                {m}
              </button>
            ))}
          </div>

          <Popover.Root open={pickerOpen} onOpenChange={setPickerOpen}>
            <Popover.Trigger
              aria-label="Categories"
              className="flex min-w-0 flex-1 items-center gap-1.5 truncate rounded-[var(--radius-input)] border border-line bg-canvas px-2.5 py-1.5 text-left text-[13px] text-ink focus:border-focus focus:outline-none"
            >
              {categoryIds.length === 0 ? (
                <span className="text-ink-muted">Auto (AI picks)</span>
              ) : (
                categories
                  .filter((c) => categoryIds.includes(c.id))
                  .map((c) => (
                    <span key={c.id} className="inline-flex shrink-0 items-center gap-1">
                      <span className="color-dot h-2 w-2 shrink-0 rounded-full" style={{ backgroundColor: c.color }} aria-hidden />
                      {c.name}
                    </span>
                  ))
              )}
              <ChevronDown size={13} className="ml-auto shrink-0 text-ink-muted" aria-hidden />
            </Popover.Trigger>
            <Popover.Portal>
              <Popover.Content
                side="top"
                align="end"
                sideOffset={6}
                className="z-50 w-64 rounded-[var(--radius-card)] border border-line bg-surface-raised p-2 shadow-[0_8px_24px_rgba(24,29,38,0.12)]"
              >
                <CategoryPicker
                  compact
                  categories={categories}
                  selected={categoryIds}
                  onChange={setCategoryIds}
                  onCreate={async (name) => {
                    const id = await ipc.createCategory({ name, color: nextColor(categories.map((c) => c.color)) });
                    setCategories(await ipc.listCategories());
                    return id;
                  }}
                />
              </Popover.Content>
            </Popover.Portal>
          </Popover.Root>
        </div>

        <div className="flex min-h-0 items-center text-xs text-ink-muted empty:hidden" aria-live="polite">
          {/* One message at a time: error > saved > image-model note > keyboard hint. */}
          {error ? (
            <span className="truncate text-signature-coral dark:text-signature-peach" role="alert">
              {error}
            </span>
          ) : state === "saved" ? (
            <span className="font-medium text-success dark:text-signature-mint">✓ Saved</span>
          ) : mode === "image" && ai?.state === "ready" && !ai.visionInstalled ? (
            <span className="truncate">
              {ai.visionDownloading ? (
                "Image model downloading. This will be filed when it's ready."
              ) : (
                <>
                  Image mode needs a one-time {formatBytes(ai.visionBytes)} add-on.{" "}
                  <button
                    type="button"
                    className="font-medium text-link"
                    onClick={() =>
                      void ipc.aiStartVisionSetup().then(
                        () => setAi({ ...ai, visionDownloading: true }),
                        () => {},
                      )
                    }
                  >
                    Download
                  </button>
                </>
              )}
            </span>
          ) : null}
        </div>
      </div>
    </div>
  );
}
