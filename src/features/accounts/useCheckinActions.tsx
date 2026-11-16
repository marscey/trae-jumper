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
  /** 单账号数据刷新（来自 useTokenActions.handleRefreshAccount，签到成功后刷新积分） */
  refreshAccount: (accountId: string) => Promise<void>;
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

  // 批量签到所有账号
  const handleCheckinAll = useCallback(async () => {
    if (accounts.length === 0) {
      addToast("warning", "没有账号可签到");
      return;
    }
    setCheckinLoading(true);
    addToast(
      "info",
      `正在签到 ${accounts.length} 个账号（已签到的会自动跳过，仅连续签到时才有延迟）...`
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
      // 更新已签到账号的本地缓存状态
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
      // 重新加载所有账号积分汇总，把签到得到的 +200 反映出来
      try {
        const refreshed = await Promise.allSettled(
          accounts.map((a) => api.getAccountCredits(a.id))
        );
        setAccounts((prev) =>
          prev.map((account) => {
            const idx = accounts.findIndex((a) => a.id === account.id);
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

      // 统计汇总 toast
      const parts: string[] = [];
      if (successCount > 0) parts.push(`签到成功 ${successCount} 个`);
      if (skipCount > 0) parts.push(`已签到跳过 ${skipCount} 个`);
      if (failCount > 0) parts.push(`失败 ${failCount} 个`);
      if (failCount === 0) {
        addToast("success", `签到完成：${parts.join("，")}`);
      } else {
        addToast("warning", `签到完成：${parts.join("，")}`);
      }
    } catch (err: any) {
      addToast("error", err.message || "批量签到失败");
    } finally {
      setCheckinLoading(false);
    }
  }, [accounts, setAccounts, addToast]);

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
    handleViewCheckinHeaders,
    handleResetCheckinDevice,
  };
}
