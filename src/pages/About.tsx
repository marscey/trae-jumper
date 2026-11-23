import { useState } from "react";
import { hasTauri } from "../api";
import type { UpdateInfo } from "../updater";
import wxQrCode from "../assets/wx.jpg";
import logoImage from "../assets/logo.png";

type UpdateStatus =
  | { state: "idle" }
  | { state: "checking" }
  | { state: "up-to-date" }
  | { state: "available"; version: string }
  | { state: "error"; message: string };

interface AboutProps {
  /** App 层传入的检查更新入口：
   *  - 内部会调用 useAutoUpdate.handleManualCheck()，有新版本时更新 autoUpdate 状态；
   *  - 返回 UpdateInfo（有新版本）或 null（已是最新）；
   *  - App 层会在发现新版本后自动打开 UpdateModal。 */
  onRequestCheckUpdate?: () => Promise<UpdateInfo | null>;
}

export function About({ onRequestCheckUpdate }: AboutProps) {
  const [showImageModal, setShowImageModal] = useState(false);
  const [updateStatus, setUpdateStatus] = useState<UpdateStatus>({ state: "idle" });

  const hasTauriEnv = hasTauri();

  const handleCheckUpdate = async () => {
    if (!onRequestCheckUpdate) {
      setUpdateStatus({
        state: "error",
        message: "更新功能仅在桌面客户端中可用",
      });
      return;
    }
    setUpdateStatus({ state: "checking" });
    try {
      const info = await onRequestCheckUpdate();
      if (!info) {
        setUpdateStatus({ state: "up-to-date" });
        return;
      }
      // 有新版本 → UpdateModal 会由 App 层自动弹出；这里只提示一下
      setUpdateStatus({ state: "available", version: info.version });
    } catch (err) {
      setUpdateStatus({
        state: "error",
        message: err?.toString?.() || String(err),
      });
    }
  };

  const renderUpdateUi = () => {
    if (!hasTauriEnv) {
      return <p className="about-desc">更新功能仅在桌面客户端中可用。</p>;
    }
    return (
      <div className="update-block">
        <button
          className="update-btn"
          onClick={handleCheckUpdate}
          disabled={updateStatus.state === "checking"}
        >
          {updateStatus.state === "checking" ? "检查中..." : "检查更新"}
        </button>

        {updateStatus.state === "up-to-date" && (
          <p className="update-status update-status-success">已是最新版本</p>
        )}
        {updateStatus.state === "available" && (
          <p className="update-status">
            发现新版本 v{updateStatus.version}（弹窗已打开）
          </p>
        )}
        {updateStatus.state === "error" && (
          <p className="update-status update-status-error">
            更新失败：{updateStatus.message}
          </p>
        )}
      </div>
    );
  };

  return (
    <div className="about-page">
      <div className="about-card">
        <div className="about-logo">
          <img src={logoImage} alt="Logo" className="about-logo-image" />
        </div>
        <h3>TraeJumper</h3>
        <p className="about-version">版本 {__APP_VERSION__}</p>
        {renderUpdateUi()}
        <p className="about-desc">
          基于 Tauri 2 + Rust 的 TRAE 系列 IDE 多账号管理工具。在"单活跃 Token 服务端策略"下安全地一键切换客户端账号，内嵌 WebView 无痕登录 + 自动续签让位机制彻底解决互踢问题。
        </p>
      </div>

      <div className="about-section">
        <h3>功能特性</h3>
        <ul className="feature-list">
          <li>🎯 <strong>一键切换客户端账号</strong> — 自动杀进程 → 清除旧登录态 → 写入新 Token → 重启客户端，支持跨客户端冲突检测</li>
          <li>🌐 <strong>内嵌 WebView 登录</strong> — 无痕隔离会话，直接从 WKHTTPCookieStore 捕获完整 HttpOnly cookies 用于后续自动续签</li>
          <li>🔐 <strong>Token 自动续签 + 多客户端让位</strong> — 后端定时任务自动维护 Token 存活；检测到客户端自管理架构时完全让位，避免单活跃 Token 策略下的互踢</li>
          <li>📅 <strong>每日自动签到领积分</strong> — 可配置时间点自动执行，签到虚拟设备档案（x-device-id / device-brand）自动生成与自愈</li>
          <li>📈 <strong>Dashboard 仪表盘</strong> — 积分/配额双模式智能切换，用量饼图、套餐分布、账号进度条预警</li>
          <li>⚙️ <strong>在线自动更新</strong> — Ed25519 签名校验，应用内检查、下载、安装一键完成</li>
          <li>🔀 <strong>多应用变体支持</strong> — Trae CN / TraeWork CN / 国际版，API 端点、安装路径、机器码自动跟随切换</li>
          <li>📋 <strong>数据导入导出</strong> — 一键备份/恢复全部账号数据（含 cookies、Token、签到设备档案）</li>
          <li>📝 <strong>完整日志系统</strong> — stdout/stderr 落盘 + 5MB 自动轮转 + Watchdog 运行中自动重建</li>
        </ul>
      </div>

      <div className="about-section">
        <h3>技术栈</h3>
        <div className="tech-tags">
          <span className="tech-tag">Tauri 2</span>
          <span className="tech-tag">React 19</span>
          <span className="tech-tag">TypeScript</span>
          <span className="tech-tag">Vite 7</span>
          <span className="tech-tag">Rust</span>
          <span className="tech-tag">Tokio</span>
          <span className="tech-tag">Reqwest</span>
          <span className="tech-tag">Recharts 3</span>
          <span className="tech-tag">AES-128-CBC</span>
          <span className="tech-tag">tauri-plugin-updater</span>
        </div>
      </div>

      <div className="about-section">
        <h3>赞赏支持</h3>
        <p className="about-desc">
          如果这个工具对您有帮助，欢迎请作者喝杯咖啡 ☕
        </p>
        <div className="appreciation-container">
          <img
            src={wxQrCode}
            alt="微信赞赏码"
            className="qr-code"
            onClick={() => setShowImageModal(true)}
          />
          <p className="appreciation-text">点击图片放大 · 微信扫码赞赏</p>
        </div>
      </div>

      {/* 图片放大模态框 */}
      {showImageModal && (
        <div className="image-modal-overlay" onClick={() => setShowImageModal(false)}>
          <div className="image-modal-content" onClick={(e) => e.stopPropagation()}>
            <button className="image-modal-close" onClick={() => setShowImageModal(false)}>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" width="24" height="24">
                <line x1="18" y1="6" x2="6" y2="18"/>
                <line x1="6" y1="6" x2="18" y2="18"/>
              </svg>
            </button>
            <img src={wxQrCode} alt="微信赞赏码" className="image-modal-img" />
            <p className="image-modal-text">微信扫码赞赏</p>
          </div>
        </div>
      )}
    </div>
  );
}
