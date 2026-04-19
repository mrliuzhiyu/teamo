import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { DataInfo } from "./types";
import { useToast } from "../lib/toast";

/// 读写 SQLite settings 表的轻量 hook。
/// 所有 value 都是 string，业务层自己解析成 bool/int/enum。
/// 失败行为：
/// - 旧实现只 console.error，state 已乐观更新但 DB 未存 → UI/DB 不一致（silent bug）
/// - 新实现：失败回滚 state + toast 错误提示，保证可见 + 一致性
export function useSetting(
  key: string,
  defaultValue: string,
): [string, (v: string) => Promise<void>] {
  const [value, setValue] = useState(defaultValue);
  const toast = useToast();
  const valueRef = useRef(value);
  useEffect(() => {
    valueRef.current = value;
  }, [value]);

  useEffect(() => {
    invoke<string | null>("get_setting", { key })
      .then((v) => setValue(v ?? defaultValue))
      .catch(() => setValue(defaultValue));
  }, [key, defaultValue]);

  const update = useCallback(
    async (newValue: string) => {
      const prev = valueRef.current;
      setValue(newValue);
      try {
        await invoke("set_setting", { key, value: newValue });
      } catch (e) {
        console.error(`set_setting ${key} failed`, e);
        setValue(prev); // 回滚到旧值
        toast("error", `设置保存失败：${e}`);
      }
    },
    [key, toast],
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
