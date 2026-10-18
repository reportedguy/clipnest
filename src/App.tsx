import { useCallback, useEffect, useState } from "react";
import { ArrowLeft } from "lucide-react";
import { formatHotkey, ipc, type AppInfo, type Card, type Category, type Settings } from "./lib/ipc";
import { LIBRARY_CHANGED, SETTINGS_CHANGED, useTauriEvent } from "./lib/events";
import { setTheme } from "./lib/theme";
import { Notice } from "./components/Notice";
import { IconButton } from "./components/ui";
import { ToastProvider } from "./components/Toast";
import { VisionPrompt } from "./components/VisionPrompt";
import { Lightbox } from "./components/CardGrid";
import { TitleBar } from "./components/TitleBar";
import { Feed } from "./views/Feed";
import { SettingsView } from "./views/SettingsView";
import { Onboarding } from "./views/Onboarding";

type View = { kind: "home" } | { kind: "settings" };

export default function App() {
  const [view, setView] = useState<View>({ kind: "home" });
  const [categories, setCategories] = useState<Category[]>([]);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [focusSearch, setFocusSearch] = useState(false);
  const [opened, setOpened] = useState<Card | null>(null);

  // Ctrl+K from any screen jumps home to the search box (on home, SearchBar handles it).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k" && view.kind !== "home") {
        e.preventDefault();
        setFocusSearch(true);
        setView({ kind: "home" });
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [view.kind]);

  // Each view starts at the top (the content scrolls under the fixed title bar).
  useEffect(() => {
    document.getElementById("scroll-root")?.scrollTo({ top: 0 });
  }, [view.kind]);

  const loadCategories = useCallback(() => ipc.listCategories().then(setCategories, (e) => setError(String(e))), []);
  const loadInfo = useCallback(() => ipc.appInfo().then(setInfo, (e) => setError(String(e))), []);

  useEffect(() => {
    void loadCategories();
    void loadInfo();
    ipc.getSettings().then(
      (s) => {
        setSettings(s);
        setTheme(s.theme);
      },
      (e) => setError(String(e)),
    );
  }, [loadCategories, loadInfo]);

  useTauriEvent<Settings>(SETTINGS_CHANGED, (s) => {
    setSettings(s);
    setTheme(s.theme);
    void loadInfo(); // hotkey status may have changed
  });
  useTauriEvent(LIBRARY_CHANGED, () => void loadCategories());

  if (!settings) {
    return error ? (
      <div className="p-6">
        <Notice tone="error">{error}</Notice>
      </div>
    ) : null;
  }

  const hotkey = info?.hotkey;
  const activeKey = hotkey && hotkey.kind !== "failed" ? hotkey.accelerator : null;

  return (
    <ToastProvider>
      <div className="flex h-full flex-col">
        <TitleBar
          theme={settings.theme}
          query={query}
          onQuery={(q) => {
            setQuery(q);
            if (view.kind !== "home") setView({ kind: "home" });
          }}
          focusSearch={focusSearch}
          onHome={() => setView({ kind: "home" })}
          onSettings={() => setView({ kind: "settings" })}
          onOpenCard={setOpened}
          onboarding={!settings.onboardingDone}
        />

        <div className="min-h-0 flex-1 overflow-y-auto" id="scroll-root">
          {!settings.onboardingDone ? (
            <Onboarding hotkey={activeKey} settings={settings} onDone={() => void ipc.updateSettings({ onboardingDone: true })} />
          ) : (
            <main className="mx-auto flex w-full max-w-[1400px] flex-col gap-6 px-8 pt-2 pb-24">
              {view.kind === "settings" && (
                <div className="mx-auto flex w-full max-w-3xl items-center gap-4 pt-6">
                  <IconButton shape="circle" label="Back to your library" onClick={() => setView({ kind: "home" })}>
                    <ArrowLeft size={17} />
                  </IconButton>
                  <div className="min-w-0">
                    <h1 className="type-display truncate text-[32px] leading-[1.2]">Settings</h1>
                    <p className="mt-1.5 truncate text-sm text-ink-muted">Everything runs on this computer.</p>
                  </div>
                </div>
              )}
              {error && <Notice tone="error">{error}</Notice>}
              {view.kind === "home" && info?.notice && <Notice tone="warn">{info.notice}</Notice>}
              {/* A fallback shortcut (the usual one is taken) still captures fine, so it's
                  only explained in Settings › Capture; home warns only when none works. */}
              {view.kind === "home" && hotkey?.kind === "failed" && (
                <Notice tone="warn">
                  Couldn't set up the capture shortcut {formatHotkey(hotkey.wanted)} ({hotkey.reason}). Capture from the tray icon, or
                  choose another shortcut in Settings.
                </Notice>
              )}
              {view.kind === "home" && info && info.ocrLanguages.length === 0 && (
                <Notice tone="warn">
                  Windows text recognition isn't installed, so screenshots will be filed using your notes and the images only. Add a
                  language with “Optical character recognition” in Windows Settings › Time &amp; language › Language &amp; region.
                </Notice>
              )}

              {view.kind === "home" && <VisionPrompt />}
              {view.kind === "home" && (
                <Feed text={query} activeHotkey={activeKey} showSummary={settings.showSummary} onOpenCard={setOpened} />
              )}
              {view.kind === "settings" && (
                <SettingsView settings={settings} info={info} categories={categories} onInfoStale={() => void loadInfo()} />
              )}
            </main>
          )}
        </div>
      </div>
      <img
        src="/founder.webp"
        alt=""
        draggable={false}
        className="pointer-events-none fixed bottom-3 left-3 z-20 h-9 w-9 object-contain select-none"
      />
      <Lightbox card={opened} onClose={() => setOpened(null)} />
    </ToastProvider>
  );
}
