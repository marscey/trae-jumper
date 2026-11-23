import { useState, useCallback } from "react";
import * as api from "../../api";
import type { AccountWithUsage } from "../../types";
import type { AddToastFn } from "../../hooks/useToast";
import type { ConfirmModalState, ContextMenuState } from "../../hooks/useAppModals";
import type { SetStateFn } from "./types";

interface UseAccountActionsParams {
  accounts: AccountWithUsage[];
  setAccounts: SetStateFn<AccountWithUsage[]>;
  loadAccounts: (retries?: number) => Promise<void>;
  refreshAccountsData: () => Promise<void>;
  markTokenExpired: (accountId: string, err: any) => void;
  addToast: AddToastFn;
  setConfirmModal: SetStateFn<ConfirmModalState | null>;
  setDetailAccount: SetStateFn<AccountWithUsage | null>;
  setContextMenu: SetStateFn<ContextMenuState | null>;
  currentClientName: string;
  /** 当前 tab 平台过滤（trae | doubao），批量操作按此限定 */
  platformFilter: "trae" | "doubao";
  /** 单账号数据刷新（来自 useTokenActions） */
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
 * 账号通用操作：添加 / 删除 / 选择 / 右键菜单 / 切换 / 详情 / 礼包 /
 * 批量刷新 / 检测当前账号 / 批量删除 / 删除过期账号。
 */
export function useAccountActions({
  accounts,
  setAccounts,
  loadAccounts,
  refreshAccountsData,
  markTokenExpired,
  addToast,
  setConfirmModal,
  setDetailAccount,
  setContextMenu,
  currentClientName,
  platformFilter,
  refreshAccount,
}: UseAccountActionsParams) {
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  // 账号切换 loading（防止切换过程中用户做其他操作）
  const [switchLoading, setSwitchLoading] = useState(false);
  // 检测当前账号（重新扫描客户端登录态）
  const [checkCurrentLoading, setCheckCurrentLoading] = useState(false);

  // 添加账号
  const handleAddAccount = useCallback(async (token: string, cookies?: string) => {
    await api.addAccountByToken(token, cookies);
    addToast("success", "账号添加成功");
    await loadAccounts();
  }, [addToast, loadAccounts]);

  // 删除账号
  const handleDeleteAccount = useCallback(async (accountId: string) => {
    const account = accounts.find((a) => a.id === accountId);
    const accountInfo = account
      ? `账号：${account.email || account.name || "未知"}`
      : "";
    const detailLines = [
      accountInfo,
      account ? `ID：${account.id}` : `ID：${accountId}`,
    ]; // 账号名 + ID 两个关键信息
    setConfirmModal({
      isOpen: true,
      title: "删除账号",
      message: `确定要删除以下账号吗？删除后无法恢复。\n\n${detailLines.join("\n")}`,
      type: "danger",
      onConfirm: async () => {
        // 点击确认立即关闭弹窗（与其他确认逻辑保持一致，避免 await 期间弹窗残留）
        setConfirmModal(null);
        try {
          await api.removeAccount(accountId);
          setSelectedIds((prev) => {
            const next = new Set(prev);
            next.delete(accountId);
            return next;
          });
          addToast("success", "账号已删除");
          await loadAccounts();
        } catch (err: any) {
          addToast("error", err.message || "删除账号失败");
        }
      },
    });
  }, [accounts, setConfirmModal, addToast, loadAccounts]);

  // 选择账号
  const handleSelectAccount = useCallback((accountId: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(accountId)) {
        next.delete(accountId);
      } else {
        next.add(accountId);
      }
      return next;
    });
  }, []);

  // 全选/取消全选（仅当前 tab 下的账号）
  const handleSelectAll = useCallback(() => {
    const scoped = filterByPlatform(accounts, platformFilter);
    const scopedIds = new Set(scoped.map((a) => a.id));
    const allScopedSelected = scoped.length > 0 && scoped.every((a) => selectedIds.has(a.id));
    if (allScopedSelected) {
      // 取消全选：只移除当前 tab 下的 id，保留其他 tab 可能选中的（防御性）
      setSelectedIds((prev) => {
        const next = new Set(prev);
        scopedIds.forEach((id) => next.delete(id));
        return next;
      });
    } else {
      // 全选：当前 tab 下的账号 id 合并进现有选中
      setSelectedIds((prev) => {
        const next = new Set(prev);
        scopedIds.forEach((id) => next.add(id));
        return next;
      });
    }
  }, [accounts, platformFilter, selectedIds, setSelectedIds]);

  // 右键菜单
  const handleContextMenu = useCallback((e: React.MouseEvent, accountId: string) => {
    e.preventDefault();
    setContextMenu({ x: e.clientX, y: e.clientY, accountId });
  }, [setContextMenu]);

  // 切换账号
  const handleSwitchAccount = useCallback(async (accountId: string) => {
    const account = accounts.find((a) => a.id === accountId);
    if (!account) return;

    const clientName = currentClientName || "客户端";

    const doSwitch = async (force: boolean) => {
      setSwitchLoading(true);
      addToast("info", "正在切换账号，请稍候...");
      try {
        await api.switchAccount(accountId, force);
        await loadAccounts();
        addToast("success", `账号切换成功，请重新打开 ${clientName}`);
      } catch (err: any) {
        const msg: string = err?.message || "切换账号失败";
        // 跨客户端同账号冲突：提示用户是否强制切换（会中断另一客户端会话）
        if (!force && msg.includes("已在") && msg.includes("登录并活跃")) {
          setSwitchLoading(false);
          setConfirmModal({
            isOpen: true,
            title: "账号已在其他客户端登录",
            message: `${msg}\n\n强制切换会中断该客户端正在进行的会话，是否继续？`,
            type: "warning",
            confirmText: "强制切换",
            onConfirm: async () => {
              setConfirmModal(null);
              await doSwitch(true);
            },
          });
        } else {
          addToast("error", msg);
        }
      } finally {
        setSwitchLoading(false);
      }
    };

    setConfirmModal({
      isOpen: true,
      title: "切换账号",
      message: `确定要切换到账号 "${account.email || account.name}" 吗？\n\n系统将自动关闭 ${clientName} 并切换登录信息。`,
      type: "warning",
      onConfirm: async () => {
        setConfirmModal(null);
        await doSwitch(false);
      },
    });
  }, [accounts, currentClientName, addToast, setConfirmModal, loadAccounts]);

  // 查看详情
  const handleViewDetail = useCallback(async (accountId: string) => {
    const account = accounts.find((a) => a.id === accountId);
    if (account) {
      try {
        // 获取完整的账号信息（包含 token 和 cookies）
        const fullAccount = await api.getAccount(accountId);
        setDetailAccount({ ...account, ...fullAccount });
      } catch (err: any) {
        addToast("error", "获取账号详情失败");
        console.error("获取账号详情失败:", err);
      }
    }
  }, [accounts, setDetailAccount, addToast]);

  // 获取礼包（仅国际版有效；国内版积分体系已无"礼包"概念，直接给提示不请求后端）
  const handleClaimGift = useCallback(async (accountId: string) => {
    const account = accounts.find((a) => a.id === accountId);
    if (!account) return;

    // 国内版 CN / WORK：credits.is_credits_billing=true → 隐藏此功能
    if (account.credits?.is_credits_billing) {
      addToast("info", "当前账号为积分体系（CN / WORK），无礼包可领取。若想获得更多积分请前往官网兑换码/签到/邀请奖励。");
      return;
    }

    setConfirmModal({
      isOpen: true,
      title: "获取礼包",
      message: `确定要为账号 "${account.email || account.name}" 领取周年礼包吗？\n\n领取后将自动刷新账号额度。`,
      type: "info",
      onConfirm: async () => {
        setConfirmModal(null);
        addToast("info", "正在领取礼包，请稍候...");
        try {
          await api.claimGift(accountId);
          // 刷新账号数据
          await refreshAccount(accountId);
          addToast("success", "礼包领取成功！额度已更新");
        } catch (err: any) {
          addToast("error", err.message || "领取礼包失败");
        }
      },
    });
  }, [accounts, addToast, setConfirmModal, refreshAccount]);

  // 批量刷新账号：同时刷新 usage（旧配额）+ credits（新积分）
  // - 无选中：刷新全部账号
  // - 有选中：刷新选中账号
  // 底层复用 handleRefreshAccount 的数据范围（usage + credits），保持一致
  const handleBatchRefresh = useCallback(async () => {
    const scopedIds = filterByPlatform(accounts, platformFilter).map((a) => a.id);
    // 选中的 id 里取当前 tab 的；没选 → 刷新当前 tab 全部
    const ids = selectedIds.size > 0
      ? scopedIds.filter((id) => selectedIds.has(id))
      : scopedIds;

    if (ids.length === 0) {
      addToast("warning", `暂无${platformFilter === "trae" ? "Trae" : "豆包"}账号可刷新`);
      return;
    }

    addToast("info", `正在刷新 ${ids.length} 个${platformFilter === "trae" ? "Trae" : "豆包"}账号...`);

    // 并行刷新（底层与 handleRefreshAccount 一致：usage + credits）
    const results = await Promise.allSettled(
      ids.map(async (id) => {
        try {
          const [usage, credits] = await Promise.all([
            api.getAccountUsage(id),
            api.getAccountCredits(id).catch(() => null),
          ]);
          setAccounts((prev) =>
            prev.map((a) => (a.id === id ? { ...a, usage, credits } : a))
          );
          return { id, success: true };
        } catch (err: any) {
          markTokenExpired(id, err);
          return { id, success: false, error: err.message };
        }
      })
    );

    // 统计结果
    const successCount = results.filter(
      (r) => r.status === 'fulfilled' && r.value.success
    ).length;
    const failCount = ids.length - successCount;

    if (failCount === 0) {
      addToast("success", `成功刷新 ${successCount} 个账号`);
    } else {
      addToast("warning", `刷新完成：${successCount} 成功，${failCount} 失败`);
    }
  }, [selectedIds, accounts, addToast, setAccounts, markTokenExpired, platformFilter]);

  // 检测当前账号：重新扫描所有已安装客户端的 storage.json，修正"客户端已退出但 TJ 还显示激活态"的标签
  const handleCheckCurrent = useCallback(async () => {
    if (checkCurrentLoading) return;
    setCheckCurrentLoading(true);
    try {
      addToast("info", "正在检测客户端登录态...");
      await refreshAccountsData();
      addToast("success", "检测完成，已更新账号状态");
    } catch (err) {
      console.warn("[WARN] 检测当前账号失败:", err);
      addToast("error", "检测失败，请重试");
    } finally {
      setCheckCurrentLoading(false);
    }
  }, [checkCurrentLoading, addToast, refreshAccountsData]);

  // 批量删除选中账号（优化：改进错误处理和反馈）
  const handleBatchDelete = useCallback(() => {
    if (selectedIds.size === 0) {
      addToast("warning", "请先选择要删除的账号");
      return;
    }

    const ids = Array.from(selectedIds);
    const selectedNames = ids
      .map((id) => {
        const a = accounts.find((acc) => acc.id === id);
        return `${a?.email || a?.name || "未知"}（${id}）`;
      })
      .join("\n");
    setConfirmModal({
      isOpen: true,
      title: "批量删除",
      message: `确定要删除选中的 ${ids.length} 个账号吗？此操作无法撤销。\n\n${selectedNames}`,
      type: "danger",
      onConfirm: async () => {
        setConfirmModal(null);
        addToast("info", `正在删除 ${ids.length} 个账号...`);

        // 并行删除所有选中的账号
        const results = await Promise.allSettled(
          ids.map((id) => api.removeAccount(id))
        );

        // 统计结果
        const successCount = results.filter((r) => r.status === 'fulfilled').length;
        const failCount = ids.length - successCount;

        setSelectedIds(new Set());
        await loadAccounts();

        if (failCount === 0) {
          addToast("success", `成功删除 ${successCount} 个账号`);
        } else {
          addToast("warning", `删除完成：${successCount} 成功，${failCount} 失败`);
        }
      },
    });
  }, [selectedIds, accounts, addToast, setConfirmModal, loadAccounts]);

  // 删除过期/失效账号
  const handleDeleteExpiredAccounts = useCallback(() => {
    // 筛选出过期或失效的账号（客户端登录中的账号除外：其 Token 由客户端持有，属正常状态）
    const expiredAccounts = accounts.filter((account) => {
      if (account.is_client_active) return false;
      if (!account.token_expired_at) return false;
      const expiry = new Date(account.token_expired_at).getTime();
      if (isNaN(expiry)) return false;
      return expiry < Date.now(); // Token 已过期
    });

    if (expiredAccounts.length === 0) {
      addToast("info", "没有找到过期或失效的账号");
      return;
    }

    setConfirmModal({
      isOpen: true,
      title: "删除过期账号",
      message: `检测到 ${expiredAccounts.length} 个过期账号，确定要删除吗？此操作无法撤销。`,
      type: "warning",
      onConfirm: async () => {
        setConfirmModal(null);
        addToast("info", `正在删除 ${expiredAccounts.length} 个过期账号...`);

        // 并行删除所有过期账号
        const results = await Promise.allSettled(
          expiredAccounts.map((account) => api.removeAccount(account.id))
        );

        // 统计结果
        const successCount = results.filter((r) => r.status === 'fulfilled').length;
        const failCount = expiredAccounts.length - successCount;

        setSelectedIds(new Set());
        await loadAccounts();

        if (failCount === 0) {
          addToast("success", `成功删除 ${successCount} 个过期账号`);
        } else {
          addToast("warning", `删除完成：${successCount} 成功，${failCount} 失败`);
        }
      },
    });
  }, [accounts, addToast, setConfirmModal, loadAccounts]);

  return {
    selectedIds,
    setSelectedIds,
    switchLoading,
    checkCurrentLoading,
    handleAddAccount,
    handleDeleteAccount,
    handleSelectAccount,
    handleSelectAll,
    handleContextMenu,
    handleSwitchAccount,
    handleViewDetail,
    handleClaimGift,
    handleBatchRefresh,
    handleCheckCurrent,
    handleBatchDelete,
    handleDeleteExpiredAccounts,
  };
}
