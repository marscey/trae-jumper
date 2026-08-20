import { useState, useEffect } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import * as api from "../api";
import type { TraeAppInfo } from "../types";

interface SettingsProps {
  onToast?: (type: "success" | "error" | "warning" | "info", message: string) => void;
  onExport?: () => void;
  onImport?: () => void;
  onClearData?: () => void;
  /** 客户端切换后回调，用于全局同步窗口标题等 */
  onClientChange?: (clientName: string) => void;
}

export function Settings({ onToast, onExport, onImport, onClearData, onClientChange }: SettingsProps) {
  const [traeApps, setTraeApps] = useState<TraeAppInfo[]>([]);
  const [switchingApp, setSwitchingApp] = useState(false);
  const [traeMachineId, setTraeMachineId] = useState<string>("");
  const [traeRefreshing, setTraeRefreshing] = useState(false);
  const [clearingTrae, setClearingTrae] = useState(false);
  const [traePath, setTraePath] = useState<string>("");
  const [traePathLoading, setTraePathLoading] = useState(false);
  const [scanning, setScanning] = useState(false);

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

  // 切换目标客户端（后端会自动扫描并保存新客户端的安装路径）
  const handleSwitchApp = async (appKey: string) => {
    if (switchingApp) return;
    setSwitchingApp(true);
    try {
      await api.setCurrentTraeApp(appKey);
      await Promise.all([loadTraeApps(), loadTraeMachineId(), loadTraePath()]);
      onToast?.("success", "目标客户端已切换");
    } catch (err: any) {
      onToast?.("error", err.message || "切换失败");
    } finally {
      setSwitchingApp(false);
    }
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
    loadTraePath();
  }, []);

  // 复制 Trae IDE 机器码
  const handleCopyTraeMachineId = async () => {
    try {
      await navigator.clipboard.writeText(traeMachineId);
      onToast?.("success", "机器码已复制到剪贴板");
    } catch {
      onToast?.("error", "复制失败");
    }
  };

  // 清除 Trae IDE 登录状态
  const handleClearTraeLoginState = async () => {
    if (!confirm("确定要清除客户端登录状态吗？\n\n这将：\n• 重置客户端机器码\n• 清除所有登录信息\n• 删除本地缓存数据\n\n操作后客户端将变成全新安装状态，需要重新登录。\n\n请确保客户端已关闭！")) {
      return;
    }

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
            </div>
          </div>

          {/* 底部提示 */}
          <div className="machine-id-tip warning" style={{ marginTop: "16px" }}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/>
              <line x1="12" y1="9" x2="12" y2="13"/>
              <line x1="12" y1="17" x2="12.01" y2="17"/>
            </svg>
            <span>切换客户端后，账号切换、登录、机器码管理将作用于所选客户端。清除登录状态会重置机器码，请先关闭客户端。</span>
          </div>
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
    </div>
  );
}
