import { useState, useEffect } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import * as api from "../../api";
import type { AccountBrief, TraeAppInfo } from "../../types";
import type { AddToastFn } from "../../hooks/useToast";
import { ConfirmModal } from "../../components/ConfirmModal";

interface ConfirmState {
  isOpen: boolean;
  title: string;
  message: string;
  type: "danger" | "warning" | "info";
  icon?: string;
  onConfirm: () => void;
}

interface ClientSectionProps {
  onToast?: AddToastFn;
  /** 客户端切换后回调，用于全局同步窗口标题等 */
  onClientChange?: (clientName: string) => void;
  /** 账号状态变更后回调（切换客户端并同步当前账号后触发）。参数为同步后的当前账号（null=未检测到已登录已知账号） */
  onAccountsChanged?: (current: AccountBrief | null) => void;
}

/** 设置页 · 客户端分区：目标客户端 / 安装路径 / 机器码 / 设备 ID / 账号切换 */
export function ClientSection({ onToast, onClientChange, onAccountsChanged }: ClientSectionProps) {
  const [traeApps, setTraeApps] = useState<TraeAppInfo[]>([]);
  const [switchingApp, setSwitchingApp] = useState(false);
  const [traeMachineId, setTraeMachineId] = useState<string>("");
  const [traeRefreshing, setTraeRefreshing] = useState(false);
  const [traeDeviceId, setTraeDeviceId] = useState<string>("");
  const [deviceRefreshing, setDeviceRefreshing] = useState(false);
  const [clearingTrae, setClearingTrae] = useState(false);
  const [traePath, setTraePath] = useState<string>("");
  const [traePathLoading, setTraePathLoading] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [switchAsNewDevice, setSwitchAsNewDevice] = useState(false);
  const [switchSettingSaving, setSwitchSettingSaving] = useState(false);
  const [confirmState, setConfirmState] = useState<ConfirmState | null>(null);

  // 加载应用列表并同步当前客户端名到全局
  const loadTraeApps = async () => {
    try {
      const apps = await api.getTraeApps();
      setTraeApps(apps);
      const current = apps.find((a) => a.is_current);
      if (current && onClientChange) {
        onClientChange(current.display_name);
      }
    } catch (err: any) {
      console.error("获取 Trae 应用列表失败:", err);
    }
  };

  // 切换目标客户端（切换前确认；后端自动扫描并保存新客户端安装路径；切换后同步当前账号）
  const handleSwitchApp = (appKey: string) => {
    if (switchingApp) return;
    const target = traeApps.find((a) => a.key === appKey);
    const targetName = target?.display_name || appKey;
    const currentName = traeApps.find((a) => a.is_current)?.display_name || "当前客户端";

    setConfirmState({
      isOpen: true,
      title: "切换目标客户端",
      icon: "🔀",
      message:
        `确定要将目标客户端从「${currentName}」切换到「${targetName}」吗？\n\n` +
        "切换后：\n" +
        "• 账号切换、登录、机器码、签到等操作将作用于新的客户端\n" +
        "• 将自动检测新客户端当前登录的账号并同步显示\n" +
        "• 原客户端下登录的账号如果在新客户端不存在，其「当前使用」状态将被清空\n\n" +
        "建议切换前先关闭目标客户端。",
      type: "warning",
      onConfirm: async () => {
        setConfirmState(null);
        setSwitchingApp(true);
        const startedAt = Date.now();
        try {
          await api.setCurrentTraeApp(appKey);
          await Promise.all([loadTraeApps(), loadTraeMachineId(), loadTraeDeviceId(), loadTraePath()]);
          // 同步当前账号：读取新客户端已登录账号，更新 current_account_id
          const current = await api.syncCurrentAccount();
          if (current) {
            onToast?.("success", `目标客户端已切换，当前账号: ${current.email || current.name}`);
          } else {
            onToast?.("info", "目标客户端已切换，但未检测到已登录的已知账号");
          }
          // 仅更新当前账号标记，不重刷整个账号列表
          onAccountsChanged?.(current);
        } catch (err: any) {
          onToast?.("error", err.message || "切换失败");
        } finally {
          // 最小显示 500ms，防止切换太快导致遮罩一闪而过（macOS 扫描路径很快）
          const elapsed = Date.now() - startedAt;
          const minShow = 500;
          if (elapsed < minShow) {
            setTimeout(() => setSwitchingApp(false), minShow - elapsed);
          } else {
            setSwitchingApp(false);
          }
        }
      },
    });
  };

  // 加载 Trae IDE 机器码
  const loadTraeMachineId = async () => {
    setTraeRefreshing(true);
    try {
      const id = await api.getTraeMachineId();
      setTraeMachineId(id);
    } catch (err: any) {
      console.error("获取 Trae IDE 机器码失败:", err);
      setTraeMachineId("未找到");
    } finally {
      setTraeRefreshing(false);
    }
  };

  // 加载 Trae 客户端的本机真实 device-id
  const loadTraeDeviceId = async () => {
    setDeviceRefreshing(true);
    try {
      const id = await api.getTraeDeviceId();
      setTraeDeviceId(id);
    } catch (err: any) {
      console.error("获取 Trae 客户端设备 ID 失败:", err);
      setTraeDeviceId("未找到");
    } finally {
      setDeviceRefreshing(false);
    }
  };

  // 加载 Trae IDE 路径（未保存或已失效时自动扫描）
  const loadTraePath = async () => {
    setTraePathLoading(true);
    try {
      const path = await api.getTraePath();
      setTraePath(path);
    } catch {
      try {
        const path = await api.scanTraePath();
        setTraePath(path);
        api.setTraePath(path).catch(() => {});
      } catch {
        setTraePath("");
      }
    } finally {
      setTraePathLoading(false);
    }
  };

  // 加载「切换账号当作新设备」开关状态
  const loadSwitchAsNewDevice = async () => {
    try {
      const enabled = await api.getSwitchAsNewDevice();
      setSwitchAsNewDevice(enabled);
    } catch (err: any) {
      console.error("加载「切换账号当作新设备」状态失败:", err);
    }
  };

  useEffect(() => {
    loadTraeApps();
    loadTraeMachineId();
    loadTraeDeviceId();
    loadTraePath();
    loadSwitchAsNewDevice();
  }, []);

  // 切换「切换账号当作新设备」开关（即时保存生效）
  const handleToggleSwitchAsNewDevice = async (enabled: boolean) => {
    setSwitchAsNewDevice(enabled);
    setSwitchSettingSaving(true);
    try {
      await api.setSwitchAsNewDevice(enabled);
      onToast?.("success", enabled ? "已开启：切换账号时清理本地数据" : "已关闭：切换账号时保留本地数据");
    } catch (err: any) {
      setSwitchAsNewDevice(!enabled);
      onToast?.("error", err.message || "保存失败");
    } finally {
      setSwitchSettingSaving(false);
    }
  };

  // 复制 Trae IDE 机器码
  const handleCopyTraeMachineId = async () => {
    try {
      await navigator.clipboard.writeText(traeMachineId);
      onToast?.("success", "机器码已复制到剪贴板");
    } catch {
      onToast?.("error", "复制失败");
    }
  };

  // 复制 Trae 客户端设备 ID
  const handleCopyTraeDeviceId = async () => {
    try {
      await navigator.clipboard.writeText(traeDeviceId);
      onToast?.("success", "设备 ID 已复制到剪贴板");
    } catch {
      onToast?.("error", "复制失败");
    }
  };

  // 清除 Trae IDE 登录状态
  const handleClearTraeLoginState = async () => {
    setConfirmState({
      isOpen: true,
      title: "清除登录状态",
      icon: "🗑️",
      message:
        "确定要清除客户端登录状态吗？\n\n" +
        "这将：\n" +
        "• 重置客户端机器码\n" +
        "• 清除所有登录信息\n" +
        "• 删除本地缓存数据\n\n" +
        "操作后客户端将变成全新安装状态，需要重新登录。\n\n" +
        "请确保客户端已关闭！",
      type: "danger",
      onConfirm: async () => {
        setConfirmState(null);
        setClearingTrae(true);
        try {
          await api.clearTraeLoginState();
          await loadTraeMachineId();
          onToast?.("success", "客户端登录状态已清除，请重新打开客户端登录");
        } catch (err: any) {
          onToast?.("error", err.message || "清除失败");
        } finally {
          setClearingTrae(false);
        }
      },
    });
  };

  // 自动扫描当前客户端的安装路径
  const handleScanTraePath = async () => {
    setScanning(true);
    try {
      const path = await api.scanTraePath();
      setTraePath(path);
      api.setTraePath(path).catch(() => {});
      onToast?.("success", "已找到客户端: " + path);
    } catch (err: any) {
      onToast?.("error", err.message || "未找到客户端，请手动设置路径");
    } finally {
      setScanning(false);
    }
  };

  // 手动设置客户端安装路径
  const handleSetTraePath = async () => {
    try {
      const isMac = navigator.userAgent.toUpperCase().includes("MAC");
      const selected = await open({
        multiple: false,
        ...(isMac
          ? {}
          : { filters: [{ name: "Trae IDE", extensions: ["exe"] }] }),
        title: isMac ? "选择客户端应用程序（.app）" : "选择 Trae.exe 文件"
      });

      if (selected) {
        const path = selected as string;
        await api.setTraePath(path);
        setTraePath(path);
        onToast?.("success", "客户端路径已保存");
      }
    } catch (err: any) {
      onToast?.("error", err.message || "选择文件失败");
    }
  };

  return (
    <>
    <div className="settings-section">
      <h3>客户端</h3>
      <div className="machine-id-card trae-card client-card">
        {/* 分组 A：目标客户端选择 */}
        <div className="client-group">
          <div className="client-group-label">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <rect x="2" y="3" width="20" height="14" rx="2" ry="2"/>
              <line x1="8" y1="21" x2="16" y2="21"/>
              <line x1="12" y1="17" x2="12" y2="21"/>
            </svg>
            <span>目标客户端</span>
          </div>
          <div className="client-group-body">
            <div className="machine-id-actions" style={{ flexWrap: "wrap", gap: "8px", marginBottom: 0 }}>
              {traeApps.map((app) => (
                <button
                  key={app.key}
                  className={`machine-id-btn${app.is_current ? " trae-app-current" : ""}`}
                  onClick={() => handleSwitchApp(app.key)}
                  disabled={!app.installed || app.is_current || switchingApp}
                  title={app.installed ? app.data_dir : "本机未检测到该客户端，请先安装后再切换"}
                >
                  {app.is_current ? "✓ " : ""}
                  {app.display_name}
                  {!app.installed ? "（未安装）" : ""}
                </button>
              ))}
              {traeApps.length === 0 && <span>加载中...</span>}
            </div>
            <div className="machine-id-tip" style={{ marginTop: "12px", marginBottom: 0 }}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <circle cx="12" cy="12" r="10"/>
                <path d="M12 16v-4M12 8h.01"/>
              </svg>
              <span>切换客户端后，账号切换、登录、机器码管理将作用于所选客户端。</span>
            </div>
          </div>
        </div>

        {/* 分组分隔线 */}
        <div className="client-divider" />

        {/* 分组 B：安装路径 */}
        <div className="client-group">
          <div className="client-group-label">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
            </svg>
            <span>安装路径</span>
          </div>
          <div className="client-group-body">
            <div className="machine-id-value" style={{ marginBottom: "10px" }}>
              <code>{traePathLoading ? "扫描中..." : (traePath || "未检测到安装路径，可自动扫描或手动设置")}</code>
            </div>
            <div className="machine-id-actions" style={{ marginBottom: 0 }}>
              <button
                className="machine-id-btn"
                onClick={handleScanTraePath}
                disabled={scanning}
                title="自动扫描"
              >
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <circle cx="11" cy="11" r="8"/>
                  <path d="M21 21l-4.35-4.35"/>
                </svg>
                {scanning ? "扫描中..." : "自动扫描"}
              </button>
              <button
                className="machine-id-btn"
                onClick={handleSetTraePath}
                title="手动设置"
              >
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/>
                  <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/>
                </svg>
                手动设置
              </button>
            </div>
          </div>
        </div>

        {/* 分组分隔线 */}
        <div className="client-divider" />

        {/* 分组 C：机器码 */}
        <div className="client-group">
          <div className="client-group-label">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M12 2L2 7l10 5 10-5-10-5z"/>
              <path d="M2 17l10 5 10-5"/>
              <path d="M2 12l10 5 10-5"/>
            </svg>
            <span>机器码</span>
          </div>
          <div className="client-group-body">
            <div className="machine-id-value" style={{ marginBottom: "10px" }}>
              <code>{traeRefreshing ? "加载中..." : traeMachineId}</code>
            </div>
            <div className="machine-id-actions" style={{ marginBottom: 0 }}>
              <button
                className="machine-id-btn"
                onClick={loadTraeMachineId}
                disabled={traeRefreshing}
                title="刷新"
              >
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <path d="M23 4v6h-6M1 20v-6h6M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/>
                </svg>
                刷新
              </button>
              <button
                className="machine-id-btn"
                onClick={handleCopyTraeMachineId}
                disabled={!traeMachineId || traeRefreshing || traeMachineId === "未找到"}
                title="复制"
              >
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/>
                  <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
                </svg>
                复制
              </button>
              <button
                className="machine-id-btn danger"
                onClick={handleClearTraeLoginState}
                disabled={clearingTrae || traeRefreshing}
                title="清除登录状态"
              >
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <path d="M3 6h18M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
                  <line x1="10" y1="11" x2="10" y2="17"/>
                  <line x1="14" y1="11" x2="14" y2="17"/>
                </svg>
                {clearingTrae ? "清除中..." : "清除登录状态"}
              </button>
            </div>
            <div className="machine-id-tip warning" style={{ marginTop: "12px", marginBottom: 0 }}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/>
                <line x1="12" y1="9" x2="12" y2="13"/>
                <line x1="12" y1="17" x2="12.01" y2="17"/>
              </svg>
              <span>清除登录状态会重置机器码，请先关闭客户端。</span>
            </div>
          </div>
        </div>

        {/* 分组分隔线 */}
        <div className="client-divider" />

        {/* 分组 D：本机真实设备 ID（所有 Trae 系产品共享） */}
        <div className="client-group">
          <div className="client-group-label" title="本机真实设备 ID：所有 Trae 系产品（TraeCode / TraeWork）共享，对应签到请求头的 x-device-id；与产品专属的机器码（machineid）不同">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <rect x="5" y="2" width="14" height="20" rx="2" ry="2"/>
              <line x1="12" y1="18" x2="12.01" y2="18"/>
            </svg>
            <span>设备 ID</span>
          </div>
          <div className="client-group-body">
            <div className="machine-id-value" style={{ marginBottom: "10px" }}>
              <code>{deviceRefreshing ? "加载中..." : traeDeviceId}</code>
            </div>
            <div className="machine-id-actions" style={{ marginBottom: 0 }}>
              <button
                className="machine-id-btn"
                onClick={loadTraeDeviceId}
                disabled={deviceRefreshing}
                title="刷新"
              >
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <path d="M23 4v6h-6M1 20v-6h6M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/>
                </svg>
                刷新
              </button>
              <button
                className="machine-id-btn"
                onClick={handleCopyTraeDeviceId}
                disabled={!traeDeviceId || deviceRefreshing || traeDeviceId === "未找到"}
                title="复制"
              >
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/>
                  <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
                </svg>
                复制
              </button>
            </div>
          </div>
        </div>

        {/* 分组分隔线 */}
        <div className="client-divider" />

        {/* 分组 E：账号切换 */}
        <div className="client-group">
          <div className="client-group-label">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M4 4v6h6M20 20v-6h-6"/>
              <path d="M20 9A6 6 0 0 0 9 5.5L4 10M4 15l5 4.5A6 6 0 0 0 20 15"/>
            </svg>
            <span>账号切换</span>
          </div>
          <div className="client-group-body">
            <div className="machine-id-value" style={{ marginBottom: 0 }}>
              <div className="client-switch-row" style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "16px" }}>
                <div className="machine-id-tip" style={{ margin: 0 }}>
                  <span>
                    切换账号当作新设备登录：
                    <br />
                    开启后，切换账号会清理客户端本地数据（如最近项目历史、UI 状态等），模拟"全新设备"体验；
                    <br />
                    关闭（默认）时仅替换登录身份，保留本地数据，避免项目访问历史丢失。
                  </span>
                </div>
                <label className="toggle" style={{ flexShrink: 0 }}>
                  <input
                    type="checkbox"
                    checked={switchAsNewDevice}
                    disabled={switchSettingSaving}
                    onChange={(e) => handleToggleSwitchAsNewDevice(e.target.checked)}
                  />
                  <span className="toggle-slider"></span>
                </label>
              </div>
            </div>
            <div className="machine-id-tip" style={{ marginTop: "8px", marginBottom: 0 }}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <circle cx="12" cy="12" r="10"/>
                <path d="M12 16v-4M12 8h.01"/>
              </svg>
              <span>{switchSettingSaving ? "保存中..." : "改动即时生效，无需额外保存。"}</span>
            </div>
          </div>
        </div>

        {/* 底部提示（已拆分到「目标客户端」与「机器码」各自分组下） */}

      </div>
    </div>

    {/* 确认弹窗 */}
    {confirmState && (
      <ConfirmModal
        isOpen={confirmState.isOpen}
        title={confirmState.title}
        message={confirmState.message}
        type={confirmState.type}
        icon={confirmState.icon}
        onConfirm={confirmState.onConfirm}
        onCancel={() => setConfirmState(null)}
      />
    )}

    {/* 切换目标客户端全屏 loading 遮罩 */}
    {switchingApp && (
      <div className="global-busy-overlay">
        <div className="global-busy-card">
          <div className="global-busy-spinner" />
          <div className="global-busy-title">正在切换目标客户端</div>
          <div className="global-busy-sub">
            自动切换后端目标 → 扫描新客户端路径 → 同步当前账号<br />
            请勿在此期间进行其他操作
          </div>
        </div>
      </div>
    )}
    </>
  );
}
