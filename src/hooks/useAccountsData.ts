import { useState, useEffect, useCallback, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import * as api from "../api";
import type { AccountBrief, AccountWithUsage, CheckinAllStatusItem, CheckinConfig } from "../types";

/**
 * 账号数据状态中枢：列表加载 / 积分懒加载 / 签到状态后台加载 / 局部刷新，
 * 以及启动初始化（Tauri 就绪 → 续签 → 加载）、token-refreshed 事件监听、
 * 自动刷新定时器（读取后端配置，动态启停）。
 */
export function useAccountsData() {
  const [accounts, setAccounts] = useState<AccountWithUsage[]>([]);
  // 正在加载积分数据的账号 ID 集合（用于占位 loading UI）
  const [creditsLoadingIds, setCreditsLoadingIds] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  // 全局签到配置（DetailModal 三态开关需要总开关状态）
  const [checkinConfig, setCheckinConfig] = useState<CheckinConfig | null>(null);

  // 检测 API 返回"Token 已过期"(401)的错误，即时将该账号的状态标签更新为"已过期"
  // （后端已把 token_expired_at 置为当前时间并持久化，这里同步更新内存态，避免重构前一直显示"正常"）
  // 客户端登录中的账号除外：单活跃 Token 下其 Token 由客户端持有，401 属预期，不标红
  const markTokenExpired = useCallback((accountId: string, err: any) => {
    if (!err) return;
    const msg = String(err?.message || "");
    if (msg.includes("Token 已过期") || msg.includes("401")) {
      setAccounts((prev) =>
        prev.map((a) =>
          a.id === accountId && !a.is_client_active ? { ...a, token_expired_at: new Date().toISOString() } : a
        )
      );
    }
  }, []);

  // 独立后台加载签到状态（与积分解耦，失败静默，不阻塞主流程）
  const loadCheckinStatuses = useCallback(async (): Promise<void> => {
    try {
      const statuses = await api.checkinStatusAll();
      const statusMap = new Map<string, CheckinAllStatusItem>();
      for (const s of statuses) {
        statusMap.set(s.account_id, s);
      }
      setAccounts((prev) =>
        prev.map((account) => {
          const s = statusMap.get(account.id);
          return s
            ? {
                ...account,
                checkin_status: {
                  code: s.code,
                  message: s.message,
                  checked_in: s.checked_in,
                  credits: s.credits,
                  enable: s.enable,
                },
              }
            : account;
        })
      );
    } catch (err) {
      console.warn("[WARN] 批量查询签到状态失败:", err);
    }
  }, []);

  // 加载账号列表（先显示列表，再后台加载使用量 + 积分 + 签到状态）
  // 增加重试机制：Tauri 初始化可能需要时间，首次调用失败时自动重试
  const loadAccounts = useCallback(async (retries = 2): Promise<void> => {
    setLoading(true);
    try {
      const list = await api.getAccounts();

      // 先显示账号列表（占位：无积分数据，卡片显示 loading）
      setAccounts(list.map((account) => ({ ...account, usage: undefined, credits: undefined, checkin_status: undefined })));
      setLoading(false);

      if (list.length > 0) {
        // 标记所有账号进入"积分加载中"状态（占位 UI）
        setCreditsLoadingIds(new Set(list.map((a) => a.id)));

        // 并行加载积分：每个账号的请求各自独立，完成一个立即更新显示一个（不等待全部完成）
        await Promise.all(
          list.map(async (account, index) => {
            // 积分优先；旧配额作为回退由后端在 get_account_credits 内自动处理，
            // 这里不再额外调用 getAccountUsage，减少请求量（CN/WORK 用积分接口，国际版后端回退）
            const credits = await api
              .getAccountCredits(account.id)
              .catch((err) => { markTokenExpired(account.id, err); return null; });

            // 逐个更新对应账号（按 index 对齐）
            setAccounts((prev) =>
              prev.map((a, i) => (i === index ? { ...a, credits } : a))
            );

            // 若积分接口返回"非积分计费"（国际版），再补拉旧配额使用量
            if (credits && !credits.is_credits_billing) {
              const usage = await api
                .getAccountUsage(account.id)
                .catch((err) => { markTokenExpired(account.id, err); return null; });
              setAccounts((prev) =>
                prev.map((a, i) => (i === index ? { ...a, usage } : a))
              );
            }

            // 该账号积分加载完成，移出加载集合
            setCreditsLoadingIds((prev) => {
              const next = new Set(prev);
              next.delete(account.id);
              return next;
            });
          })
        );
      }

      // 签到状态独立后台并行加载，与积分解耦、不阻塞积分显示
      loadCheckinStatuses().catch(() => {});
    } catch (err: any) {
      console.error("[ERROR] loadAccounts failed:", err);
      if (retries > 0) {
        console.log(`[INFO] Retrying loadAccounts (${retries} retries left)...`);
        setTimeout(() => loadAccounts(retries - 1), 500);
      } else {
        setError(err.message || "加载账号失败");
        setCreditsLoadingIds(new Set());
        setLoading(false);
      }
    }
  }, [loadCheckinStatuses, markTokenExpired]);

  // 仅同步「当前账号」标记（切换目标客户端后调用）。
  // 只做本地状态更新（is_current 徽章），不重新请求账号列表与用量/积分/签到状态。
  const handleAccountsStatusSync = useCallback((current: AccountBrief | null) => {
    setAccounts((prev) =>
      prev.map((account) => ({ ...account, is_current: current ? account.id === current.id : false }))
    );
  }, []);

  // 局部刷新：后台请求新数据，逐个替换，不重置 UI、不闪烁
  const refreshAccountsData = useCallback(async () => {
    try {
      const list = await api.getAccounts();
      // 先同步账号元数据（名称、状态标签等），保留旧 usage/credits
      setAccounts((prev) => {
        const map = new Map(prev.map((a) => [a.id, a]));
        return list.map((a) => {
          const old = map.get(a.id);
          return old ? { ...old, ...a } : { ...a, usage: undefined, credits: undefined };
        });
      });

      // 逐个请求积分数据，完成后原地替换
      await Promise.all(
        list.map(async (account, index) => {
          const credits = await api
            .getAccountCredits(account.id)
            .catch((err) => { markTokenExpired(account.id, err); return null; });

          setAccounts((prev) =>
            prev.map((a, i) => (i === index ? { ...a, credits } : a))
          );

          if (credits && !credits.is_credits_billing) {
            const usage = await api
              .getAccountUsage(account.id)
              .catch((err) => { markTokenExpired(account.id, err); return null; });
            setAccounts((prev) =>
              prev.map((a, i) => (i === index ? { ...a, usage } : a))
            );
          }
        })
      );

      // 签到状态后台加载
      loadCheckinStatuses().catch(() => {});
    } catch (err) {
      console.warn("[WARN] 局部刷新失败:", err);
    }
  }, [loadCheckinStatuses, markTokenExpired]);

  // 初始化加载：等待 Tauri 就绪后加载账号
  // 顺序：等待 Tauri → 续签 Token（含客户端同步）→ 加载账号
  useEffect(() => {
    let cancelled = false;

    const init = async () => {
      // 等待 Tauri 就绪（最多 3 秒）
      for (let i = 0; i < 30; i++) {
        if (cancelled) return;
        if (api.hasTauri()) break;
        await new Promise((r) => setTimeout(r, 100));
      }

      if (cancelled) return;

      // 先续签 Token（含从客户端同步活跃账号的最新 Token），避免加载时命中过期 Token
      try {
        const refreshed = await api.refreshAllTokens();
        if (refreshed.length > 0 && !cancelled) {
          console.log(`[INFO] 启动时自动刷新了 ${refreshed.length} 个 Token`);
        }
      } catch (e) {
        console.warn("[WARN] refreshAllTokens failed on startup:", e);
      }

      if (cancelled) return;

      // 加载账号列表
      await loadAccounts();
    };

    init();

    return () => {
      cancelled = true;
    };
  }, [loadAccounts]);

  // 后端自动续签成功（token-refreshed 事件）→ 静默局部刷新账号列表，
  // 修复"列表显示过期但实际已续期"不同步的问题
  useEffect(() => {
    if (!api.hasTauri()) return;
    let unlisten: (() => void) | undefined;
    let disposed = false;
    listen<string[]>("token-refreshed", () => {
      refreshAccountsData();
    })
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      })
      .catch(() => {});
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [refreshAccountsData]);

  // 自动刷新：读取 auto_refresh 配置，动态启停定时器；每 30s 重新拉一次配置以响应设置页的即时修改
  // 使用 refreshAccountsData（局部刷新）而非 loadAccounts（全量重建），避免 UI 闪烁
  const autoRefreshTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const configWatcherRef = useRef<ReturnType<typeof setInterval> | null>(null);
  useEffect(() => {
    let stopped = false;
    let lastEnabled = false;
    let lastInterval = -1;

    const apply = (enabled: boolean, interval: number) => {
      // 配置未变 → 不动，避免反复重建
      if (enabled === lastEnabled && interval === lastInterval) return;
      lastEnabled = enabled;
      lastInterval = interval;

      if (autoRefreshTimerRef.current) {
        clearInterval(autoRefreshTimerRef.current);
        autoRefreshTimerRef.current = null;
      }
      if (!enabled) return;
      const ms = Math.max(1, interval) * 60 * 1000;
      autoRefreshTimerRef.current = setInterval(async () => {
        try {
          // 先续签 Token（含从客户端同步活跃账号的最新 Token），再局部刷新数据
          await api.refreshAllTokens().catch(() => {});
          await refreshAccountsData();
        } catch (e) {
          console.warn("[WARN] 自动刷新账号数据失败:", e);
        }
      }, ms);
    };

    const loadAndApply = async () => {
      if (stopped) return;
      try {
        const cfg = await api.getCheckinConfig();
        if (stopped) return;
        apply(cfg.auto_refresh_enabled, Number(cfg.auto_refresh_interval));
      } catch (e) {
        console.warn("[WARN] 加载自动刷新配置失败:", e);
      }
    };

    loadAndApply();
    // 配置监视器：设置页改完后，最多 30s 生效（不改则无副作用）
    configWatcherRef.current = setInterval(loadAndApply, 30_000);

    return () => {
      stopped = true;
      if (autoRefreshTimerRef.current) {
        clearInterval(autoRefreshTimerRef.current);
        autoRefreshTimerRef.current = null;
      }
      if (configWatcherRef.current) {
        clearInterval(configWatcherRef.current);
        configWatcherRef.current = null;
      }
    };
  }, [refreshAccountsData]);

  return {
    accounts,
    setAccounts,
    creditsLoadingIds,
    loading,
    error,
    setError,
    checkinConfig,
    setCheckinConfig,
    markTokenExpired,
    loadAccounts,
    refreshAccountsData,
    handleAccountsStatusSync,
  };
}
