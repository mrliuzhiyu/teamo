import { createContext, useCallback, useContext, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

export type ToastTone = "success" | "error" | "info";

interface Toast {
  id: string;
  tone: ToastTone;
  text: string;
}

const ToastContext = createContext<((tone: ToastTone, text: string) => void) | null>(null);

const toneClass: Record<ToastTone, string> = {
  success: "bg-stone-900 text-white border-stone-800",
  error: "bg-red-600 text-white border-red-700",
  info: "bg-stone-800 text-white border-stone-700",
};

const toneIcon: Record<ToastTone, string> = {
  success: "✓",
  error: "!",
  info: "·",
};

/// Toast 容器 + push API。包在应用根部，子树任何位置 useToast 即可触发。
/// 自动 2.5s 淡出，最多同时显示 5 条（超出则替换最老的）
export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);

  const push = useCallback((tone: ToastTone, text: string) => {
    const id = `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
    setToasts((prev) => {
      const next = [...prev, { id, tone, text }];
      return next.length > 5 ? next.slice(next.length - 5) : next;
    });
    window.setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, 2500);
  }, []);

  return (
    <ToastContext.Provider value={push}>
      {children}
      {createPortal(
        <div className="fixed bottom-4 right-4 z-[100] flex flex-col gap-2 items-end pointer-events-none">
          {toasts.map((t) => (
            <div
              key={t.id}
              className={`px-3 py-2 rounded-md shadow-lg text-[12px] border flex items-center gap-2 animate-[slide-in_150ms_ease-out] ${toneClass[t.tone]}`}
            >
              <span className="text-[14px] leading-none">{toneIcon[t.tone]}</span>
              <span>{t.text}</span>
            </div>
          ))}
        </div>,
        document.body,
      )}
    </ToastContext.Provider>
  );
}

export function useToast() {
  const push = useContext(ToastContext);
  if (!push) {
    // Provider 未包裹时降级打 console，避免 crash
    return (tone: ToastTone, text: string) => {
      console.warn(`[toast ${tone}]`, text);
    };
  }
  return push;
}
