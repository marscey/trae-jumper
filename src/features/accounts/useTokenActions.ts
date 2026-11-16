import { useState, useCallback } from "react";
import * as api from "../../api";
import type { AccountWithUsage } from "../../types";
import type { AddToastFn } from "../../hooks/useToast";
import type { SetStateFn } from "./types";
import type { UpdateTokenModalState } from "../../hooks/useAppModals";

interface UseTokenActionsParams {
  accounts: AccountWithUsage[];
  setAccounts: SetStateFn<AccountWithUsage[]>;
  markTokenExpired: (accountId: string, err: any) => void;
  addToast: AddToastFn;
  setUpdateTokenModal: SetStateFn<UpdateTokenModalState | null>;
}

/**
 * Token 相关操作：单账号数据刷新、cookies 手动续签、
 * 「续签并写回客户端」、更新 Token。
 */
export function useTokenActions({
  accounts,
  setAccounts,
  markTokenExpired,
  addToast,
  setUpdateTokenModal,
}: UseTokenActionsParams) {
  // 刷新中的账号 ID
  const [refreshingIds, setRefreshingIds] = useState<Set<string>>(new Set());
  const [renewingClientIds, setRenewingClientIds] = useState<Set<string>>(new Set());

  // 刷新单个账号
  const handleRefreshAccount = useCallback(async (accountId: string) => {
    // 防止重复刷新
    if (refreshingIds.has(accountId)) {
      return;
    }

    setRefreshingIds((prev) => new Set(prev).add(accountId));

    try {
      const [account, usage, credits] = await Promise.all([
        api.getAccount(accountId).catch(() => null),
        api.getAccountUsage(accountId),
        api.getAccountCredits(accountId).catch(() => null),
      ]);
      setAccounts((prev) =>
        prev.map((a) =>
          a.id === accountId ? { ...a, ...(account || {}), usage, credits } : a
        )
      );
      addToast("success", "数据刷新成功");
    } catch (err: any) {
      markTokenExpired(accountId, err);
      addToast("error", err.message || "刷新失败");
    } finally {
      setRefreshingIds((prev) => {
        const next = new Set(prev);
        next.delete(accountId);
        return next;
      });
    }
  }, [refreshingIds, setAccounts, markTokenExpired, addToast]);

  // 手动续签 Token：调用后端 refresh_token（cookies 续签），成功后刷新账号数据
  const handleManualRefreshToken = useCallback(async (accountId: string) => {
    if (refreshingIds.has(accountId)) return;
    setRefreshingIds((prev) => new Set(prev).add(accountId));
    try {
      await api.refreshToken(accountId);
      addToast("success", "Token 续签成功");
      // 续签成功后刷新该账号数据（token、积分、用量）
      await handleRefreshAccount(accountId);
    } catch (err: any) {
      addToast("error", err.message || "续签失败");
    } finally {
      setRefreshingIds((prev) => {
        const next = new Set(prev);
        next.delete(accountId);
        return next;
      });
    }
  }, [refreshingIds, handleRefreshAccount, addToast]);

  // 手动「续签并写回客户端」：立即 GetUserToken 续签写入客户端 storage.json（不经 cookies 续签守卫，不重启客户端）
  const handleRenewClientToken = useCallback(async (accountId: string) => {
    if (renewingClientIds.has(accountId)) return;
    setRenewingClientIds((prev) => new Set(prev).add(accountId));
    try {
      const msg = await api.renewTokenAndWriteClient(accountId);
      addToast("success", msg || "已续签并写回客户端");
      // 写回后刷新该账号数据（token、续签时间等）
      await handleRefreshAccount(accountId);
    } catch (err: any) {
      addToast("error", err.message || "续签并写回客户端失败");
    } finally {
      setRenewingClientIds((prev) => {
        const next = new Set(prev);
        next.delete(accountId);
        return next;
      });
    }
  }, [renewingClientIds, handleRefreshAccount, addToast]);

  // 更新 Token
  const handleUpdateToken = useCallback(async (accountId: string, token: string) => {
    try {
      await api.updateAccountToken(accountId, token);
      addToast("success", "Token 更新成功，账号数据已刷新");
      // 更新 Token 后刷新该账号数据（与右击"刷新数据"同一逻辑：状态标签、用量、积分）
      await handleRefreshAccount(accountId);
    } catch (err: any) {
      throw err; // 让弹窗显示错误
    }
  }, [handleRefreshAccount, addToast]);

  // 打开更新 Token 弹窗
  const handleOpenUpdateToken = useCallback((accountId: string) => {
    const account = accounts.find((a) => a.id === accountId);
    if (account) {
      setUpdateTokenModal({
        accountId,
        accountName: account.email || account.name,
      });
    }
  }, [accounts, setUpdateTokenModal]);

  return {
    refreshingIds,
    renewingClientIds,
    handleRefreshAccount,
    handleManualRefreshToken,
    handleRenewClientToken,
    handleUpdateToken,
    handleOpenUpdateToken,
  };
}
