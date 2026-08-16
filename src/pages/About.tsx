import { useState } from "react";
import wxQrCode from "../assets/wx.jpg";
import logoImage from "../assets/logo.png";

export function About() {
  const [showImageModal, setShowImageModal] = useState(false);

  return (
    <div className="about-page">
      <div className="about-card">
        <div className="about-logo">
          <img src={logoImage} alt="Logo" className="about-logo-image" />
        </div>
        <h3>TraeJumper</h3>
        <p className="about-version">版本 1.0.0</p>
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
