import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

interface ConfirmOptions {
  title: string;
  body?: string;
  confirmText?: string;
  cancelText?: string;
  /** 破坏性操作，确认按钮用红色 */
  danger?: boolean;
}

type Resolver = (ok: boolean) => void;

interface Pending extends ConfirmOptions {
  resolve: Resolver;
}

const ConfirmContext = createContext<((opts: ConfirmOptions) => Promise<boolean>) | null>(null);

/// 自定义 Confirm dialog（替代浏览器原生 window.confirm 丑样式）。
/// 使用：const confirm = useConfirm(); const ok = await confirm({...});
export function ConfirmProvider({ children }: { children: ReactNode }) {
  const [pending, setPending] = useState<Pending | null>(null);

  const confirm = useCallback((opts: ConfirmOptions) => {
    return new Promise<boolean>((resolve) => {
      setPending({ ...opts, resolve });
    });
  }, []);

  const close = useCallback(
    (ok: boolean) => {
      if (!pending) return;
      pending.resolve(ok);
      setPending(null);
    },
    [pending],
  );

  // Esc 取消，Enter 确认
  useEffect(() => {
    if (!pending) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close(false);
      if (e.key === "Enter") close(true);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [pending, close]);

  return (
    <ConfirmContext.Provider value={confirm}>
      {children}
      {pending &&
        createPortal(
          <div
            role="dialog"
            aria-modal="true"
            data-teamo-dialog="open"
            className="fixed inset-0 z-[90] flex items-center justify-center bg-black/30 backdrop-blur-[1px]"
            onMouseDown={(e) => {
              if (e.target === e.currentTarget) close(false);
            }}
          >
            <div className="w-[340px] max-w-[90vw] bg-white rounded-lg shadow-2xl border border-stone-200 overflow-hidden">
              <div className="px-5 pt-4 pb-2">
                <div className="text-[14px] font-semibold text-stone-900">{pending.title}</div>
                {pending.body && (
                  <div className="mt-2 text-[12px] text-stone-600 whitespace-pre-line leading-relaxed">
                    {pending.body}
                  </div>
                )}
              </div>
              <div className="px-5 py-3 bg-stone-50 border-t border-stone-100 flex items-center justify-end gap-2">
                <button
                  onClick={() => close(false)}
                  className="px-3 py-1 text-[12px] bg-white border border-stone-300 text-stone-700 rounded hover:bg-stone-100"
                  autoFocus
                >
                  {pending.cancelText ?? "取消"}
                </button>
                <button
                  onClick={() => close(true)}
                  className={`px-3 py-1 text-[12px] rounded ${
                    pending.danger
                      ? "bg-red-600 text-white hover:bg-red-700"
                      : "bg-stone-900 text-white hover:bg-stone-800"
                  }`}
                >
                  {pending.confirmText ?? "确认"}
                </button>
              </div>
            </div>
          </div>,
          document.body,
        )}
    </ConfirmContext.Provider>
  );
}

export function useConfirm() {
  const confirm = useContext(ConfirmContext);
  if (!confirm) {
    // 降级到浏览器 confirm，不影响功能
    return async (opts: ConfirmOptions) => {
      const text = opts.body ? `${opts.title}\n\n${opts.body}` : opts.title;
      return Promise.resolve(window.confirm(text));
    };
  }
  return confirm;
}
