import { useState, useEffect, useRef, useCallback } from "react";
import { relaunch } from "@tauri-apps/plugin-process";
import type { Update } from "@tauri-apps/plugin-updater";
import { checkForUpdate, downloadUpdate, installUpdate, type UpdateInfo } from "../updater";
import * as api from "../api";
import type { AddToastFn } from "./useToast";

/** localStorage 键：跳过本次更新的版本号列表 */
const SKIPPED_VERSIONS_KEY = "traejumper_skipped_versions";

function loadSkippedVersions(): string[] {
  try {
    const raw = localStorage.getItem(SKIPPED_VERSIONS_KEY);
    if (!raw) return [];
    const arr = JSON.parse(raw);
    return Array.isArray(arr) ? arr.filter((v) => typeof v === "string") : [];
  } catch {
    return [];
  }
}

function saveSkippedVersions(versions: string[]) {
  try {
    localStorage.setItem(SKIPPED_VERSIONS_KEY, JSON.stringify(versions));
  } catch {
    /* ignore quota errors */
  }
}

/** 更新状态：有新版本可更 / 下载中 / 已就绪待重启 */
export type AutoUpdateStatus = "available" | "downloading" | "ready";

export interface AutoUpdateState {
  status: AutoUpdateStatus;
  version: string;
  date: string;
  notes: string;
  percent: number;
  update: Update;
}

/**
 * 自动更新钩子：
 *  - 启动 10s 后首次检查，之后每 6h 检查一次；
 *  - 发现新版本 → 置为 available，**不自动下载**，等用户在 UI 里点"立即更新"；
 *  - 用户选择"跳过本次" → 版本号记入 localStorage，后续不再提示；
 *  - 用户点"立即更新" → downloading → install → ready，弹窗提示重启。
 */
export function useAutoUpdate(addToast: AddToastFn) {
  const [autoUpdate, setAutoUpdate] = useState<AutoUpdateState | null>(null);

  /** 轮询中已尝试过的版本号（避免短时间重复触发同一版本的 check 流程） */
  const checkedVersionsRef = useRef<Set<string>>(new Set());
  /** 正在执行的 check 标记（防重入） */
  const runningRef = useRef(false);
  /** 最新的 Update 对象（手动 check 返回时临时持有） */
  const latestInfoRef = useRef<UpdateInfo | null>(null);

  const checkOnce = useCallback(async (): Promise<UpdateInfo | null> => {
    try {
      const info = await checkForUpdate();
      return info;
    } catch (e) {
      console.warn("[WARN] 检查更新失败:", e);
      return null;
    }
  }, []);

  /** 内部：把检查结果写入 state（会先过滤 skipped 版本） */
  const applyUpdateInfo = useCallback((info: UpdateInfo) => {
    const skipped = loadSkippedVersions();
    if (skipped.includes(info.version)) {
      // 用户之前跳过了这个版本 → 静默忽略，关闭 Update 句柄
      info.update.close().catch(() => {});
      return false;
    }
    latestInfoRef.current = info;
    setAutoUpdate({
      status: "available",
      version: info.version,
      date: info.date,
      notes: info.notes,
      percent: 0,
      update: info.update,
    });
    return true;
  }, []);

  /** 自动轮询（首次 + 定时） */
  useEffect(() => {
    if (import.meta.env.DEV || !api.hasTauri()) return;
    let cancelled = false;

    const runCheck = async () => {
      if (runningRef.current || cancelled) return;
      runningRef.current = true;
      try {
        const info = await checkOnce();
        if (!info) return;
        if (cancelled) return;
        if (checkedVersionsRef.current.has(info.version)) return;
        checkedVersionsRef.current.add(info.version);
        applyUpdateInfo(info);
      } finally {
        runningRef.current = false;
      }
    };

    const firstTimer = setTimeout(runCheck, 10_000);
    const interval = setInterval(runCheck, 6 * 60 * 60 * 1000);
    return () => {
      cancelled = true;
      clearTimeout(firstTimer);
      clearInterval(interval);
    };
  }, [checkOnce, applyUpdateInfo]);

  // ---------- 用户动作 ----------

  /** 跳过本次版本：记 localStorage + 关闭弹窗 + 关闭 Update 句柄 */
  const skipVersion = useCallback((version: string) => {
    const skipped = loadSkippedVersions();
    if (!skipped.includes(version)) {
      skipped.push(version);
      saveSkippedVersions(skipped);
    }
    // 关闭句柄，释放下载资源
    latestInfoRef.current?.update.close().catch(() => {});
    latestInfoRef.current = null;
    setAutoUpdate(null);
  }, []);

  /** 用户点"立即更新" → 下载 + 安装 */
  const startDownload = useCallback(async () => {
    setAutoUpdate((prev) =>
      prev?.status === "available" ? { ...prev, status: "downloading", percent: 0 } : prev
    );
    // 用闭包捕获当前 Update 对象（避免 React state 闭包陈旧）
    const updateObj = latestInfoRef.current?.update;
    if (!updateObj) {
      addToast("error", "更新对象已失效，请重试");
      setAutoUpdate(null);
      return;
    }
    try {
      await downloadUpdate(updateObj, (percent) => {
        setAutoUpdate((prev) =>
          prev && prev.status === "downloading" ? { ...prev, percent } : prev
        );
      });
      await installUpdate(updateObj);
      setAutoUpdate((prev) =>
        prev ? { ...prev, status: "ready", percent: 100 } : prev
      );
    } catch (e) {
      console.warn("[WARN] 下载/安装更新失败:", e);
      addToast(
        "error",
        `更新失败：${e instanceof Error ? e.message : String(e)}`
      );
      setAutoUpdate((prev) =>
        prev && prev.status === "downloading" ? null : prev
      );
    }
  }, [addToast]);

  /** 重启应用（ready 状态下用户点重启） */
  const handleRestartForUpdate = useCallback(async () => {
    try {
      await relaunch();
    } catch (e) {
      addToast("error", `重启应用失败：${e instanceof Error ? e.message : String(e)}`);
    }
  }, [addToast]);

  /** 手动触发检查（About 页面入口） */
  const handleManualCheck = useCallback(async (): Promise<UpdateInfo | null> => {
    const info = await checkOnce();
    if (!info) return null;
    applyUpdateInfo(info);
    return info;
  }, [checkOnce, applyUpdateInfo]);

  /** 关闭弹窗但不跳过版本（下次轮询还会再弹）——目前没有这个场景，预留 */
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  const dismissUpdate = useCallback(() => {
    setAutoUpdate(null);
  }, []);

  return {
    autoUpdate,
    skipVersion,
    startDownload,
    handleRestartForUpdate,
    handleManualCheck,
    dismissUpdate,
  };
}
