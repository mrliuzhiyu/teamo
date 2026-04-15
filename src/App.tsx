import { useEffect, useState } from "react";

export default function App() {
  const [version, setVersion] = useState<string>("loading…");

  useEffect(() => {
    // Tauri 2.x getVersion 异步示例（脚手架阶段仅作占位）
    import("@tauri-apps/api/app")
      .then((api) => api.getVersion())
      .then((v) => setVersion(v))
      .catch(() => setVersion("dev"));
  }, []);

  return (
    <main className="min-h-screen flex items-center justify-center px-6">
      <div className="text-center max-w-lg">
        <div className="mx-auto mb-6 w-16 h-16 rounded-2xl bg-white border border-stone-200 shadow-sm flex items-center justify-center">
          <span className="text-3xl font-semibold text-stone-900">T</span>
          <span className="absolute -mt-6 ml-7 w-2.5 h-2.5 rounded-full bg-brand" />
        </div>
        <h1 className="text-2xl font-semibold tracking-tight text-stone-900">
          Teamo
        </h1>
        <p className="mt-2 text-stone-600">你的人生记录 Agent</p>
        <p className="mt-8 text-xs text-stone-400">
          v{version} · pre-alpha · 脚手架阶段
        </p>
      </div>
    </main>
  );
}
