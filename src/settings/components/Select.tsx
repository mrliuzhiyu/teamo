import { useEffect, useRef, useState } from "react";

interface Option<T extends string> {
  value: T;
  label: string;
}

interface Props<T extends string> {
  value: T;
  options: Option<T>[];
  onChange: (v: T) => void;
  disabled?: boolean;
}

/// 自定义下拉 — 替代浏览器原生 <select>（Windows 上是丑的 Win32 灰色下拉，和设计系统断裂）
export default function Select<T extends string>({ value, options, onChange, disabled }: Props<T>) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const current = options.find((o) => o.value === value) ?? options[0];

  useEffect(() => {
    if (!open) return;
    const onDocDown = (e: MouseEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDocDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDocDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <div ref={rootRef} className="relative inline-block">
      <button
        type="button"
        onClick={() => !disabled && setOpen((v) => !v)}
        disabled={disabled}
        className="flex items-center gap-1.5 text-[12px] px-2.5 py-1 bg-white border border-stone-200 rounded hover:border-stone-300 disabled:opacity-40 disabled:cursor-not-allowed focus:outline-none focus:border-stone-400 min-w-[110px] justify-between"
      >
        <span className="text-stone-700">{current?.label}</span>
        <svg width="10" height="10" viewBox="0 0 10 10" className="text-stone-400 flex-shrink-0">
          <path d="M2 4L5 7L8 4" stroke="currentColor" strokeWidth="1.3" fill="none" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      </button>
      {open && (
        <div className="absolute right-0 top-full mt-1 min-w-full bg-white border border-stone-200 rounded-md shadow-lg py-1 z-20">
          {options.map((opt) => (
            <button
              key={opt.value}
              type="button"
              onClick={() => {
                onChange(opt.value);
                setOpen(false);
              }}
              className={`w-full text-left text-[12px] px-3 py-1.5 hover:bg-stone-100 whitespace-nowrap ${
                opt.value === value ? "text-stone-900 font-medium" : "text-stone-700"
              }`}
            >
              {opt.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
