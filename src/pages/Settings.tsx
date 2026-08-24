import { useState, useEffect } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import * as api from "../api";
import type { AccountBrief, CheckinConfig, TraeAppInfo } from "../types";
import { ConfirmModal } from "../components/ConfirmModal";

interface SettingsProps {
  onToast?: (type: "success" | "error" | "warning" | "info", message: string) => void;
  onExport?: () => void;
  onImport?: () => void;
  onClearData?: () => void;
  /** 客户端切换后回调，用于全局同步窗口标题等 */
  onClientChange?: (clientName: string) => void;
  /** 账号状态变更后回调（切换客户端并同步当前账号后触发）。参数为同步后的当前账号（null=未检测到已登录已知账号） */
  onAccountsChanged?: (current: AccountBrief | null) => void;
}

interface ConfirmState {
  isOpen: boolean;
  title: string;
  message: string;
  type: "danger" | "warning" | "info";
  icon?: string;
  onConfirm: () => void;
}

export function Settings({ onToast, onExport, onImport, onClearData, onClientChange, onAccountsChanged }: SettingsProps) {
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

  const [checkinConfig, setCheckinConfig] = useState<CheckinConfig>({
    device_id_strategy: "real_device_prefix",
    status_delay_min: 1,
    status_delay_max: 3,
    claim_delay_min: 20,
    claim_delay_max: 60,
  });
  const [checkinConfigSaving, setCheckinConfigSaving] = useState(false);
  const [switchAsNewDevice, setSwitchAsNewDevice] = useState(false);
  const [switchSettingSaving, setSwitchSettingSaving] = useState(false);
  const [confirmState, setConfirmState] = useState<ConfirmState | null>(null);

  // 加载签到配置
  const loadCheckinConfig = async () => {
    try {
      const config = await api.getCheckinConfig();
      setCheckinConfig(config);
    } catch (err: any) {
      console.error("加载签到配置失败:", err);
    }
  };

  // 保存签到配置
  const handleSaveCheckinConfig = async () => {
    setCheckinConfigSaving(true);
    try {
      await api.updateCheckinConfig(checkinConfig);
      onToast?.("success", "签到配置已保存");
    } catch (err: any) {
      onToast?.("error", err.message || "保存失败");
    } finally {
      setCheckinConfigSaving(false);
    }
  };

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
          setSwitchingApp(false);
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

  useEffect(() => {
    loadTraeApps();
    loadTraeMachineId();
    loadTraeDeviceId();
    loadTraePath();
    loadCheckinConfig();
    loadSwitchAsNewDevice();
  }, []);

  // 加载「切换账号当作新设备」开关状态
  const loadSwitchAsNewDevice = async () => {
    try {
      const enabled = await api.getSwitchAsNewDevice();
      setSwitchAsNewDevice(enabled);
    } catch (err: any) {
      console.error("加载「切换账号当作新设备」状态失败:", err);
    }
  };

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

  // 批量重置所有账号的签到虚拟设备档案（v5 → v4 重新生成）
  const handleResetAllCheckinDevices = async () => {
    setConfirmState({
      isOpen: true,
      title: "重置所有签到设备",
      icon: "🔄",
      message:
        "确定要为所有账号重新分配签到虚拟设备吗？\n\n" +
        "每个账号都会换掉当前的设备型号 / 设备ID / 机器码 / 会话ID，\n" +
        "用于修复早期 v5 机器码导致的 9074 签到失败。\n" +
        "重置后请重新尝试签到。",
      type: "danger",
      onConfirm: async () => {
        setConfirmState(null);
        try {
          const { count } = await api.resetCheckinDevices();
          onToast?.("success", `已重置 ${count} 个账号的签到虚拟设备`);
        } catch (err: any) {
          onToast?.("error", err.message || "重置签到设备失败");
        }
      },
    });
  };

  return (
    <div className="settings-page">
      {/* 客户端（合并目标客户端 + 机器码 + 路径） */}
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

      <div className="settings-section">
        <h3>通用设置</h3>
        <div className="setting-item">
          <div className="setting-info">
            <div className="setting-label">自动刷新</div>
            <div className="setting-desc">定时自动刷新账号使用量数据</div>
          </div>
          <label className="toggle">
            <input type="checkbox" />
            <span className="toggle-slider"></span>
          </label>
        </div>

        <div className="setting-item">
          <div className="setting-info">
            <div className="setting-label">刷新间隔</div>
            <div className="setting-desc">自动刷新的时间间隔（分钟）</div>
          </div>
          <select className="setting-select">
            <option value="5">5 分钟</option>
            <option value="10">10 分钟</option>
            <option value="30">30 分钟</option>
            <option value="60">60 分钟</option>
          </select>
        </div>
      </div>

      {/* 签到配置 */}
      <div className="settings-section">
        <h3>签到配置</h3>
        
        {/* 设备 ID 生成策略 */}
        <div className="setting-item" style={{ flexDirection: "column", alignItems: "stretch", gap: "10px" }}>
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: "16px" }}>
            <div className="setting-info">
              <div className="setting-label">设备 ID 生成策略</div>
              <div className="setting-desc">
                策略一（默认）：读取本机真实设备 ID 前 13 位 + 后 3 位随机，100% 合法
                <br />
                策略二：FNV 哈希映射到安全数值区间，无需本机真实设备即可跨机器一致
              </div>
            </div>
            <select className="setting-select" style={{ flexShrink: 0, minWidth: "260px" }} value={checkinConfig.device_id_strategy} onChange={(e) => setCheckinConfig(prev => ({ ...prev, device_id_strategy: e.target.value as any }))}>
              <option value="real_device_prefix">策略一：真实设备前缀 + 随机后缀</option>
              <option value="safe_range_fnv">策略二：安全区间 FNV 哈希</option>
            </select>
          </div>
          <div className="info-hint" style={{ fontSize: "12px", color: "var(--text-secondary)" }}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" width="14" height="14" style={{ flexShrink: 0 }}>
              <circle cx="12" cy="12" r="10"/>
              <path d="M12 16v-4M12 8h.01"/>
            </svg>
            <span>策略变更仅影响新生成的设备档案。已有账号如需应用新策略，请使用下方「重置所有签到设备」或右键账号 → 重置签到档案。</span>
          </div>
        </div>

        {/* 批量签到延迟 */}
        <div className="setting-item">
          <div className="setting-info">
            <div className="setting-label">批量签到延迟范围</div>
            <div className="setting-desc">每个账号签到之间的等待时间，模拟人类操作节奏</div>
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
            <input
              type="number"
              className="setting-input"
              style={{ width: "60px" }}
              min={5}
              max={300}
              value={checkinConfig.claim_delay_min}
              onChange={(e) => setCheckinConfig(prev => ({ ...prev, claim_delay_min: Math.max(5, Math.min(prev.claim_delay_max, Number(e.target.value))) }))}
            />
            <span>~</span>
            <input
              type="number"
              className="setting-input"
              style={{ width: "60px" }}
              min={5}
              max={300}
              value={checkinConfig.claim_delay_max}
              onChange={(e) => setCheckinConfig(prev => ({ ...prev, claim_delay_max: Math.max(prev.claim_delay_min, Math.min(300, Number(e.target.value))) }))}
            />
            <span>秒</span>
          </div>
        </div>

        {/* 设备型号池 */}
        <div className="setting-item" style={{ flexDirection: "column", alignItems: "flex-start" }}>
          <div className="setting-info" style={{ marginBottom: "8px" }}>
            <div className="setting-label">设备型号池</div>
            <div className="setting-desc">预设的虚拟设备型号，每个账号添加时随机分配</div>
          </div>
          <div className="device-model-pool" style={{ display: "flex", flexWrap: "wrap", gap: "6px" }}>
            {[
              "MacBookAir10,1", "MacBookAir10,2",
              "MacBookPro18,3", "MacBookPro18,4",
              "MacBookPro16,1", "Mac14,2",
              "Mac14,3", "MacBookPro14,3",
            ].map((model) => (
              <span key={model} className="device-model-tag" style={{
                padding: "4px 10px",
                background: "var(--bg-secondary)",
                borderRadius: "6px",
                fontSize: "12px",
                fontFamily: "monospace",
                color: "var(--text-secondary)",
                border: "1px solid var(--border-color)",
              }}>
                {model}
              </span>
            ))}
          </div>
        </div>

        {/* 重置所有签到设备 */}
        <div className="setting-item" style={{ borderTop: "1px solid var(--border-color)", paddingTop: "16px" }}>
          <div className="setting-info">
            <div className="setting-label">重置所有签到设备</div>
            <div className="setting-desc">为所有账号重新分配一套签到虚拟设备（用于修复旧策略生成的 9074 签到失败，或切换策略后应用新策略）</div>
          </div>
          <button className="setting-btn" onClick={handleResetAllCheckinDevices}>重置</button>
        </div>

        {/* 保存按钮 */}
        <div className="setting-item" style={{ border: "none", paddingBottom: 0 }}>
          <div></div>
          <button
            className="setting-btn primary"
            onClick={handleSaveCheckinConfig}
            disabled={checkinConfigSaving}
          >
            {checkinConfigSaving ? "保存中..." : "保存签到配置"}
          </button>
        </div>
      </div>

      <div className="settings-section">
        <h3>数据管理</h3>
        <div className="setting-item">
          <div className="setting-info">
            <div className="setting-label">导出数据</div>
            <div className="setting-desc">导出所有账号数据为 JSON 文件</div>
          </div>
          <button className="setting-btn" onClick={onExport}>导出</button>
        </div>

        <div className="setting-item">
          <div className="setting-info">
            <div className="setting-label">导入数据</div>
            <div className="setting-desc">从 JSON 文件导入账号数据</div>
          </div>
          <button className="setting-btn" onClick={onImport}>导入</button>
        </div>

        <div className="setting-item danger">
          <div className="setting-info">
            <div className="setting-label">清空数据</div>
            <div className="setting-desc">删除所有账号数据（不可恢复）</div>
          </div>
          <button className="setting-btn danger" onClick={onClearData}>清空</button>
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
    </div>
  );
}
