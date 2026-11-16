import { useState } from "react";
import * as api from "../../api";
import type { AddToastFn } from "../../hooks/useToast";
import { ConfirmModal } from "../../components/ConfirmModal";
import type { SettingsConfigSlice } from "./useSettingsConfig";

interface ConfirmState {
  isOpen: boolean;
  title: string;
  message: string;
  type: "danger" | "warning" | "info";
  icon?: string;
  onConfirm: () => void;
}

interface CheckinSectionProps {
  cfg: SettingsConfigSlice;
  onToast?: AddToastFn;
}

/** 设置页 · 签到配置分区：设备策略 / 批量延迟 / 自动签到 / 型号池 / 重置设备 */
export function CheckinSection({ cfg, onToast }: CheckinSectionProps) {
  const { checkinConfig, setCheckinConfig, autoRefreshSaving, setAutoRefreshSaving, checkinConfigSaving } = cfg;

  const [confirmState, setConfirmState] = useState<ConfirmState | null>(null);
  // 签到开始时间：编辑态状态机
  const [checkinTimeEditing, setCheckinTimeEditing] = useState(false);
  const [checkinTimeDraft, setCheckinTimeDraft] = useState("");

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
    <>
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
          <div className="setting-desc">连续签到多个账号时的等待时间，已签到账号跳过、不计入延迟</div>
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

      {/* 自动签到 */}
      <div className="setting-item">
        <div className="setting-info">
          <div className="setting-label">自动签到</div>
          <div className="setting-desc">
            每日到达设定时间后，自动为今日尚未签到的账号签到
            {checkinConfig.auto_checkin_enabled && (
              <span style={{ display: "inline-block", marginLeft: "6px", color: "var(--accent-color)" }}>
                · 每天 {checkinConfig.auto_checkin_time} 后执行
              </span>
            )}
          </div>
        </div>
        <label className="toggle">
          <input
            type="checkbox"
            checked={checkinConfig.auto_checkin_enabled}
            disabled={autoRefreshSaving}
            onChange={(e) => cfg.handleToggleAutoCheckin(e.target.checked)}
          />
          <span className="toggle-slider"></span>
        </label>
      </div>

      <div className="setting-item">
        <div className="setting-info">
          <div className="setting-label">签到开始时间</div>
          <div className="setting-desc">晚于该时间点触发自动签到，确保在零点前完成（含 2 分钟缓冲）</div>
        </div>
        {checkinTimeEditing ? (
          // 编辑态：输入框 + 对勾保存 + × 取消
          <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
            <input
              type="time"
              className="setting-select"
              style={{ width: "140px" }}
              value={checkinTimeDraft}
              autoFocus
              onChange={(e) => setCheckinTimeDraft(e.target.value)}
            />
            <button
              className="setting-btn"
              style={{ padding: "8px 10px", minWidth: "36px" }}
              title="保存"
              disabled={!checkinTimeDraft || checkinTimeDraft === checkinConfig.auto_checkin_time}
              onClick={async () => {
                if (!checkinTimeDraft) return;
                setCheckinConfig((prev) => ({ ...prev, auto_checkin_time: checkinTimeDraft }));
                setCheckinTimeEditing(false);
                setAutoRefreshSaving(true);
                try {
                  await api.updateCheckinConfig({ ...checkinConfig, auto_checkin_time: checkinTimeDraft });
                  onToast?.("success", `签到时间已设为 ${checkinTimeDraft}`);
                } catch (err: any) {
                  setCheckinConfig((prev) => ({ ...prev, auto_checkin_time: checkinConfig.auto_checkin_time }));
                  onToast?.("error", err.message || "保存失败");
                } finally {
                  setAutoRefreshSaving(false);
                }
              }}
            >
              ✓
            </button>
            <button
              className="setting-btn"
              style={{ padding: "8px 10px", minWidth: "36px" }}
              title="取消"
              onClick={() => setCheckinTimeEditing(false)}
            >
              ✕
            </button>
          </div>
        ) : (
          // 展示态：显示当前值 + 编辑按钮
          <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
            <span style={{ fontSize: "14px", color: "var(--text-primary)", padding: "8px 4px" }}>
              {checkinConfig.auto_checkin_time}
            </span>
            <button
              className="setting-btn"
              disabled={autoRefreshSaving || !checkinConfig.auto_checkin_enabled}
              onClick={() => {
                setCheckinTimeDraft(checkinConfig.auto_checkin_time);
                setCheckinTimeEditing(true);
              }}
              title="点击修改签到时间"
            >
              编辑
            </button>
          </div>
        )}
      </div>

      {/* 设备型号池 */}
      <div className="setting-item" style={{ flexDirection: "column", alignItems: "flex-start" }}>
        <div className="setting-info" style={{ marginBottom: "8px" }}>
          <div className="setting-label">设备型号池</div>
          <div className="setting-desc">预设的虚拟设备型号，每个账号添加时随机分配</div>
        </div>
        <div style={{ width: "100%", display: "flex", flexDirection: "column", gap: "14px" }}>
          {/* macOS 型号 */}
          <div style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
            <div style={{ fontSize: "12px", color: "var(--text-secondary)", fontWeight: 500 }}>macOS</div>
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
          {/* Windows 型号 */}
          <div style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
            <div style={{ fontSize: "12px", color: "var(--text-secondary)", fontWeight: 500 }}>Windows（主板型号）</div>
            <div className="device-model-pool" style={{ display: "flex", flexWrap: "wrap", gap: "6px" }}>
              {[
                "Z390 GAMING X", "B760M-A GAMING WIFI",
                "B660M MORTAR WIFI", "H610M PLUS",
                "X570 AORUS ELITE",
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
          onClick={cfg.handleSaveCheckinConfig}
          disabled={checkinConfigSaving}
        >
          {checkinConfigSaving ? "保存中..." : "保存签到配置"}
        </button>
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
    </>
  );
}
