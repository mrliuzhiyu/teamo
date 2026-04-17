import { useEffect, useRef, useState } from "react";
import { getAllWebviewWindows, getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import CloudCtaButton from "../lib/CloudCtaButton";

interface Props {
  isPaused: boolean;
  onPause: (minutes: number | null) => void;
  onResume: () => void;
}

const PAUSE_OPTIONS: Array<{ label: string; minutes: number | null }> = [
  { label: "5 分钟", minutes: 5 },
  { label: "1 小时", minutes: 60 },
  { label: "直到我恢复", minutes: null },
];

export default function ActionBar({ isPaused, onPause, onResume }: Props) {
  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!menuOpen) return;
    const onDocClick = (e: MouseEvent) => {
      if (!menuRef.current?.contains(e.target as Node)) {
        setMenuOpen(false);
      }
    };
    document.addEventListener("mousedown", onDocClick);
    return () => document.removeEventListener("mousedown", onDocClick);
  }, [menuOpen]);

  const handlePauseClick = () => {
    if (isPaused) {
      onResume();
    } else {
      setMenuOpen((v) => !v);
    }
  };

  const pick = (minutes: number | null) => {
    onPause(minutes);
    setMenuOpen(false);
  };

  const openSettings = async () => {
    try {
      const wins = await getAllWebviewWindows();
      const main = wins.find((w) => w.label === "main");
      if (main) {
        await main.show();
        await main.unminimize().catch(() => undefined);
        await main.setFocus();
      }
      await getCurrentWebviewWindow().hide();
    } catch (e) {
      console.error("open settings failed", e);
    }
  };

  return (
    <div className="relative px-2 py-1.5 border-t border-stone-200 bg-stone-50 flex items-center gap-1 text-[11px]">
      <div ref={menuRef} className="relative">
        <button
          onClick={handlePauseClick}
          className="px-2 py-1 rounded hover:bg-stone-200 text-stone-600 transition-colors"
          title={isPaused ? "恢复记录" : "暂停记录"}
        >
          {isPaused ? "▶ 继续记录" : "⏸ 暂停记录"}
        </button>
        {menuOpen && (
          <div className="absolute bottom-full left-0 mb-1 w-36 bg-white border border-stone-200 rounded shadow-lg py-1 z-10">
            {PAUSE_OPTIONS.map((opt) => (
              <button
                key={opt.label}
                onClick={() => pick(opt.minutes)}
                className="w-full text-left px-3 py-1.5 hover:bg-stone-100 text-stone-700"
              >
                {opt.label}
              </button>
            ))}
          </div>
        )}
      </div>
      <CloudCtaButton variant="compact" />
      <button
        onClick={openSettings}
        className="px-2 py-1 rounded hover:bg-stone-200 text-stone-600 transition-colors ml-auto"
        title="打开设置"
      >
        ⚙️ 设置
      </button>
    </div>
  );
}
