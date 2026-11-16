import { useState, useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import { AccountCard } from "../components/AccountCard";
import { AccountListItem } from "../components/AccountListItem";
import type { AccountWithUsage, CheckinConfig, ViewMode } from "../types";

interface AccountsPageProps {
  accounts: AccountWithUsage[];
  creditsLoadingIds: Set<string>;
  refreshingIds: Set<string>;
  renewingClientIds: Set<string>;
  selectedIds: Set<string>;
  loading: boolean;
  currentClientName: string;
  checkinConfig: CheckinConfig | null;
  viewMode: ViewMode;
  onViewModeChange: (mode: ViewMode) => void;
  checkinLoading: boolean;
  checkCurrentLoading: boolean;
  /** 头部「添加账号」按钮 */
  onAddAccountClick: () => void;
  /** 头部操作菜单：删除过期账号 */
  onDeleteExpired: () => void;
  /** 头部操作菜单：导入账号（带说明弹窗） */
  onImportMenu: () => void;
  /** 头部操作菜单：导出账号（带说明弹窗） */
  onExportMenu: () => void;
  /** 空状态「导入账号」按钮（直接打开文件选择） */
  onImportClick: () => void;
  /** 工具栏 */
  onSelectAll: () => void;
  onBatchRefresh: () => void;
  onCheckCurrent: () => void;
  onCheckinAll: () => void;
  onSwitchSelected: (accountId: string) => void;
  onBatchDelete: () => void;
  /** 账号卡片/行操作 */
  onSelect: (accountId: string) => void;
  onContextMenu: (e: React.MouseEvent, accountId: string) => void;
  onViewDetail: (accountId: string) => void;
  /** 手动续签 Token（卡片"刷新"） */
  onRefresh: (accountId: string) => void;
  /** 续签并写回客户端 */
  onRenewClient: (accountId: string) => void;
}

/** 账号管理页：头部工具栏 + 操作菜单 + 卡片/列表视图 */
export function AccountsPage({
  accounts,
  creditsLoadingIds,
  refreshingIds,
  renewingClientIds,
  selectedIds,
  loading,
  currentClientName,
  checkinConfig,
  viewMode,
  onViewModeChange,
  checkinLoading,
  checkCurrentLoading,
  onAddAccountClick,
  onDeleteExpired,
  onImportMenu,
  onExportMenu,
  onImportClick,
  onSelectAll,
  onBatchRefresh,
  onCheckCurrent,
  onCheckinAll,
  onSwitchSelected,
  onBatchDelete,
  onSelect,
  onContextMenu,
  onViewDetail,
  onRefresh,
  onRenewClient,
}: AccountsPageProps) {
  // 操作组下拉菜单状态（纯展示态，收拢在本组件内）
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

  return (
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
          <button className="add-btn" onClick={onAddAccountClick}>
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
                    onClick={() => { setActionMenuOpen(false); onDeleteExpired(); }}
                    disabled={accounts.length === 0}
                  >
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" width="16" height="16">
                      <path d="M3 6h18M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
                      <line x1="10" y1="11" x2="10" y2="17"/>
                      <line x1="14" y1="11" x2="14" y2="17"/>
                    </svg>
                    删除过期
                    {(() => {
                      // 与 handleDeleteExpiredAccounts 的筛选保持一致：客户端登录中的账号不算过期
                      const expiredCount = accounts.filter((account) => {
                        if (account.is_client_active) return false;
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
                    onClick={() => { setActionMenuOpen(false); onImportMenu(); }}
                  >
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" width="16" height="16">
                      <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M7 10l5 5 5-5M12 15V3"/>
                    </svg>
                    导入账号
                  </button>
                  <button
                    className="action-menu-item"
                    onClick={() => { setActionMenuOpen(false); onExportMenu(); }}
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
                  onChange={onSelectAll}
                />
                全选 ({selectedIds.size}/{accounts.length})
              </label>
              <button
                className="batch-btn"
                onClick={onBatchRefresh}
                title={selectedIds.size > 0 ? `刷新选中的 ${selectedIds.size} 个账号` : "刷新全部账号"}
              >
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" width="14" height="14">
                  <path d="M23 4v6h-6M1 20v-6h6M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/>
                </svg>
                {selectedIds.size > 0 ? `刷新选中 (${selectedIds.size})` : "刷新全部"}
              </button>
              <button
                className={`batch-btn ${checkCurrentLoading ? "loading" : ""}`}
                onClick={onCheckCurrent}
                disabled={checkCurrentLoading || accounts.length === 0}
                title="重新扫描所有客户端 storage.json，修正'当前活跃'标签（解决客户端已退出但 TJ 仍显示激活态的问题）"
              >
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" width="14" height="14">
                  <circle cx="12" cy="12" r="10"/>
                  <circle cx="12" cy="12" r="4"/>
                  <line x1="21.17" y1="8" x2="12" y2="8"/>
                  <line x1="3.95" y1="6.06" x2="8.54" y2="14"/>
                  <line x1="10.88" y1="21.94" x2="15.46" y2="14"/>
                </svg>
                {checkCurrentLoading ? "检测中..." : "检测当前账号"}
              </button>
              <button
                className="batch-btn"
                onClick={onCheckinAll}
                disabled={checkinLoading || accounts.length === 0}
                title="所有账号每日签到领取积分"
              >
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" width="14" height="14">
                  <rect x="3" y="4" width="18" height="18" rx="2" ry="2"/>
                  <line x1="16" y1="2" x2="16" y2="6"/>
                  <line x1="8" y1="2" x2="8" y2="6"/>
                  <line x1="3" y1="10" x2="21" y2="10"/>
                  <path d="M9 16l2 2 4-4"/>
                </svg>
                {checkinLoading ? "签到中..." : "全部签到"}
              </button>
              {selectedIds.size > 0 && (
                <div className="batch-actions">
                  <button
                    className="batch-btn primary"
                    onClick={() => onSwitchSelected(Array.from(selectedIds)[0])}
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
                  <button className="batch-btn danger" onClick={onBatchDelete}>
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
                  onClick={() => onViewModeChange("grid")}
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
                  onClick={() => onViewModeChange("list")}
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
              <button className="empty-btn primary" onClick={onAddAccountClick}>
                添加账号
              </button>
              <button className="empty-btn" onClick={onImportClick}>
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
                creditsLoading={creditsLoadingIds.has(account.id)}
                selected={selectedIds.has(account.id)}
                onSelect={onSelect}
                onContextMenu={onContextMenu}
                onViewDetail={onViewDetail}
                onRefresh={onRefresh}
                refreshing={refreshingIds.has(account.id)}
                onRenewClient={onRenewClient}
                renewingClient={renewingClientIds.has(account.id)}
                checkinConfig={checkinConfig}
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
                creditsLoading={creditsLoadingIds.has(account.id)}
                selected={selectedIds.has(account.id)}
                onSelect={onSelect}
                onContextMenu={onContextMenu}
                onViewDetail={onViewDetail}
                onRefresh={onRefresh}
                refreshing={refreshingIds.has(account.id)}
                onRenewClient={onRenewClient}
                renewingClient={renewingClientIds.has(account.id)}
                checkinConfig={checkinConfig}
              />
            ))}
          </div>
        )}
      </main>
    </>
  );
}
