import type { SettingsConfigSlice } from "./useSettingsConfig";

interface GeneralSectionProps {
  cfg: SettingsConfigSlice;
}

/** 设置页 · 通用设置分区：自动刷新 / 刷新间隔 / 日志自动重建 / 日志检查间隔 */
export function GeneralSection({ cfg }: GeneralSectionProps) {
  const { checkinConfig, autoRefreshSaving } = cfg;

  return (
    <div className="settings-section">
      <h3>通用设置 <span className="section-badge shared">所有账号</span></h3>
      <div className="setting-item">
        <div className="setting-info">
          <div className="setting-label">自动刷新</div>
          <div className="setting-desc">
            定时自动刷新账号使用量数据
            {checkinConfig.auto_refresh_enabled && (
              <span style={{ display: "inline-block", marginLeft: "6px", color: "var(--accent-color)" }}>
                · 每 {checkinConfig.auto_refresh_interval} 分钟执行
              </span>
            )}
          </div>
        </div>
        <label className="toggle">
          <input
            type="checkbox"
            checked={checkinConfig.auto_refresh_enabled}
            disabled={autoRefreshSaving}
            onChange={(e) => cfg.handleToggleAutoRefresh(e.target.checked)}
          />
          <span className="toggle-slider"></span>
        </label>
      </div>

      <div className="setting-item">
        <div className="setting-info">
          <div className="setting-label">刷新间隔</div>
          <div className="setting-desc">自动刷新的时间间隔（分钟）</div>
        </div>
        <select
          className="setting-select"
          value={String(checkinConfig.auto_refresh_interval)}
          disabled={autoRefreshSaving || !checkinConfig.auto_refresh_enabled}
          onChange={(e) => cfg.handleChangeRefreshInterval(Number(e.target.value))}
        >
          <option value="5">5 分钟</option>
          <option value="10">10 分钟</option>
          <option value="30">30 分钟</option>
          <option value="60">60 分钟</option>
        </select>
      </div>

      <div className="setting-item">
        <div className="setting-info">
          <div className="setting-label">日志自动重建</div>
          <div className="setting-desc">
            日志文件被外部删除/替换时自动重建并继续写入
            {checkinConfig.log_watchdog_enabled && (
              <span style={{ display: "inline-block", marginLeft: "6px", color: "var(--accent-color)" }}>
                · 每 {checkinConfig.log_watchdog_interval} 秒检查
              </span>
            )}
          </div>
        </div>
        <label className="toggle">
          <input
            type="checkbox"
            checked={checkinConfig.log_watchdog_enabled}
            disabled={autoRefreshSaving}
            onChange={(e) => cfg.handleToggleLogWatchdog(e.target.checked)}
          />
          <span className="toggle-slider"></span>
        </label>
      </div>

      <div className="setting-item">
        <div className="setting-info">
          <div className="setting-label">日志检查间隔</div>
          <div className="setting-desc">日志自动重建的检查间隔（秒，最小 2 秒）</div>
        </div>
        <select
          className="setting-select"
          value={String(checkinConfig.log_watchdog_interval)}
          disabled={autoRefreshSaving || !checkinConfig.log_watchdog_enabled}
          onChange={(e) => cfg.handleChangeLogWatchdogInterval(Number(e.target.value))}
        >
          <option value="5">5 秒</option>
          <option value="10">10 秒</option>
          <option value="30">30 秒</option>
          <option value="60">60 秒</option>
        </select>
      </div>
    </div>
  );
}
