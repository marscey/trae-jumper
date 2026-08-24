import { useState } from "react";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { ask } from "@tauri-apps/plugin-dialog";
import { hasTauri } from "../api";
import wxQrCode from "../assets/wx.jpg";
import logoImage from "../assets/logo.png";

type UpdateStatus =
  | { state: "idle" }
  | { state: "checking" }
  | { state: "up-to-date" }
  | { state: "available"; version: string; date: string }
  | { state: "downloading"; percent: number }
  | { state: "downloaded" }
  | { state: "error"; message: string };

export function About() {
  const [showImageModal, setShowImageModal] = useState(false);
  const [updateStatus, setUpdateStatus] = useState<UpdateStatus>({ state: "idle" });

  const hasTauriEnv = hasTauri();

  const handleCheckUpdate = async () => {
    setUpdateStatus({ state: "checking" });
    try {
      const update = await check();
      if (!update) {
        setUpdateStatus({ state: "up-to-date" });
        return;
      }
      const date = update.date ? new Date(update.date).toLocaleDateString("zh-CN") : "";
      setUpdateStatus({ state: "available", version: update.version, date });

      const confirmed = await ask(
        `检测到新版本 v${update.version}${date ? `（发布于 ${date}）` : ""}，是否现在下载并更新？`,
        { title: "发现新版本", kind: "info" }
      );
      if (!confirmed) {
        setUpdateStatus({ state: "idle" });
        return;
      }

      setUpdateStatus({ state: "downloading", percent: 0 });
      let downloaded = 0;
      let total = 0;
      await update.download((event) => {
        switch (event.event) {
          case "Started":
            total = event.data.contentLength ?? 0;
            break;
          case "Progress":
            downloaded += event.data.chunkLength;
            break;
          case "Finished":
            break;
        }
        if (total > 0) {
          const percent = Math.min(100, Math.round((downloaded / total) * 100));
          setUpdateStatus({ state: "downloading", percent });
        }
      });

      setUpdateStatus({ state: "downloaded" });
      await update.install();
      const restarted = await ask("更新已完成，是否立即重启应用？", {
        title: "更新完成",
        kind: "info",
      });
      if (restarted) {
        await relaunch();
      }
      setUpdateStatus({ state: "idle" });
    } catch (err) {
      setUpdateStatus({ state: "error", message: err?.toString?.() || String(err) });
    }
  };

  const renderUpdateUi = () => {
    if (!hasTauriEnv) {
      return <p className="about-desc">更新功能仅在桌面客户端中可用。</p>;
    }
    return (
      <div className="update-block">
        <button className="update-btn" onClick={handleCheckUpdate} disabled={updateStatus.state === "checking" || updateStatus.state === "downloading"}>
          {updateStatus.state === "checking"
            ? "检查中..."
            : updateStatus.state === "downloading"
            ? `下载中 ${updateStatus.percent}%`
            : "检查更新"}
        </button>

        {updateStatus.state === "up-to-date" && (
          <p className="update-status update-status-success">已是最新版本</p>
        )}
        {updateStatus.state === "available" && (
          <p className="update-status">发现新版本 v{updateStatus.version}</p>
        )}
        {updateStatus.state === "error" && (
          <p className="update-status update-status-error">更新失败：{updateStatus.message}</p>
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
          Trae 账号使用量管理工具，帮助您轻松管理多个 Trae 账号的使用情况。
        </p>
      </div>

      <div className="about-section">
        <h3>功能特性</h3>
        <ul className="feature-list">
          <li>📊 多账号使用量统计</li>
          <li>🔄 实时刷新账号数据</li>
          <li>📋 一键复制账号信息</li>
          <li>🎨 简洁美观的界面</li>
        </ul>
      </div>

      <div className="about-section">
        <h3>技术栈</h3>
        <div className="tech-tags">
          <span className="tech-tag">Tauri</span>
          <span className="tech-tag">React</span>
          <span className="tech-tag">TypeScript</span>
          <span className="tech-tag">Rust</span>
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
