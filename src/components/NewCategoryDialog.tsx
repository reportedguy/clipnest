import { useCallback, useState } from "react";
import { Check } from "lucide-react";
import { ipc } from "../lib/ipc";
import { PALETTE } from "../lib/palette";
import { Button, Modal, inputClass } from "./ui";
import { useToast } from "./Toast";

/**
 * After a category is created, offers to have the AI look at existing screenshots and add
 * the new category where it fits (it only adds; nothing is moved or removed).
 */
export function useOfferRecheck() {
  const toast = useToast();
  return useCallback(
    (name: string) =>
      toast({
        message: `Added “${name}”. Check your existing screenshots for it?`,
        action: {
          label: "Check",
          run: () =>
            void ipc.recheckCategories().then(
              (n) =>
                toast({
                  message: n ? `Checking ${n} screenshot${n === 1 ? "" : "s"} in the background` : "No screenshots to check yet",
                }),
              (e) => toast({ message: String(e), tone: "error" }),
            ),
        },
      }),
    [toast],
  );
}

/** Name, color and an optional hint that helps the AI recognise what belongs here. */
export function NewCategoryDialog({
  open,
  initialName = "",
  defaultColor,
  onClose,
  onCreated,
}: {
  open: boolean;
  initialName?: string;
  defaultColor: string;
  onClose: () => void;
  onCreated: (id: number, name: string) => void;
}) {
  const [name, setName] = useState(initialName);
  const [hint, setHint] = useState("");
  const [color, setColor] = useState(defaultColor);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  async function create() {
    setSaving(true);
    setError(null);
    try {
      const id = await ipc.createCategory({ name, hint, color });
      onCreated(id, name.trim());
      onClose();
    } catch (e) {
      setError(String(e));
      setSaving(false);
    }
  }

  return (
    <Modal open={open} onOpenChange={(o) => !o && onClose()} title="New category">
      <form
        className="flex flex-col gap-4"
        onSubmit={(e) => {
          e.preventDefault();
          if (name.trim()) void create();
        }}
      >
        <label className="flex flex-col gap-1.5 text-sm text-ink">
          Name
          <input
            className={inputClass}
            value={name}
            maxLength={40}
            autoFocus
            placeholder="e.g. Portfolio redesign"
            onChange={(e) => setName(e.target.value)}
          />
        </label>
        <label className="flex flex-col gap-1.5 text-sm text-ink">
          <span>
            What belongs here? <span className="text-ink-muted">(optional, helps the AI)</span>
          </span>
          <input
            className={inputClass}
            value={hint}
            maxLength={160}
            placeholder="e.g. screens, fonts and references for my new portfolio site"
            onChange={(e) => setHint(e.target.value)}
          />
        </label>
        <div className="flex flex-col gap-1.5 text-sm text-ink">
          Color
          <div role="radiogroup" aria-label="Color" className="flex flex-wrap gap-2">
            {PALETTE.map((c) => (
              <button
                key={c}
                type="button"
                role="radio"
                aria-checked={color === c}
                aria-label={c}
                onClick={() => setColor(c)}
                className="flex h-8 w-8 items-center justify-center rounded-full border border-line"
                style={{ backgroundColor: c }}
              >
                {color === c && (
                  <Check size={15} className={["#f5e9d4", "#f4d35e", "#a8d8c4", "#fcab79"].includes(c) ? "text-[#181d26]" : "text-white"} />
                )}
              </button>
            ))}
          </div>
        </div>
        {error && (
          <p role="alert" className="text-sm text-signature-coral dark:text-signature-peach">
            {error}
          </p>
        )}
        <div className="flex justify-end gap-2 pt-1">
          <Button onClick={onClose}>Cancel</Button>
          <Button type="submit" variant="primary" disabled={!name.trim() || saving}>
            Create category
          </Button>
        </div>
      </form>
    </Modal>
  );
}
