import type { ReactNode } from "react";

interface Props {
  title: string;
  description?: string;
  children: ReactNode;
}

/// 设置页的区块容器：标题 + 描述 + 内容卡片
export default function Section({ title, description, children }: Props) {
  return (
    <section className="mb-8">
      <div className="px-6 mb-3">
        <h2 className="text-[13px] font-semibold text-stone-900 tracking-tight">
          {title}
        </h2>
        {description && (
          <p className="mt-0.5 text-[11px] text-stone-500">{description}</p>
        )}
      </div>
      <div className="mx-6 bg-white border border-stone-200 rounded-lg divide-y divide-stone-100">
        {children}
      </div>
    </section>
  );
}

/// Section 内的一行（左标签 + 右控件）
export function Row({
  label,
  hint,
  children,
  danger,
}: {
  label: string;
  hint?: string;
  children: ReactNode;
  danger?: boolean;
}) {
  return (
    <div className="flex items-center justify-between px-4 py-3 gap-4">
      <div className="flex-1 min-w-0">
        <div
          className={`text-[13px] ${danger ? "text-red-700" : "text-stone-800"}`}
        >
          {label}
        </div>
        {hint && <div className="mt-0.5 text-[11px] text-stone-500">{hint}</div>}
      </div>
      <div className="flex-shrink-0">{children}</div>
    </div>
  );
}
