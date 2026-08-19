import { useState, useEffect, useCallback, useRef } from "react";
import { createPortal } from "react-dom";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Sidebar } from "./components/Sidebar";
import { AccountCard } from "./components/AccountCard";
import { AccountListItem } from "./components/AccountListItem";
import { AddAccountModal } from "./components/AddAccountModal";
import { ContextMenu } from "./components/ContextMenu";
import { DetailModal } from "./components/DetailModal";
import { Toast } from "./components/Toast";
import { ConfirmModal } from "./components/ConfirmModal";
import { InfoModal } from "./components/InfoModal";
import { UpdateTokenModal } from "./components/UpdateTokenModal";
import { Dashboard } from "./pages/Dashboard";
import { Settings } from "./pages/Settings";
import { About } from "./pages/About";
import { useToast } from "./hooks/useToast";
import * as api from "./api";
import type { AccountBrief, CreditSummary, UsageSummary } from "./types";
import "./App.css";

interface AccountWithUsage extends AccountBrief {
  usage?: UsageSummary | null;
  credits?: CreditSummary | null;
}

type ViewMode = "grid" | "list";

function App() {
  const [accounts, setAccounts] = useState<AccountWithUsage[]>([]);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [showAddModal, setShowAddModal] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [importing, setImporting] = useState(false);
  const [currentPage, setCurrentPage] = useState("dashboard");
  const [viewMode, setViewMode] = useState<ViewMode>("grid");
  const [currentClientName, setCurrentClientName] = useState<string>("");

  // 同步窗口标题（含当前客户端名）
  useEffect(() => {
    const title = currentClientName
      ? `TraeJumper · ${currentClientName}`
      : "TraeJumper";
    getCurrentWindow().setTitle(title).catch(() => {});
  }, [currentClientName]);

  // 启动时加载当前客户端名
  useEffect(() => {
    api.getTraeApps()
      .then((apps) => {
        const current = apps.find((a) => a.is_current);
        if (current) setCurrentClientName(current.display_name);
      })
      .catch(() => {});
  }, []);

  // 使用自定义 Toast hook
  const { toasts, addToast, removeToast } = useToast();

  // 确认弹窗状态
  const [confirmModal, setConfirmModal] = useState<{
    isOpen: boolean;
    title: string;
    message: string;
    type: "danger" | "warning" | "info";
    onConfirm: () => void;
  } | null>(null);

  // 右键菜单状态
  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
    accountId: string;
  } | null>(null);

  // 详情弹窗状态
  const [detailAccount, setDetailAccount] = useState<AccountWithUsage | null>(null);

  // 刷新中的账号 ID
  const [refreshingIds, setRefreshingIds] = useState<Set<string>>(new Set());

  // 更新 Token 弹窗状态
  const [updateTokenModal, setUpdateTokenModal] = useState<{
    accountId: string;
    accountName: string;
  } | null>(null);

  // 操作组下拉菜单状态
  const [actionMenuOpen, setActionMenuOpen] = useState(false);
  const [menuPos, setMenuPos] = useState<{ top: number; right: number }>({ top: 0, right: 0 });
  const actionBtnRef = useRef<HTMLButtonElement>(null);

  const toggleActionMenu = () => {
    if (!actionMenuOpen && actionBtnRef.current) {
      const rect = actionBtnRef.current.getBoundingClientRect();
      setMenuPos({ top: rect.bottom + 8, right: window.innerWidth - rect.right });
    }
    setActionMenuOpen(!actionMenuOpen);
  };

  // 菜单打开时监听 scroll/resize，自动关闭防止定位错乱
  useEffect(() => {
    if (!actionMenuOpen) return;
    const close = () => setActionMenuOpen(false);
    window.addEventListener("scroll", close, true);
    window.addEventListener("resize", close);
    return () => {
      window.removeEventListener("scroll", close, true);
      window.removeEventListener("resize", close);
    };
  }, [actionMenuOpen]);

  // 信息展示弹窗状态
  const [infoModal, setInfoModal] = useState<{
    isOpen: boolean;
    title: string;
    icon: string;
    sections: Array<{
      title?: string;
      content: string;
      type?: "text" | "code" | "list";
    }>;
    confirmText: string;
    onConfirm: () => void;
  } | null>(null);

  // 加载账号列表（先显示列表，再后台加载使用量 + 积分）
  // 增加重试机制：Tauri 初始化可能需要时间，首次调用失败时自动重试
  const loadAccounts = useCallback(async (retries = 2): Promise<void> => {
    setLoading(true);
    try {
      const list = await api.getAccounts();

      setAccounts(list.map((account) => ({ ...account, usage: undefined, credits: undefined })));
      setLoading(false);

      // 后台并行加载：使用量（旧配额） + 新积分汇总
      if (list.length > 0) {
        const [usageResults, creditsResults] = await Promise.all([
          Promise.allSettled(list.map((account) => api.getAccountUsage(account.id))),
          Promise.allSettled(list.map((account) => api.getAccountCredits(account.id))),
        ]);

        setAccounts((prev) =>
          prev.map((account, index) => {
            const u = usageResults[index];
            const c = creditsResults[index];
            return {
              ...account,
              usage: u.status === 'fulfilled' ? u.value : null,
              credits: c.status === 'fulfilled' ? c.value : null,
            };
          })
        );
      }
    } catch (err: any) {
      console.error("[ERROR] loadAccounts failed:", err);
      if (retries > 0) {
        console.log(`[INFO] Retrying loadAccounts (${retries} retries left)...`);
        setTimeout(() => loadAccounts(retries - 1), 500);
      } else {
        setError(err.message || "加载账号失败");
        setLoading(false);
      }
    }
  }, []);

  // 初始化加载：等待 Tauri 就绪后加载账号
  // 顺序：等待 Tauri → 加载账号 → 刷新 Token → 重新加载
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

      // 加载账号列表
      await loadAccounts();

      if (cancelled) return;

      // 刷新即将过期的 Token，如果有刷新则重新加载
      try {
        const refreshed = await api.refreshAllTokens();
        if (refreshed.length > 0 && !cancelled) {
          console.log(`[INFO] 启动时自动刷新了 ${refreshed.length} 个 Token`);
          await loadAccounts();
        }
      } catch (e) {
        console.warn("[WARN] refreshAllTokens failed on startup:", e);
      }
    };

    init();

    // 定时刷新 Token（每 30 分钟）
    const interval = setInterval(async () => {
      try {
        const refreshed = await api.refreshAllTokens();
        if (refreshed.length > 0) {
          console.log(`[INFO] 定时自动刷新了 ${refreshed.length} 个 Token`);
          await loadAccounts();
        }
      } catch (e) {
        console.warn("[WARN] 定时刷新 Token 失败:", e);
      }
    }, 30 * 60 * 1000);

    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, [loadAccounts]);

  // 添加账号
  const handleAddAccount = async (token: string, cookies?: string) => {
    await api.addAccountByToken(token, cookies);
    addToast("success", "账号添加成功");
    await loadAccounts();
  };

  // 删除账号
  const handleDeleteAccount = async (accountId: string) => {
    setConfirmModal({
      isOpen: true,
      title: "删除账号",
      message: "确定要删除此账号吗？删除后无法恢复。",
      type: "danger",
      onConfirm: async () => {
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
        setConfirmModal(null);
      },
    });
  };

  // 刷新单个账号
  const handleRefreshAccount = async (accountId: string) => {
    // 防止重复刷新
    if (refreshingIds.has(accountId)) {
      return;
    }

    setRefreshingIds((prev) => new Set(prev).add(accountId));

    try {
      const [usage, credits] = await Promise.all([
        api.getAccountUsage(accountId),
        api.getAccountCredits(accountId).catch(() => null),
      ]);
      setAccounts((prev) =>
        prev.map((a) => (a.id === accountId ? { ...a, usage, credits } : a))
      );
      addToast("success", "数据刷新成功");
    } catch (err: any) {
      addToast("error", err.message || "刷新失败");
    } finally {
      setRefreshingIds((prev) => {
        const next = new Set(prev);
        next.delete(accountId);
        return next;
      });
    }
  };

  // 选择账号
  const handleSelectAccount = (accountId: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(accountId)) {
        next.delete(accountId);
      } else {
        next.add(accountId);
      }
      return next;
    });
  };

  // 全选/取消全选
  const handleSelectAll = () => {
    if (selectedIds.size === accounts.length) {
      setSelectedIds(new Set());
    } else {
      setSelectedIds(new Set(accounts.map((a) => a.id)));
    }
  };

  // 右键菜单
  const handleContextMenu = (e: React.MouseEvent, accountId: string) => {
    e.preventDefault();
    setContextMenu({ x: e.clientX, y: e.clientY, accountId });
  };

  // 复制 Token
  const handleCopyToken = async (accountId: string) => {
    try {
      const account = await api.getAccount(accountId);
      if (account.jwt_token) {
        await navigator.clipboard.writeText(account.jwt_token);
        addToast("success", "Token 已复制到剪贴板");
      } else {
        addToast("warning", "该账号没有有效的 Token");
      }
    } catch (err: any) {
      addToast("error", err.message || "获取 Token 失败");
    }
  };

  // 切换账号
  const handleSwitchAccount = async (accountId: string) => {
    const account = accounts.find((a) => a.id === accountId);
    if (!account) return;

    const clientName = currentClientName || "客户端";
    setConfirmModal({
      isOpen: true,
      title: "切换账号",
      message: `确定要切换到账号 "${account.email || account.name}" 吗？\n\n系统将自动关闭 ${clientName} 并切换登录信息。`,
      type: "warning",
      onConfirm: async () => {
        setConfirmModal(null);
        addToast("info", "正在切换账号，请稍候...");
        try {
          await api.switchAccount(accountId);
          await loadAccounts();
          addToast("success", `账号切换成功，请重新打开 ${clientName}`);
        } catch (err: any) {
          addToast("error", err.message || "切换账号失败");
        }
      },
    });
  };

  // 查看详情
  const handleViewDetail = async (accountId: string) => {
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
  };

  // 更新 Token
  const handleUpdateToken = async (accountId: string, token: string) => {
    try {
      const usage = await api.updateAccountToken(accountId, token);
      setAccounts((prev) =>
        prev.map((a) => (a.id === accountId ? { ...a, usage } : a))
      );
      addToast("success", "Token 更新成功，数据已刷新");
    } catch (err: any) {
      throw err; // 让弹窗显示错误
    }
  };

  // 打开更新 Token 弹窗
  const handleOpenUpdateToken = (accountId: string) => {
    const account = accounts.find((a) => a.id === accountId);
    if (account) {
      setUpdateTokenModal({
        accountId,
        accountName: account.email || account.name,
      });
    }
  };

  // 获取礼包（仅国际版有效；国内版积分体系已无"礼包"概念，直接给提示不请求后端）
  const handleClaimGift = async (accountId: string) => {
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
          await handleRefreshAccount(accountId);
          addToast("success", "礼包领取成功！额度已更新");
        } catch (err: any) {
          addToast("error", err.message || "领取礼包失败");
        }
      },
    });
  };

  // 显示导出说明
  const handleShowExportInfo = () => {
    if (accounts.length === 0) {
      addToast("warning", "没有账号可以导出");
      return;
    }

    setInfoModal({
      isOpen: true,
      title: "导出账号说明",
      icon: "📤",
      sections: [
        {
          title: "📄 导出格式",
          content: "JSON 文件 (.json)",
          type: "text"
        },
        {
          title: "📁 保存位置",
          content: "浏览器默认下载文件夹\n文件名格式：trae-accounts-YYYY-MM-DD.json",
          type: "text"
        },
        {
          title: "📋 文件内容",
          content: `<ul>
<li>所有账号的完整信息</li>
<li>Token 和 Cookies 数据</li>
<li>使用量统计信息</li>
<li>账号创建和更新时间</li>
</ul>`,
          type: "list"
        },
        {
          title: "✅ 导出后可以",
          content: `<ul>
<li>备份账号数据</li>
<li>迁移到其他设备</li>
<li>恢复误删的账号</li>
<li>分享给其他设备使用</li>
</ul>`,
          type: "list"
        },
        {
          title: "⚠️ 安全提示",
          content: `<ul>
<li><strong>导出文件包含敏感信息</strong></li>
<li><strong>请妥善保管导出的文件</strong></li>
<li><strong>不要分享给他人</strong></li>
<li>建议加密存储导出文件</li>
</ul>`,
          type: "list"
        },
        {
          content: `当前将导出 ${accounts.length} 个账号`,
          type: "text"
        }
      ],
      confirmText: "开始导出",
      onConfirm: () => {
        setInfoModal(null);
        handleExportAccounts();
      }
    });
  };

  // 导出账号
  const handleExportAccounts = async () => {
    try {
      const data = await api.exportAccounts();
      const blob = new Blob([data], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      const fileName = `trae-accounts-${new Date().toISOString().split("T")[0]}.json`;
      a.download = fileName;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
      addToast("success", `已导出 ${accounts.length} 个账号到下载文件夹：${fileName}`);
    } catch (err: any) {
      addToast("error", err.message || "导出失败");
    }
  };

  // 显示导入说明
  const handleShowImportInfo = () => {
    setInfoModal({
      isOpen: true,
      title: "导入账号说明",
      icon: "📥",
      sections: [
        {
          title: "📄 文件格式",
          content: "JSON 文件 (.json)",
          type: "text"
        },
        {
          title: "📋 文件结构示例",
          content: `{
  "accounts": [
    {
      "id": "账号ID",
      "name": "用户名",
      "email": "邮箱地址",
      "jwt_token": "Token字符串",
      "cookies": "Cookies字符串",
      "plan_type": "套餐类型",
      "created_at": 时间戳,
      "is_active": true,
      ...
    }
  ],
  "active_account_id": "当前活跃账号ID",
  "current_account_id": "当前使用账号ID"
}`,
          type: "code"
        },
        {
          title: "✅ 导入步骤",
          content: `<ul>
<li>确认后选择 JSON 文件</li>
<li>系统自动验证格式</li>
<li>导入所有有效账号</li>
</ul>`,
          type: "list"
        },
        {
          title: "⚠️ 注意事项",
          content: `<ul>
<li>仅支持本应用导出的格式</li>
<li>导入会自动跳过重复账号</li>
<li>建议定期备份账号数据</li>
</ul>`,
          type: "list"
        }
      ],
      confirmText: "选择文件",
      onConfirm: () => {
        setInfoModal(null);
        handleImportAccounts();
      }
    });
  };

  // 导入账号
  const handleImportAccounts = () => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".json";
    input.onchange = async (e) => {
      const file = (e.target as HTMLInputElement).files?.[0];
      if (!file) return;

      try {
        setImporting(true);
        const text = await file.text();
        const count = await api.importAccounts(text);
        addToast("success", `成功导入 ${count} 个账号`);
        await loadAccounts();
      } catch (err: any) {
        addToast("error", err.message || "导入失败");
      } finally {
        setImporting(false);
      }
    };
    input.click();
  };

  // 清空所有数据（弹窗确认）
  const handleClearData = () => {
    if (accounts.length === 0) {
      addToast("warning", "没有数据可清空");
      return;
    }

    setConfirmModal({
      isOpen: true,
      title: "清空所有数据",
      message: `确定要清空所有账号数据吗？\n\n此操作将删除全部 ${accounts.length} 个账号的数据，包括：\n• 账号信息\n• Token 和 Cookies\n• 使用量统计\n\n此操作不可恢复，请谨慎操作！`,
      type: "danger",
      onConfirm: async () => {
        setConfirmModal(null);
        try {
          const count = await api.clearAllAccounts();
          setSelectedIds(new Set());
          addToast("success", `已清空 ${count} 个账号数据`);
          await loadAccounts();
        } catch (err: any) {
          addToast("error", err.message || "清空数据失败");
        }
      },
    });
  };

  // 批量刷新账号：同时刷新 usage（旧配额）+ credits（新积分）
  // - 无选中：刷新全部账号
  // - 有选中：刷新选中账号
  // 底层复用 handleRefreshAccount 的数据范围（usage + credits），保持一致
  const handleBatchRefresh = async () => {
    const ids = selectedIds.size > 0
      ? Array.from(selectedIds)
      : accounts.map((a) => a.id);

    if (ids.length === 0) {
      addToast("warning", "暂无账号可刷新");
      return;
    }

    addToast("info", `正在刷新 ${ids.length} 个账号...`);

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
  };

  // 批量删除选中账号（优化：改进错误处理和反馈）
  const handleBatchDelete = () => {
    if (selectedIds.size === 0) {
      addToast("warning", "请先选择要删除的账号");
      return;
    }

    const ids = Array.from(selectedIds);
    setConfirmModal({
      isOpen: true,
      title: "批量删除",
      message: `确定要删除选中的 ${ids.length} 个账号吗？此操作无法撤销。`,
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
  };

  // 删除过期/失效账号
  const handleDeleteExpiredAccounts = () => {
    // 筛选出过期或失效的账号
    const expiredAccounts = accounts.filter((account) => {
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
  };

  return (
    <div className="app">
      <Sidebar currentPage={currentPage} onNavigate={setCurrentPage} />

      <div className="app-content">
        {error && (
          <div className="error-banner">
            {error}
            <button onClick={() => setError(null)}>×</button>
          </div>
        )}

        {currentPage === "dashboard" && (
          <Dashboard accounts={accounts} currentClientName={currentClientName} />
        )}

        {currentPage === "accounts" && (
          <>
            <header className="page-header">
              <div className="header-left">
                <h2 className="page-title">账号管理</h2>
                <p>管理您的账号</p>
              </div>
              <div className="header-right">
                {currentClientName && (
                  <span className="client-badge" title="当前管理的客户端">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" width="14" height="14">
                      <rect x="2" y="3" width="20" height="14" rx="2" ry="2"/>
                      <line x1="8" y1="21" x2="16" y2="21"/>
                      <line x1="12" y1="17" x2="12" y2="21"/>
                    </svg>
                    {currentClientName}
                  </span>
                )}
                <span className="account-count">共 {accounts.length} 个账号</span>
                <button className="add-btn" onClick={() => setShowAddModal(true)}>
                  <span>+</span> 添加账号
                </button>
                <div className="action-menu-wrapper">
                  <button
                    ref={actionBtnRef}
                    className="header-btn icon-only"
                    onClick={toggleActionMenu}
                    title="更多操作"
                  >
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" width="16" height="16">
                      <circle cx="12" cy="12" r="1"/>
                      <circle cx="12" cy="5" r="1"/>
                      <circle cx="12" cy="19" r="1"/>
                    </svg>
                  </button>
                  {actionMenuOpen && createPortal(
                    <>
                      <div className="action-menu-overlay" onClick={() => setActionMenuOpen(false)} />
                      <div
                        className="action-menu"
                        style={{ top: menuPos.top, right: menuPos.right }}
                      >
                        <button
                          className="action-menu-item danger"
                          onClick={() => { setActionMenuOpen(false); handleDeleteExpiredAccounts(); }}
                          disabled={accounts.length === 0}
                        >
                          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" width="16" height="16">
                            <path d="M3 6h18M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
                            <line x1="10" y1="11" x2="10" y2="17"/>
                            <line x1="14" y1="11" x2="14" y2="17"/>
                          </svg>
                          删除过期
                          {(() => {
                            const expiredCount = accounts.filter((account) => {
                              if (!account.token_expired_at) return false;
                              const expiry = new Date(account.token_expired_at).getTime();
                              if (isNaN(expiry)) return false;
                              return expiry < Date.now();
                            }).length;
                            return expiredCount > 0 ? <span className="badge-count">{expiredCount}</span> : null;
                          })()}
                        </button>
                        <button
                          className="action-menu-item"
                          onClick={() => { setActionMenuOpen(false); handleShowImportInfo(); }}
                        >
                          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" width="16" height="16">
                            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M7 10l5 5 5-5M12 15V3"/>
                          </svg>
                          导入账号
                        </button>
                        <button
                          className="action-menu-item"
                          onClick={() => { setActionMenuOpen(false); handleShowExportInfo(); }}
                          disabled={accounts.length === 0}
                        >
                          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" width="16" height="16">
                            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M17 8l-5-5-5 5M12 3v12"/>
                          </svg>
                          导出账号
                        </button>
                      </div>
                    </>,
                    document.body
                  )}
                </div>
              </div>
            </header>

            <main className="app-main">
              {accounts.length > 0 && (
                <div className="toolbar">
                  <div className="toolbar-left">
                    <label className="select-all">
                      <input
                        type="checkbox"
                        checked={selectedIds.size === accounts.length && accounts.length > 0}
                        onChange={handleSelectAll}
                      />
                      全选 ({selectedIds.size}/{accounts.length})
                    </label>
                    <button
                      className="batch-btn"
                      onClick={handleBatchRefresh}
                      title={selectedIds.size > 0 ? `刷新选中的 ${selectedIds.size} 个账号` : "刷新全部账号"}
                    >
                      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" width="14" height="14">
                        <path d="M23 4v6h-6M1 20v-6h6M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/>
                      </svg>
                      {selectedIds.size > 0 ? `刷新选中 (${selectedIds.size})` : "刷新全部"}
                    </button>
                    {selectedIds.size > 0 && (
                      <div className="batch-actions">
                        <button
                          className="batch-btn primary"
                          onClick={() => handleSwitchAccount(Array.from(selectedIds)[0])}
                          disabled={selectedIds.size !== 1}
                          title={selectedIds.size === 1 ? "切换到选中账号" : "请选择单个账号"}
                        >
                          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" width="14" height="14">
                            <path d="M17 1l4 4-4 4"/>
                            <path d="M3 11V9a4 4 0 0 1 4-4h14"/>
                            <path d="M7 23l-4-4 4-4"/>
                            <path d="M21 13v2a4 4 0 0 1-4 4H3"/>
                          </svg>
                          切换账号
                        </button>
                        <button className="batch-btn danger" onClick={handleBatchDelete}>
                          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" width="14" height="14">
                            <path d="M3 6h18M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
                          </svg>
                          删除
                        </button>
                      </div>
                    )}
                  </div>
                  <div className="toolbar-right">
                    <div className="view-toggle">
                      <button
                        className={`view-btn ${viewMode === "grid" ? "active" : ""}`}
                        onClick={() => setViewMode("grid")}
                        title="卡片视图"
                      >
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" width="16" height="16">
                          <rect x="3" y="3" width="7" height="7"/>
                          <rect x="14" y="3" width="7" height="7"/>
                          <rect x="3" y="14" width="7" height="7"/>
                          <rect x="14" y="14" width="7" height="7"/>
                        </svg>
                      </button>
                      <button
                        className={`view-btn ${viewMode === "list" ? "active" : ""}`}
                        onClick={() => setViewMode("list")}
                        title="列表视图"
                      >
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" width="16" height="16">
                          <line x1="8" y1="6" x2="21" y2="6"/>
                          <line x1="8" y1="12" x2="21" y2="12"/>
                          <line x1="8" y1="18" x2="21" y2="18"/>
                          <line x1="3" y1="6" x2="3.01" y2="6"/>
                          <line x1="3" y1="12" x2="3.01" y2="12"/>
                          <line x1="3" y1="18" x2="3.01" y2="18"/>
                        </svg>
                      </button>
                    </div>
                    <div className="toolbar-help">
                      <div className="help-tooltip">
                        <div className="help-tooltip-title">💡 快捷操作</div>
                        <div className="help-tooltip-item"><kbd>右键</kbd> 打开更多操作菜单</div>
                        <div className="help-tooltip-item"><kbd>双击</kbd> 查看账号详情</div>
                        <div className="help-tooltip-item"><kbd>Ctrl+A</kbd> 全选账号</div>
                      </div>
                      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                        <circle cx="12" cy="12" r="10"/>
                        <path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3"/>
                        <line x1="12" y1="17" x2="12.01" y2="17"/>
                      </svg>
                    </div>
                  </div>
                </div>
              )}

              {loading ? (
                <div className="loading">
                  <div className="spinner"></div>
                  <p>加载中...</p>
                </div>
              ) : accounts.length === 0 ? (
                <div className="empty-state">
                  <div className="empty-icon">📋</div>
                  <h3>暂无账号</h3>
                  <p>点击上方按钮添加账号，或导入已有账号</p>
                  <div className="empty-actions">
                    <button className="empty-btn primary" onClick={() => setShowAddModal(true)}>
                      添加账号
                    </button>
                    <button className="empty-btn" onClick={handleImportAccounts}>
                      导入账号
                    </button>
                  </div>
                </div>
              ) : viewMode === "grid" ? (
                <div className="account-grid">
                  {accounts.map((account) => (
                    <AccountCard
                      key={account.id}
                      account={account}
                      usage={account.usage || null}
                      credits={account.credits || null}
                      selected={selectedIds.has(account.id)}
                      onSelect={handleSelectAccount}
                      onContextMenu={handleContextMenu}
                      onViewDetail={handleViewDetail}
                    />
                  ))}
                </div>
              ) : (
                <div className="account-list">
                  {accounts.map((account) => (
                    <AccountListItem
                      key={account.id}
                      account={account}
                      usage={account.usage || null}
                      credits={account.credits || null}
                      selected={selectedIds.has(account.id)}
                      onSelect={handleSelectAccount}
                      onContextMenu={handleContextMenu}
                      onViewDetail={handleViewDetail}
                    />
                  ))}
                </div>
              )}
            </main>
          </>
        )}

        {currentPage === "settings" && (
          <>
            <header className="page-header">
              <div className="header-left">
                <h2 className="page-title">设置</h2>
                <p>配置应用程序选项</p>
              </div>
            </header>
            <Settings onToast={addToast} onExport={handleShowExportInfo} onImport={handleShowImportInfo} onClearData={handleClearData} onClientChange={setCurrentClientName} />
          </>
        )}

        {currentPage === "about" && (
          <>
            <header className="page-header">
              <div className="header-left">
                <h2 className="page-title">关于</h2>
                <p>应用程序信息</p>
              </div>
            </header>
            <About />
          </>
        )}
      </div>

      {/* Toast 通知 */}
      <Toast messages={toasts} onRemove={removeToast} />

      {/* 导入加载遮罩 */}
      {importing && (
        <div className="importing-overlay">
          <div className="importing-modal">
            <div className="spinner"></div>
            <p>正在导入账号...</p>
          </div>
        </div>
      )}

      {/* 确认弹窗 */}
      {confirmModal && (
        <ConfirmModal
          isOpen={confirmModal.isOpen}
          title={confirmModal.title}
          message={confirmModal.message}
          type={confirmModal.type}
          confirmText="确定"
          cancelText="取消"
          onConfirm={confirmModal.onConfirm}
          onCancel={() => setConfirmModal(null)}
        />
      )}

      {/* 信息展示弹窗 */}
      {infoModal && (
        <InfoModal
          isOpen={infoModal.isOpen}
          title={infoModal.title}
          icon={infoModal.icon}
          sections={infoModal.sections}
          confirmText={infoModal.confirmText}
          onConfirm={infoModal.onConfirm}
          onCancel={() => setInfoModal(null)}
        />
      )}

      {/* 右键菜单 */}
      {contextMenu && (
        <ContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          onClose={() => setContextMenu(null)}
          onViewDetail={() => {
            handleViewDetail(contextMenu.accountId);
            setContextMenu(null);
          }}
          onRefresh={() => {
            handleRefreshAccount(contextMenu.accountId);
            setContextMenu(null);
          }}
          onUpdateToken={() => {
            handleOpenUpdateToken(contextMenu.accountId);
            setContextMenu(null);
          }}
          onCopyToken={() => {
            handleCopyToken(contextMenu.accountId);
            setContextMenu(null);
          }}
          onSwitchAccount={() => {
            handleSwitchAccount(contextMenu.accountId);
            setContextMenu(null);
          }}
          onClaimGift={() => {
            handleClaimGift(contextMenu.accountId);
            setContextMenu(null);
          }}
          onDelete={() => {
            handleDeleteAccount(contextMenu.accountId);
            setContextMenu(null);
          }}
          isCurrent={accounts.find(a => a.id === contextMenu.accountId)?.is_current || false}
          showClaimGift={!accounts.find(a => a.id === contextMenu.accountId)?.credits?.is_credits_billing}
        />
      )}

      {/* 添加账号弹窗 */}
      <AddAccountModal
        isOpen={showAddModal}
        onClose={() => setShowAddModal(false)}
        onAdd={handleAddAccount}
        onToast={addToast}
        onAccountAdded={loadAccounts}
      />

      {/* 详情弹窗 */}
      <DetailModal
        isOpen={!!detailAccount}
        onClose={() => setDetailAccount(null)}
        account={detailAccount}
        usage={detailAccount?.usage || null}
        credits={detailAccount?.credits || null}
      />

      {/* 更新 Token 弹窗 */}
      <UpdateTokenModal
        isOpen={!!updateTokenModal}
        accountId={updateTokenModal?.accountId || ""}
        accountName={updateTokenModal?.accountName || ""}
        onClose={() => setUpdateTokenModal(null)}
        onUpdate={handleUpdateToken}
      />
    </div>
  );
}

export default App;
