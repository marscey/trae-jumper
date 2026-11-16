import { useState, useEffect, useRef } from "react";
import { relaunch } from "@tauri-apps/plugin-process";
import type { Update } from "@tauri-apps/plugin-updater";
import { checkForUpdate, downloadUpdate, installUpdate } from "../updater";
import * as api from "../api";
import type { AddToastFn } from "./useToast";

/** 自动更新状态（后台检查 + 静默下载 + 重启提示） */
export interface AutoUpdateState {
  status: "downloading" | "ready";
  version: string;
  percent: number;
  update: Update;
}

/**
 * 自动检查更新：启动 10s 后检查一次，之后每 6 小时检查一次；
 * 发现新版本后后台静默下载并安装，完成后提示重启生效（避免依赖用户手动点击）。
 */
export function useAutoUpdate(addToast: AddToastFn) {
  const [autoUpdate, setAutoUpdate] = useState<AutoUpdateState | null>(null);

  const checkedUpdateVersionsRef = useRef<Set<string>>(new Set());
  useEffect(() => {
    // 开发模式（tauri dev）下跳过，避免 dev 环境触发正式包安装
    if (import.meta.env.DEV || !api.hasTauri()) return;
    let cancelled = false;
    let running = false;

    const runCheck = async () => {
      if (running || cancelled) return;
      running = true;
      let attemptedVersion: string | null = null;
      try {
        const info = await checkForUpdate();
        if (!info) return;
        if (cancelled || checkedUpdateVersionsRef.current.has(info.version)) {
          info.update.close().catch(() => {});
          return;
        }
        attemptedVersion = info.version;
        checkedUpdateVersionsRef.current.add(info.version);
        setAutoUpdate({ status: "downloading", version: info.version, percent: 0, update: info.update });
        await downloadUpdate(info.update, (percent) => {
          if (cancelled) return;
          setAutoUpdate((prev) =>
            prev && prev.status === "downloading" ? { ...prev, percent } : prev
          );
        });
        if (cancelled) return;
        await installUpdate(info.update);
        if (cancelled) return;
        setAutoUpdate((prev) => (prev ? { ...prev, status: "ready", percent: 100 } : prev));
      } catch (e) {
        console.warn("[WARN] 自动检查/下载更新失败:", e);
        // 失败时移除标记，后续检查仍可重试
        if (attemptedVersion) checkedUpdateVersionsRef.current.delete(attemptedVersion);
        if (!cancelled) {
          setAutoUpdate((prev) => (prev && prev.status === "downloading" ? null : prev));
        }
      } finally {
        running = false;
      }
    };

    const firstTimer = setTimeout(runCheck, 10_000);
    const interval = setInterval(runCheck, 6 * 60 * 60 * 1000);
    return () => {
      cancelled = true;
      clearTimeout(firstTimer);
      clearInterval(interval);
    };
  }, []);

  // 安装更新后重启应用
  const handleRestartForUpdate = async () => {
    try {
      await relaunch();
    } catch (e) {
      addToast("error", `重启应用失败：${e instanceof Error ? e.message : String(e)}`);
    }
  };

  return { autoUpdate, setAutoUpdate, handleRestartForUpdate };
}
