import type { SettingsConfigSlice } from "./useSettingsConfig";

interface RenewSectionProps {
  cfg: SettingsConfigSlice;
}

/** 设置页 · 续签配置分区：Token 自动续签 / 续签后重启客户端 */
export function RenewSection({ cfg }: RenewSectionProps) {
  const { checkinConfig, autoRefreshSaving } = cfg;

  return (
    <div className="settings-section">
      <h3>续签配置</h3>

      <div className="setting-item">
        <div className="setting-info">
          <div className="setting-label">Token 自动续签</div>
          <div className="setting-desc">
            后台每分钟检查 Token 状态，符合条件时自动续签；关闭后 Token 过期需手动续签
          </div>
          {checkinConfig.auto_renew_enabled && (
            <div className="renew-threshold-list" style={{ marginTop: "8px", padding: "10px 12px", background: "var(--bg-secondary)", borderRadius: "8px", fontSize: "12px", color: "var(--text-muted)", lineHeight: "1.8" }}>
              <div style={{ fontWeight: 500, color: "var(--text-secondary)", marginBottom: "4px" }}>触发条件（满足其一即续签）：</div>
              <div>① Token <strong style={{ color: "var(--text-primary)" }}>1 小时内过期</strong>（含已过期）→ 必须续签</div>
              <div>② Cookies session <strong style={{ color: "var(--text-primary)" }}>超过 12 小时未保活</strong> → 维持服务端会话活跃</div>
              <div>③ OAuth Native 账号 Token <strong style={{ color: "var(--text-primary)" }}>2 小时内过期</strong> → 用 Refresh Flow 续签</div>
              <div>④ TJ 注入 + 客户端活跃账号 Token <strong style={{ color: "var(--text-primary)" }}>2 小时内过期</strong> → 续签并写回客户端</div>
              <div style={{ marginTop: "4px", color: "var(--text-muted)" }}>· 客户端原生自管账号（storage 含 icube-dc:*）自动让位，不介入其 Token 生命周期</div>
            </div>
          )}
        </div>
        <label className="toggle">
          <input
            type="checkbox"
            checked={checkinConfig.auto_renew_enabled}
            disabled={autoRefreshSaving}
            onChange={(e) => cfg.handleToggleAutoRenew(e.target.checked)}
          />
          <span className="toggle-slider"></span>
        </label>
      </div>

      {checkinConfig.auto_renew_enabled && (
        <div className="setting-item">
          <div className="setting-info">
            <div className="setting-label">自动续签后重启 Trae 客户端</div>
            <div className="setting-desc">
              续签写回 storage.json 后，自动 kill+relaunch Trae 客户端以加载新 token
            </div>
            {checkinConfig.auto_restart_after_refresh && (
              <div style={{ marginTop: "6px", padding: "8px 12px", background: "var(--bg-secondary)", borderRadius: "8px", fontSize: "12px", color: "var(--text-warning)", lineHeight: "1.7" }}>
                ⚠️ 关闭此选项会导致客户端内存 token 与 storage.json 不一致，Trae 客户端的 verifyStrictTokenInBackground（Mac 锁屏解锁后/每 5 分钟）会检测到并执行 clearUserInfo 登出。仅建议你确认清楚风险后再关闭。
              </div>
            )}
            {!checkinConfig.auto_restart_after_refresh && (
              <div style={{ marginTop: "6px", padding: "8px 12px", background: "rgba(255,165,0,0.1)", borderRadius: "8px", fontSize: "12px", color: "var(--text-warning)", lineHeight: "1.7", border: "1px solid rgba(255,165,0,0.3)" }}>
                ⚠️ 已关闭！客户端正在交互中的会话不会被打断，但 Mac 锁屏解锁后 Trae 客户端可能因 token 不一致被自动登出。
              </div>
            )}
          </div>
          <label className="toggle">
            <input
              type="checkbox"
              checked={checkinConfig.auto_restart_after_refresh}
              disabled={autoRefreshSaving}
              onChange={(e) => cfg.handleToggleAutoRestart(e.target.checked)}
            />
            <span className="toggle-slider"></span>
          </label>
        </div>
      )}
    </div>
  );
}
