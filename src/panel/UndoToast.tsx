import { useEffect, useState } from "react";

interface Props {
  onUndo: () => void;
  pendingId: string;
  durationMs: number;
}

export default function UndoToast({ onUndo, pendingId, durationMs }: Props) {
  const [remaining, setRemaining] = useState(durationMs);

  useEffect(() => {
    setRemaining(durationMs);
    const startedAt = Date.now();
    const timer = window.setInterval(() => {
      const left = Math.max(0, durationMs - (Date.now() - startedAt));
      setRemaining(left);
      if (left <= 0) window.clearInterval(timer);
    }, 100);
    return () => window.clearInterval(timer);
  }, [durationMs, pendingId]);

  const secLeft = Math.ceil(remaining / 1000);

  return (
    <div className="absolute left-1/2 -translate-x-1/2 bottom-10 z-20 flex items-center gap-3 px-3 py-2 bg-stone-900 text-white text-xs rounded-md shadow-lg">
      <span>已忘记（{secLeft}s）</span>
      <button
        onClick={onUndo}
        className="text-brand hover:text-brand-500 font-medium"
      >
        撤销
      </button>
    </div>
  );
}
