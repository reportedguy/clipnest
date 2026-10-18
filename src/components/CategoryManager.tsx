import { useState } from "react";
import { AlertDialog } from "radix-ui";
import { ArrowDown, ArrowUp, Plus, Trash2 } from "lucide-react";
import { ipc, type Category } from "../lib/ipc";
import { nextColor } from "../lib/palette";
import { Button, IconButton, inputClass } from "./ui";
import { useToast } from "./Toast";

/**
 * Add, rename, recolor, re-hint, reorder and delete categories. Hints are passed to the
 * AI ("fonts and typography I like") to file screenshots more accurately.
 */
export function CategoryManager({ categories, onCreated }: { categories: Category[]; onCreated?: (name: string) => void }) {
  const toast = useToast();
  const fail = (e: unknown) => toast({ message: String(e), tone: "error" });
  const user = categories.filter((c) => !c.isSystem);
  const system = categories.filter((c) => c.isSystem);

  async function move(index: number, by: -1 | 1) {
    const ids = user.map((c) => c.id);
    const j = index + by;
    if (j < 0 || j >= ids.length) return;
    [ids[index], ids[j]] = [ids[j], ids[index]];
    await ipc.reorderCategories(ids).catch(fail);
  }

  return (
    <div className="flex flex-col gap-2">
      <ul className="flex flex-col gap-2">
        {user.map((c, i) => (
          <CategoryRow
            key={`${c.id}:${c.name}:${c.hint}:${c.color}`}
            category={c}
            onError={fail}
            controls={
              <>
                <IconButton label={`Move ${c.name} up`} disabled={i === 0} onClick={() => void move(i, -1)}>
                  <ArrowUp size={15} />
                </IconButton>
                <IconButton label={`Move ${c.name} down`} disabled={i === user.length - 1} onClick={() => void move(i, 1)}>
                  <ArrowDown size={15} />
                </IconButton>
                <DeleteCategory category={c} onError={fail} />
              </>
            }
          />
        ))}
        {system.map((c) => (
          <CategoryRow key={`${c.id}:${c.hint}:${c.color}`} category={c} onError={fail} />
        ))}
      </ul>
      <AddCategory
        nextColor={nextColor(categories.map((c) => c.color))}
        onError={fail}
        onCreated={(name) => {
          // The parent offers to check existing screenshots; fall back to a plain confirmation.
          if (onCreated) onCreated(name);
          else toast({ message: `Added “${name}”` });
        }}
      />
    </div>
  );
}

function CategoryRow({
  category: c,
  controls,
  onError,
}: {
  category: Category;
  controls?: React.ReactNode;
  onError: (e: unknown) => void;
}) {
  const save = (input: Parameters<typeof ipc.updateCategory>[1]) => void ipc.updateCategory(c.id, input).catch(onError);

  return (
    <li className="flex items-center gap-2 rounded-[var(--radius-card)] border border-line bg-surface p-2">
      <input
        type="color"
        aria-label={`Color for ${c.name}`}
        defaultValue={c.color}
        onBlur={(e) => e.target.value !== c.color && save({ color: e.target.value })}
        className="h-7 w-7 shrink-0 cursor-pointer rounded-[var(--radius-input)] border-0 bg-transparent p-0"
      />
      <div className="grid min-w-0 flex-1 grid-cols-1 gap-1.5 sm:grid-cols-[minmax(8rem,14rem)_1fr]">
        <input
          aria-label="Category name"
          defaultValue={c.name}
          disabled={c.isSystem}
          title={c.isSystem ? "Miscellaneous is permanent and can't be renamed" : undefined}
          maxLength={40}
          onBlur={(e) => e.target.value.trim() && e.target.value.trim() !== c.name && save({ name: e.target.value })}
          onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
          className={`${inputClass} disabled:opacity-70`}
        />
        <input
          aria-label={`Hint for ${c.name}`}
          defaultValue={c.hint ?? ""}
          placeholder={c.isSystem ? "Used when nothing else fits" : "Hint for the AI, e.g. “fonts and typography I like”"}
          maxLength={160}
          onBlur={(e) => e.target.value.trim() !== (c.hint ?? "") && save({ hint: e.target.value })}
          onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
          className={inputClass}
        />
      </div>
      <div className="flex shrink-0 items-center">{controls}</div>
    </li>
  );
}

function DeleteCategory({ category, onError }: { category: Category; onError: (e: unknown) => void }) {
  return (
    <AlertDialog.Root>
      <AlertDialog.Trigger asChild>
        <IconButton label={`Delete ${category.name}`}>
          <Trash2 size={15} />
        </IconButton>
      </AlertDialog.Trigger>
      <AlertDialog.Portal>
        <AlertDialog.Overlay className="fixed inset-0 z-40 bg-[#181d26]/50" />
        <AlertDialog.Content className="fixed top-1/2 left-1/2 z-50 w-[calc(100vw-2rem)] max-w-sm -translate-x-1/2 -translate-y-1/2 rounded-[var(--radius-card)] border border-line bg-surface-raised p-5 text-ink shadow-xl">
          <AlertDialog.Title className="font-medium">Delete “{category.name}”?</AlertDialog.Title>
          <AlertDialog.Description className="mt-2 text-sm text-ink-muted">
            {category.itemCount === 0
              ? "This category is empty."
              : `It's removed from ${category.itemCount} screenshot${category.itemCount === 1 ? "" : "s"}. Screenshots aren't deleted; any left with no category go to Miscellaneous.`}
          </AlertDialog.Description>
          <div className="mt-5 flex justify-end gap-2">
            <AlertDialog.Cancel asChild>
              <Button>Cancel</Button>
            </AlertDialog.Cancel>
            <AlertDialog.Action asChild>
              <Button variant="danger" onClick={() => void ipc.deleteCategory(category.id).catch(onError)}>
                Delete category
              </Button>
            </AlertDialog.Action>
          </div>
        </AlertDialog.Content>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  );
}

function AddCategory({
  nextColor,
  onCreated,
  onError,
}: {
  nextColor: string;
  onCreated: (name: string) => void;
  onError: (e: unknown) => void;
}) {
  const [name, setName] = useState("");
  const [hint, setHint] = useState("");
  const [color, setColor] = useState(nextColor);

  async function add() {
    try {
      await ipc.createCategory({ name, hint, color });
      onCreated(name.trim());
      setName("");
      setHint("");
    } catch (e) {
      onError(e);
    }
  }

  return (
    <form
      className="flex items-center gap-2 rounded-[var(--radius-card)] border border-dashed border-line p-2"
      onSubmit={(e) => {
        e.preventDefault();
        if (name.trim()) void add();
      }}
    >
      <input
        type="color"
        aria-label="Color for the new category"
        value={color}
        onChange={(e) => setColor(e.target.value)}
        className="h-7 w-7 shrink-0 cursor-pointer rounded-[var(--radius-input)] border-0 bg-transparent p-0"
      />
      <div className="grid min-w-0 flex-1 grid-cols-1 gap-1.5 sm:grid-cols-[minmax(8rem,14rem)_1fr]">
        <input
          aria-label="New category name"
          placeholder="New category, e.g. Fonts"
          value={name}
          maxLength={40}
          onChange={(e) => setName(e.target.value)}
          className={inputClass}
        />
        <input
          aria-label="Hint for the new category"
          placeholder="Optional hint for the AI"
          value={hint}
          maxLength={160}
          onChange={(e) => setHint(e.target.value)}
          className={inputClass}
        />
      </div>
      <Button type="submit" variant="primary" disabled={!name.trim()}>
        <Plus size={15} /> Add
      </Button>
    </form>
  );
}
