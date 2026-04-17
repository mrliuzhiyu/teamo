import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { DataInfo } from "./types";

/// 读写 SQLite settings 表的轻量 hook。
/// 所有 value 都是 string，业务层自己解析成 bool/int/enum。
export function useSetting(
  key: string,
  defaultValue: string,
): [string, (v: string) => Promise<void>] {
  const [value, setValue] = useState(defaultValue);

  useEffect(() => {
    invoke<string | null>("get_setting", { key })
      .then((v) => setValue(v ?? defaultValue))
      .catch(() => setValue(defaultValue));
  }, [key, defaultValue]);

  const update = useCallback(
    async (newValue: string) => {
      setValue(newValue);
      try {
        await invoke("set_setting", { key, value: newValue });
      } catch (e) {
        console.error(`set_setting ${key} failed`, e);
      }
    },
    [key],
  );

  return [value, update];
}

/// bool 语义包装：存 "1" / "0"
export function useBoolSetting(
  key: string,
  defaultValue: boolean,
): [boolean, (v: boolean) => Promise<void>] {
  const [raw, setRaw] = useSetting(key, defaultValue ? "1" : "0");
  const update = useCallback((v: boolean) => setRaw(v ? "1" : "0"), [setRaw]);
  return [raw === "1", update];
}

/// 查询本地数据信息
export function useDataInfo() {
  const [info, setInfo] = useState<DataInfo | null>(null);
  const [loading, setLoading] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const data = await invoke<DataInfo>("get_data_info");
      setInfo(data);
    } catch (e) {
      console.error("get_data_info failed", e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { info, loading, refresh };
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
}
