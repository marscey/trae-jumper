import { useState, useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { UpdateBanner } from "./components/UpdateBanner";
import { Sidebar } from "./components/Sidebar";
import { AddAccountModal } from "./components/AddAccountModal";
import { ContextMenu } from "./components/ContextMenu";
import { DetailModal } from "./components/DetailModal";
import { UsageModal } from "./components/UsageModal";
import { Toast } from "./components/Toast";
import { ConfirmModal } from "./components/ConfirmModal";
import { InfoModal } from "./components/InfoModal";
import { UpdateTokenModal } from "./components/UpdateTokenModal";
import { Dashboard } from "./pages/Dashboard";
import { AccountsPage } from "./pages/AccountsPage";
import { Settings } from "./pages/Settings";
import { About } from "./pages/About";
import { useToast } from "./hooks/useToast";
import { useAccountsData } from "./hooks/useAccountsData";
import { useAutoUpdate } from "./hooks/useAutoUpdate";
import { useAppModals } from "./hooks/useAppModals";
import { useTokenActions } from "./features/accounts/useTokenActions";
import { useCheckinActions } from "./features/accounts/useCheckinActions";
import { useAccountActions } from "./features/accounts/useAccountActions";
import { useImportExport } from "./features/accounts/useImportExport";
import * as api from "./api";
import type { ViewMode } from "./types";
import "./App.css";

/**
 * 应用组合根：
 * - 数据层：useAccountsData（账号列表/积分/签到状态 + 定时刷新）
 * - 弹窗层：useAppModals（全局弹窗状态）
 * - 操作层：useTokenActions / useCheckinActions / useAccountActions / useImportExport
 * - 自更新：useAutoUpdate（后台检查 + 静默下载安装）
 */
function App() {
  // 页面导航与客户端信息
  const [currentPage, setCurrentPage] = useState("dashboard");
  const [viewMode, setViewMode] = useState<ViewMode>("grid");
  const [currentClientName, setCurrentClientName] = useState<string>("");
  const [currentLoginUrl, setCurrentLoginUrl] = useState<string>("");
  const [showAddModal, setShowAddModal] = useState(false);

  const { toasts, addToast, removeToast } = useToast();

  // 同步窗口标题（含当前客户端名）
  useEffect(() => {
    const title = currentClientName
      ? `TraeJumper · ${currentClientName}`
      : "TraeJumper";
    try {
      getCurrentWindow().setTitle(title).catch(() => {});
    } catch {
      // 浏览器开发环境（无 Tauri）下忽略窗口标题设置
    }
  }, [currentClientName]);

  // 启动时加载当前客户端名 + 全局签到配置
  useEffect(() => {
    api.getTraeApps()
      .then((apps) => {
        const current = apps.find((a) => a.is_current);
        if (current) {
          setCurrentClientName(current.display_name);
          setCurrentLoginUrl(current.login_url);
        }
      })
      .catch(() => {});
  }, []);

  // ==== 数据层 ====
  const {
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
  } = useAccountsData();

  // 启动时同步全局签到配置（checkinConfig 由 useAccountsData 持有）
  useEffect(() => {
    api.getCheckinConfig()
      .then((cfg) => setCheckinConfig(cfg))
      .catch(() => {});
  }, [setCheckinConfig]);

  // ==== 自更新 ====
  const { autoUpdate, setAutoUpdate, handleRestartForUpdate } = useAutoUpdate(addToast);

  // ==== 弹窗层 ====
  const {
    confirmModal,
    setConfirmModal,
    infoModal,
    setInfoModal,
    contextMenu,
    setContextMenu,
    detailAccount,
    setDetailAccount,
    usageModal,
    setUsageModal,
    updateTokenModal,
    setUpdateTokenModal,
  } = useAppModals();

  // ==== 操作层 ====
  const {
    refreshingIds,
    renewingClientIds,
    handleRefreshAccount,
    handleManualRefreshToken,
    handleRenewClientToken,
    handleUpdateToken,
    handleOpenUpdateToken,
  } = useTokenActions({
    accounts,
    setAccounts,
    markTokenExpired,
    addToast,
    setUpdateTokenModal,
  });

  const {
    checkinLoading,
    handleCheckin,
    handleCheckinAll,
    handleViewCheckinHeaders,
  } = useCheckinActions({
    accounts,
    setAccounts,
    addToast,
    setConfirmModal,
    setInfoModal,
    refreshAccount: handleRefreshAccount,
  });

  const {
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
  } = useAccountActions({
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
    refreshAccount: handleRefreshAccount,
  });

  const {
    importing,
    handleShowExportInfo,
    handleShowImportInfo,
    handleImportAccounts,
    handleClearData,
  } = useImportExport({
    accounts,
    loadAccounts,
    addToast,
    setConfirmModal,
    setInfoModal,
    setSelectedIds,
  });

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

        {autoUpdate && (
          <UpdateBanner
            status={autoUpdate.status}
            version={autoUpdate.version}
            percent={autoUpdate.percent}
            onRestart={handleRestartForUpdate}
            onDismiss={() => setAutoUpdate(null)}
          />
        )}

        {currentPage === "dashboard" && (
          <Dashboard accounts={accounts} currentClientName={currentClientName} />
        )}

        {currentPage === "accounts" && (
          <AccountsPage
            accounts={accounts}
            creditsLoadingIds={creditsLoadingIds}
            refreshingIds={refreshingIds}
            renewingClientIds={renewingClientIds}
            selectedIds={selectedIds}
            loading={loading}
            currentClientName={currentClientName}
            checkinConfig={checkinConfig}
            viewMode={viewMode}
            onViewModeChange={setViewMode}
            checkinLoading={checkinLoading}
            checkCurrentLoading={checkCurrentLoading}
            onAddAccountClick={() => setShowAddModal(true)}
            onDeleteExpired={handleDeleteExpiredAccounts}
            onImportMenu={handleShowImportInfo}
            onExportMenu={handleShowExportInfo}
            onImportClick={handleImportAccounts}
            onSelectAll={handleSelectAll}
            onBatchRefresh={handleBatchRefresh}
            onCheckCurrent={handleCheckCurrent}
            onCheckinAll={handleCheckinAll}
            onSwitchSelected={handleSwitchAccount}
            onBatchDelete={handleBatchDelete}
            onSelect={handleSelectAccount}
            onContextMenu={handleContextMenu}
            onViewDetail={handleViewDetail}
            onRefresh={handleManualRefreshToken}
            onRenewClient={handleRenewClientToken}
          />
        )}

        {currentPage === "settings" && (
          <>
            <header className="page-header">
              <div className="header-left">
                <h2 className="page-title">设置</h2>
                <p>配置应用程序选项</p>
              </div>
            </header>
            <Settings onToast={addToast} onExport={handleShowExportInfo} onImport={handleShowImportInfo} onClearData={handleClearData} onClientChange={setCurrentClientName} onAccountsChanged={handleAccountsStatusSync} />
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
          extraText={infoModal.extraText}
          onExtra={infoModal.onExtra}
        />
      )}

      {/* 确认弹窗（放在所有弹窗之后渲染，配合 confirm-overlay 高 z-index，确保叠加时始终在最上层） */}
      {confirmModal && (
        <ConfirmModal
          isOpen={confirmModal.isOpen}
          title={confirmModal.title}
          message={confirmModal.message}
          type={confirmModal.type}
          icon={confirmModal.icon}
          confirmText={confirmModal.confirmText || "确定"}
          cancelText="取消"
          onConfirm={confirmModal.onConfirm}
          onCancel={() => setConfirmModal(null)}
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
          onViewUsage={() => {
            const acc = accounts.find(a => a.id === contextMenu.accountId);
            if (acc) {
              setUsageModal({
                accountId: acc.id,
                accountName: acc.email || acc.name || acc.id.slice(0, 8),
              });
            }
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
          onSwitchAccount={() => {
            handleSwitchAccount(contextMenu.accountId);
            setContextMenu(null);
          }}
          onClaimGift={() => {
            handleClaimGift(contextMenu.accountId);
            setContextMenu(null);
          }}
          onCheckin={() => {
            handleCheckin(contextMenu.accountId);
            setContextMenu(null);
          }}
          onViewCheckinHeaders={() => {
            handleViewCheckinHeaders(contextMenu.accountId);
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
        clientName={currentClientName}
        loginUrl={currentLoginUrl}
      />

      {/* 详情弹窗 */}
      <DetailModal
        isOpen={!!detailAccount}
        onClose={() => setDetailAccount(null)}
        account={detailAccount}
        usage={detailAccount?.usage || null}
        credits={detailAccount?.credits || null}
        checkinConfig={checkinConfig}
        onAccountChanged={async () => {
          await refreshAccountsData();
          // 只同步 restart_after_refresh_override 字段，保留 prev 的完整 token 等详情数据
          // （accounts 是 AccountBrief[]，如果 { ...fresh } 展开会把 cookies/jwt_token 等覆盖没）
          setDetailAccount((prev) => {
            if (!prev) return prev;
            const fresh = accounts.find((a) => a.id === prev.id);
            if (!fresh) return prev;
            return {
              ...prev,
              restart_after_refresh_override: fresh.restart_after_refresh_override,
              checkin_status: fresh.checkin_status,
            };
          });
        }}
      />

      {/* 用量明细弹窗 */}
      <UsageModal
        isOpen={!!usageModal}
        accountId={usageModal?.accountId || ""}
        accountName={usageModal?.accountName || ""}
        onClose={() => setUsageModal(null)}
      />

      {/* 更新 Token 弹窗 */}
      <UpdateTokenModal
        isOpen={!!updateTokenModal}
        accountId={updateTokenModal?.accountId || ""}
        accountName={updateTokenModal?.accountName || ""}
        clientName={currentClientName}
        onClose={() => setUpdateTokenModal(null)}
        onUpdate={handleUpdateToken}
        onToast={addToast}
        onSuccess={() => {
          // 客户端读取 / 浏览器登录 更新成功后，刷新该账号数据
          // （与右击"刷新数据"同一逻辑：状态标签、用量、积分）
          const id = updateTokenModal?.accountId;
          if (id) {
            handleRefreshAccount(id).catch(() => {});
          }
        }}
      />

      {/* 批量签到全屏 loading 遮罩 */}
      {checkinLoading && (
        <div className="global-busy-overlay">
          <div className="global-busy-card">
            <div className="global-busy-spinner" />
            <div className="global-busy-title">正在批量签到 {accounts.length} 个账号</div>
            <div className="global-busy-sub">
              每个账号之间有随机延迟，预计 10~60 秒完成<br />
              请勿在此期间切换账号或关闭应用
            </div>
          </div>
        </div>
      )}

      {/* 账号切换 loading 遮罩 */}
      {switchLoading && (
        <div className="global-busy-overlay">
          <div className="global-busy-card">
            <div className="global-busy-spinner" />
            <div className="global-busy-title">正在切换账号</div>
            <div className="global-busy-sub">
              自动关闭 {currentClientName || "客户端"} → 清除旧登录态 → 写入新账号 → 重新打开<br />
              请勿在此期间进行其他操作
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default App;
