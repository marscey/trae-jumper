import { useState, useCallback } from "react";
import * as api from "../../api";
import type { AccountWithUsage, CheckinHeaderEntry } from "../../types";
import type { AddToastFn } from "../../hooks/useToast";
import type { ConfirmModalState, InfoModalState } from "../../hooks/useAppModals";
import type { SetStateFn } from "./types";
import HeaderRow from "../../components/HeaderRow";

interface UseCheckinActionsParams {
  accounts: AccountWithUsage[];
  setAccounts: SetStateFn<AccountWithUsage[]>;
  addToast: AddToastFn;
  setConfirmModal: SetStateFn<ConfirmModalState | null>;
  setInfoModal: SetStateFn<InfoModalState | null>;
  /** 当前 tab 平台过滤（trae | doubao） */
  platformFilter: "trae" | "doubao";
  /** 当前 tab 下选中的账号 id（用于批量签到"没选=全部、选了=选中"） */
  scopedSelectedIds: string[];
  /** 单账号数据刷新（来自 useTokenActions.handleRefreshAccount，签到成功后刷新积分） */
  refreshAccount: (accountId: string) => Promise<void>;
}

/** 辅助：按平台过滤账号 */
function filterByPlatform<T extends { platform: string }>(
  list: T[],
  p: "trae" | "doubao"
): T[] {
  return list.filter((a) =>
    p === "doubao" ? a.platform === "doubao" : a.platform !== "doubao"
  );
}

/**
 * 签到相关操作：单账号签到、批量签到、签到请求头预览、重置签到虚拟设备。
 */
export function useCheckinActions({
  accounts,
  setAccounts,
  addToast,
  setConfirmModal,
  setInfoModal,
  platformFilter,
  scopedSelectedIds,
  refreshAccount,
}: UseCheckinActionsParams) {
  // 批量签到 loading
  const [checkinLoading, setCheckinLoading] = useState(false);

  // 单个账号签到
  const handleCheckin = useCallback(async (accountId: string) => {
    const account = accounts.find((a) => a.id === accountId);
    if (!account) return;
    // 先查签到状态：已签到就直接提示，避免重复 claim 接口调用
    try {
      const status = await api.checkinStatus(accountId);
      if (status.code === 0 && status.checked_in) {
        addToast("warning", `${account.email || account.name}：今日已签到`);
        // 更新缓存状态
        setAccounts((prev) =>
          prev.map((a) => a.id === accountId ? { ...a, checkin_status: status } : a)
        );
        return;
      }
    } catch (e) {
      // 查状态失败不阻塞，继续尝试签到
      console.warn("[WARN] checkinStatus 查询失败，继续签到：", e);
    }
    addToast("info", `正在为 ${account.email || account.name} 签到...`);
    try {
      const result = await api.checkin(accountId);
      if (result.code === 0) {
        addToast("success", `${account.email || account.name} 签到成功！+200 积分`);
        // 更新本地状态为已签到
        setAccounts((prev) =>
          prev.map((a) => a.id === accountId
            ? { ...a, checkin_status: { code: 0, message: "success", checked_in: true, credits: 200, enable: true } }
            : a)
        );
        // 刷新一下账号积分汇总（签到后 +200 体现出来）
        refreshAccount(accountId).catch(() => {});
      } else {
        addToast("warning", `${account.email || account.name}：${result.message}`);
      }
    } catch (err: any) {
      addToast("error", err.message || "签到失败");
    }
  }, [accounts, setAccounts, addToast, refreshAccount]);

  // 豆包签到（D10 checkin 接口既是查询也是 claim，幂等）
  const handleDoubaoCheckin = useCallback(async (accountId: string) => {
    const account = accounts.find((a) => a.id === accountId);
    if (!account) return;
    const name = account.name || account.email || accountId.slice(0, 8);
    // 本地日期（豆包返回 YYYY-MM-DD 为本地时区，不能用 toISOString 的 UTC）
    const now = new Date();
    const pad = (n: number) => String(n).padStart(2, "0");
    const today = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
    // 已签到（本地缓存判断）直接提示，避免重复请求
    if (account.doubao_checkin?.checked_in && account.doubao_checkin.checkin_date === today) {
      addToast("warning", `${name}：今日已签到（连续 ${account.doubao_checkin.continuous_days} 天）`);
      return;
    }
    addToast("info", `正在为 ${name} 执行豆包签到...`);
    try {
      const view = await api.doubaoCheckin(accountId);
      setAccounts((prev) =>
        prev.map((a) => (a.id === accountId ? { ...a, doubao_checkin: view } : a))
      );
      if (view.checked_in) {
        addToast(
          "success",
          `${name} 签到成功！连续 ${view.continuous_days} 天，+${view.reward_amount} 额度`
        );
      } else {
        addToast("warning", `${name}：签到状态未更新`);
      }
      refreshAccount(accountId).catch(() => {});
    } catch (err: any) {
      addToast("error", err.message || "豆包签到失败");
    }
  }, [accounts, setAccounts, addToast, refreshAccount]);

  // 批量签到：按当前 tab 平台分别调 Trae / 豆包签到
  // - 没选 → 当前 tab 全部账号
  // - 有选 → 当前 tab 下选中的
  const handleCheckinAll = useCallback(async () => {
    const scoped = filterByPlatform(accounts, platformFilter);
    const ids = scopedSelectedIds.length > 0
      ? scopedSelectedIds.filter((id) => scoped.some((a) => a.id === id))
      : scoped.map((a) => a.id);

    if (ids.length === 0) {
      addToast("warning", `暂无${platformFilter === "trae" ? "Trae" : "豆包"}账号可签到`);
      return;
    }

    setCheckinLoading(true);

    if (platformFilter === "doubao") {
      // 豆包：串行调 doubaoCheckin（后端已实现批量，但返回不带 accountId，
      // 我们这里按 ids 过滤后调 api.doubaoCheckinAll，再关联账号）
      addToast("info", `正在为 ${ids.length} 个豆包账号执行签到...`);
      try {
        // doubaoCheckinAll 目前返回 View[] 不带 accountId 关联，
        // 改走串行单账号调用，精确感知每个账号结果
        let ok = 0, skip = 0, fail = 0;
        const updatedPairs: [string, import("../../types").DoubaoCheckinView][] = [];
        for (const id of ids) {
          try {
            const v = await api.doubaoCheckin(id);
            updatedPairs.push([id, v]);
            if (v.checked_in) {
              if (new Date(v.checkin_date || "").toDateString() === new Date().toDateString()) {
                ok++;
              } else {
                skip++;
              }
            } else {
              fail++;
            }
          } catch (e: any) {
            fail++;
            console.warn(`[WARN] 豆包签到 account=${id}:`, e?.message || e);
          }
        }
        // 更新本地缓存
        if (updatedPairs.length > 0) {
          setAccounts((prev) =>
            prev.map((a) => {
              const pair = updatedPairs.find(([id]) => id === a.id);
              if (pair) return { ...a, doubao_checkin: pair[1] };
              return a;
            })
          );
          // 刷额度（doubao_quota）—— 签到获得 reward_amount，本地 quota 停留在旧值
          Promise.allSettled(
            updatedPairs.map(([id]) => refreshAccount(id))
          ).catch(() => {});
        }
        const parts: string[] = [];
        if (ok > 0) parts.push(`签到成功 ${ok}`);
        if (skip > 0) parts.push(`今日已签 ${skip}`);
        if (fail > 0) parts.push(`失败 ${fail}`);
        addToast(
          fail > 0 && ok === 0 ? "error" : ok === 0 && fail === 0 ? "warning" : "success",
          parts.join("，") || "无操作"
        );
      } catch (err: any) {
        addToast("error", err.message || "豆包批量签到失败");
      } finally {
        setCheckinLoading(false);
      }
      return;
    }

    // ==== Trae 路径 ====
    // 全量 → 走后端 checkinAll（带 skip 统计）；有选 → 串行调单账号
    if (scopedSelectedIds.length === 0) {
      addToast(
        "info",
        `正在签到 ${ids.length} 个 Trae 账号（已签到的会自动跳过，仅连续签到时才有延迟）...`
      );
      try {
        const results = await api.checkinAll();
        let successCount = 0;
        let skipCount = 0;
        let failCount = 0;
        for (const r of results) {
          if (r.code === 0) {
            if (r.skipped) skipCount++;
            else successCount++;
          } else {
            failCount++;
          }
        }
        const updated = new Map<string, boolean>();
        for (const r of results) {
          if (r.code === 0) updated.set(r.account_id, true);
        }
        if (updated.size > 0) {
          setAccounts((prev) =>
            prev.map((a) =>
              updated.has(a.id)
                ? { ...a, checkin_status: { code: 0, message: "success", checked_in: true, credits: 200, enable: true } }
                : a
            )
          );
        }
        // 刷新积分
        try {
          const refreshed = await Promise.allSettled(
            ids.map((id) => api.getAccountCredits(id))
          );
          setAccounts((prev) =>
            prev.map((account) => {
              const idx = ids.findIndex((id) => id === account.id);
              if (idx < 0) return account;
              const r = refreshed[idx];
              return {
                ...account,
                credits: r?.status === "fulfilled" ? r.value : account.credits,
              };
            })
          );
        } catch (e) {
          console.warn("[WARN] 签到后刷新积分汇总失败:", e);
        }
        const parts: string[] = [];
        if (successCount > 0) parts.push(`签到成功 ${successCount} 个`);
        if (skipCount > 0) parts.push(`今日已签 ${skipCount} 个`);
        if (failCount > 0) parts.push(`失败 ${failCount} 个`);
        addToast(
          failCount > 0 && successCount === 0 ? "error" : successCount === 0 && failCount === 0 ? "info" : "success",
          parts.join("，") || "无账号签到"
        );
      } catch (err: any) {
        console.error("[ERROR] checkinAll failed:", err);
        addToast("error", err?.message || "批量签到失败");
      } finally {
        setCheckinLoading(false);
      }
      return;
    }

    // 有选中 → 串行单账号
    addToast("info", `正在为 ${ids.length} 个选中的 Trae 账号执行签到...`);
    try {
      let ok = 0, skip = 0, fail = 0;
      for (const id of ids) {
        try {
          const s = await api.checkinStatus(id);
          if (s.code === 0 && s.checked_in) {
            skip++;
            continue;
          }
          const r = await api.checkin(id);
          if (r.code === 0) {
            ok++;
            // 签到成功 → 本地立即标记 checked_in=true，而不是把签到前 status 写回
            setAccounts((prev) =>
              prev.map((a) => a.id === id ? {
                ...a,
                checkin_status: { code: 0, message: "success", checked_in: true, credits: 200, enable: true },
              } : a)
            );
            await refreshAccount(id).catch(() => {});
          } else {
            fail++;
          }
        } catch { fail++; }
      }
      addToast(
        fail > 0 && ok === 0 ? "error" : ok === 0 && fail === 0 ? "info" : "success",
        (ok > 0 ? `签到成功 ${ok} 个，` : "") +
        (skip > 0 ? `今日已签 ${skip} 个，` : "") +
        (fail > 0 ? `失败 ${fail} 个` : "无操作")
      );
    } catch (err: any) {
      addToast("error", err.message || "批量签到失败");
    } finally {
      setCheckinLoading(false);
    }
  }, [accounts, platformFilter, scopedSelectedIds, setAccounts, addToast, refreshAccount]);

  // 查看账号签到请求头配置（右键菜单 → 签到请求头）
  const handleViewCheckinHeaders = useCallback(async (accountId: string) => {
    const account = accounts.find((a) => a.id === accountId);
    if (!account) return;
    try {
      const entries = await api.getCheckinHeaders(accountId);
      const fixed = entries.filter((e) => e.kind === "fixed");
      const accountOnes = entries.filter((e) => e.kind === "account");
      const variable = entries.filter(
        (e) => e.kind === "credential" || e.kind === "dynamic"
      );
      const renderRows = (
        list: CheckinHeaderEntry[],
        actions?: Record<string, { label: string; action: () => void }>
      ) => (
        <div className="info-header-list">
          {list.map((e) => (
            <HeaderRow
              key={e.name}
              entry={e}
              actionLabel={actions?.[e.name]?.label}
              onAction={actions?.[e.name]?.action}
            />
          ))}
        </div>
      );
      setInfoModal({
        isOpen: true,
        title: `签到请求头 · ${account.email || account.name}`,
        icon: "🧾",
        sections: [
          {
            title: `固定请求头 · ${fixed.length} 项（所有账号一致，对齐真实客户端抓包）`,
            content: renderRows(fixed),
            type: "list",
          },
          {
            title: `账号专属 · 虚拟设备 · ${accountOnes.length} 项（已持久化，跨天不变）`,
            content: renderRows(accountOnes, {
              "x-device-id": {
                label: "重新生成设备 ID",
                action: () => {
                  setConfirmModal({
                    isOpen: true,
                    title: "重新生成设备 ID",
                    message:
                      "⚠️ 确定要重新生成该账号的设备 ID 吗？\n\n" +
                      "• 将按照当前「设置 → 签到配置」中的设备 ID 策略重新生成一个全新的 x-device-id\n" +
                      "• 设备型号、机器码、会话 ID 保持不变\n" +
                      "• 新设备 ID 生效后，下次签到请求会携带它，服务端可能将该账号识别为一台新设备\n" +
                      "• 建议仅在遇到 9074 等设备风控拒绝时使用",
                    type: "warning",
                    onConfirm: async () => {
                      setConfirmModal(null);
                      setInfoModal(null);
                      try {
                        await api.regenerateDeviceId(accountId);
                        addToast("success", "设备 ID 已重新生成");
                        handleViewCheckinHeaders(accountId);
                      } catch (err: any) {
                        addToast("error", err.message || "操作失败");
                      }
                    },
                  });
                },
              },
              "x-device-brand": {
                label: "更换虚拟设备型号",
                action: () => {
                  setConfirmModal({
                    isOpen: true,
                    title: "更换虚拟设备型号",
                    message:
                      "⚠️ 确定要更换该账号的虚拟设备型号吗？\n\n" +
                      "• 将从设备型号池（MacBookAir / MacBookPro / Mac mini 等 8 款型号）中重新随机分配一台\n" +
                      "• 设备 ID、机器码、会话 ID 保持不变\n" +
                      "• 仅改变 x-device-brand 字段值，用于模拟该账号更换了同品牌不同型号的设备",
                    type: "warning",
                    onConfirm: async () => {
                      setConfirmModal(null);
                      setInfoModal(null);
                      try {
                        await api.swapDeviceBrand(accountId);
                        addToast("success", "虚拟设备型号已更换");
                        handleViewCheckinHeaders(accountId);
                      } catch (err: any) {
                        addToast("error", err.message || "操作失败");
                      }
                    },
                  });
                },
              },
            }),
            type: "list",
          },
          {
            title: `身份凭证 / 每次请求变化 · ${variable.length} 项`,
            content: renderRows(variable),
            type: "list",
          },
          {
            content:
              "每个账号在添加时即分配一台独立的虚拟设备（设备型号 / 设备ID / 机器码 / 会话ID），此后所有签到与状态查询请求始终携带同一组值，模拟该账号固定在一台设备上签到的真实行为；不同账号对应不同虚拟设备，避免多账号共享同一设备指纹被服务端聚类识别。",
            type: "hint",
          },
        ],
        confirmText: "关闭",
        onConfirm: () => setInfoModal(null),
        extraText: "⚠️ 重置完整签到档案",
        onExtra: () => handleResetCheckinDevice(accountId),
      });
    } catch (err: any) {
      addToast("error", err.message || "获取签到请求头失败");
    }
  }, [accounts, setInfoModal, setConfirmModal, addToast]);

  // 重置单个账号的签到虚拟设备档案（被风控时单独换指纹）
  const handleResetCheckinDevice = useCallback(async (accountId: string) => {
    const account = accounts.find((a) => a.id === accountId);
    const name = account?.email || account?.name || accountId;
    setConfirmModal({
      isOpen: true,
      title: "重置完整签到档案",
      icon: "🔄",
      message:
        `⚠️ 确定要重置账号「${name}」的签到虚拟设备吗？\n\n` +
        "这将为该账号重新分配一套全新的设备型号 / 设备ID / 机器码 / 会话ID，\n" +
        "换掉当前这套指纹（用于解决 9074 等风控拒绝）。\n" +
        "重置后请重新尝试签到。",
      type: "danger",
      onConfirm: async () => {
        setConfirmModal(null);
        try {
          await api.resetCheckinDevice(accountId);
          addToast("success", `已重置「${name}」的签到虚拟设备，请重新尝试签到`);
          // 关闭旧弹窗，刷新新的请求头展示
          setInfoModal(null);
          handleViewCheckinHeaders(accountId);
        } catch (err: any) {
          addToast("error", err.message || "重置签到设备失败");
        }
      },
    });
  }, [accounts, setConfirmModal, setInfoModal, addToast, handleViewCheckinHeaders]);

  return {
    checkinLoading,
    handleCheckin,
    handleCheckinAll,
    handleDoubaoCheckin,
    handleViewCheckinHeaders,
    handleResetCheckinDevice,
  };
}
