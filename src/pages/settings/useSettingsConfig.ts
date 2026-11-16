import { useState, useCallback } from "react";
import * as api from "../../api";
import type { CheckinConfig } from "../../types";
import type { AddToastFn } from "../../hooks/useToast";

/** 默认签到配置（真实值在挂载后由 loadCheckinConfig 拉取覆盖） */
const DEFAULT_CHECKIN_CONFIG: CheckinConfig = {
  device_id_strategy: "real_device_prefix",
  status_delay_min: 1,
  status_delay_max: 3,
  claim_delay_min: 20,
  claim_delay_max: 60,
  auto_refresh_enabled: true,
  auto_refresh_interval: 10,
  auto_renew_enabled: true,
  auto_restart_after_refresh: true,
  auto_checkin_enabled: false,
  auto_checkin_time: "22:00",
  log_watchdog_enabled: true,
  log_watchdog_interval: 5,
};

/** useSettingsConfig 的返回签名（供设置页各分区组件 prop 类型复用） */
export interface SettingsConfigSlice {
  checkinConfig: CheckinConfig;
  setCheckinConfig: React.Dispatch<React.SetStateAction<CheckinConfig>>;
  autoRefreshSaving: boolean;
  setAutoRefreshSaving: React.Dispatch<React.SetStateAction<boolean>>;
  checkinConfigSaving: boolean;
  loadCheckinConfig: () => Promise<void>;
  handleToggleAutoRefresh: (enabled: boolean) => Promise<void>;
  handleChangeRefreshInterval: (interval: number) => Promise<void>;
  handleToggleAutoRenew: (enabled: boolean) => Promise<void>;
  handleToggleAutoRestart: (enabled: boolean) => Promise<void>;
  handleToggleLogWatchdog: (enabled: boolean) => Promise<void>;
  handleChangeLogWatchdogInterval: (interval: number) => Promise<void>;
  handleToggleAutoCheckin: (enabled: boolean) => Promise<void>;
  handleSaveCheckinConfig: () => Promise<void>;
}

/**
 * 设置页共享的签到配置状态与即时保存处理器。
 * 各分区（通用设置/续签配置/签到配置）共用同一份配置对象与保存守卫。
 */
export function useSettingsConfig(onToast: AddToastFn | undefined): SettingsConfigSlice {
  const [checkinConfig, setCheckinConfig] = useState<CheckinConfig>(DEFAULT_CHECKIN_CONFIG);
  const [autoRefreshSaving, setAutoRefreshSaving] = useState(false);
  const [checkinConfigSaving, setCheckinConfigSaving] = useState(false);

  // 加载签到配置
  const loadCheckinConfig = useCallback(async () => {
    try {
      const config = await api.getCheckinConfig();
      setCheckinConfig(config);
    } catch (err: any) {
      console.error("加载签到配置失败:", err);
    }
  }, []);

  /** 通用的"乐观更新 → 保存 → 失败回滚"处理器工厂 */
  const optimisticUpdate = useCallback(
    async (
      patch: Partial<CheckinConfig>,
      successMessage: string
    ) => {
      if (autoRefreshSaving) return;
      const next = { ...checkinConfig, ...patch };
      // 先乐观更新 UI，避免开关视觉回弹
      setCheckinConfig(next);
      setAutoRefreshSaving(true);
      try {
        await api.updateCheckinConfig(next);
        onToast?.("success", successMessage);
      } catch (err: any) {
        // 失败回滚：仅把本次修改的字段恢复为修改前的值（保留其他字段的最新状态）
        setCheckinConfig((prev) => {
          const restored = { ...prev } as unknown as Record<string, unknown>;
          const previous = checkinConfig as unknown as Record<string, unknown>;
          for (const key of Object.keys(patch)) {
            restored[key] = previous[key];
          }
          return restored as unknown as CheckinConfig;
        });
        onToast?.("error", err.message || "保存失败");
      } finally {
        setAutoRefreshSaving(false);
      }
    },
    [autoRefreshSaving, checkinConfig, onToast]
  );

  // 切换自动刷新开关（即时保存，与"切换为新设备"风格一致）
  const handleToggleAutoRefresh = useCallback(
    (enabled: boolean) =>
      optimisticUpdate(
        { auto_refresh_enabled: enabled },
        enabled ? "已开启自动刷新" : "已关闭自动刷新"
      ),
    [optimisticUpdate]
  );

  // 修改自动刷新间隔（即时保存）
  const handleChangeRefreshInterval = useCallback(
    (interval: number) =>
      optimisticUpdate(
        { auto_refresh_interval: interval },
        `刷新间隔已设为 ${interval} 分钟`
      ),
    [optimisticUpdate]
  );

  // 切换 Token 自动续签开关（即时保存）
  const handleToggleAutoRenew = useCallback(
    (enabled: boolean) =>
      optimisticUpdate(
        { auto_renew_enabled: enabled },
        enabled ? "已开启 Token 自动续签" : "已关闭 Token 自动续签"
      ),
    [optimisticUpdate]
  );

  // 切换日志 watchdog 开关（即时保存）
  const handleToggleLogWatchdog = useCallback(
    (enabled: boolean) =>
      optimisticUpdate(
        { log_watchdog_enabled: enabled },
        enabled ? "已开启日志自动重建" : "已关闭日志自动重建"
      ),
    [optimisticUpdate]
  );

  // 修改日志 watchdog 检查间隔（即时保存）
  const handleChangeLogWatchdogInterval = useCallback(
    (interval: number) =>
      optimisticUpdate(
        { log_watchdog_interval: interval },
        `日志检查间隔已设为 ${interval} 秒`
      ),
    [optimisticUpdate]
  );

  // 切换自动签到开关（即时保存）
  const handleToggleAutoCheckin = useCallback(
    (enabled: boolean) =>
      optimisticUpdate(
        { auto_checkin_enabled: enabled },
        enabled ? "已开启自动签到" : "已关闭自动签到"
      ),
    [optimisticUpdate]
  );

  // 切换"自动续签后重启 Trae 客户端"开关
  // 与其他开关不同：不乐观更新，保存成功后才更新 UI
  const handleToggleAutoRestart = useCallback(
    async (enabled: boolean) => {
      const next = { ...checkinConfig, auto_restart_after_refresh: enabled };
      api.updateCheckinConfig(next).then(() => {
        setCheckinConfig(next);
        onToast?.("success", enabled ? "已开启自动续签后重启客户端" : "已关闭自动续签后重启客户端");
      }).catch(() => {
        onToast?.("error", "设置失败");
      });
    },
    [checkinConfig, onToast]
  );

  // 保存签到配置
  const handleSaveCheckinConfig = useCallback(async () => {
    setCheckinConfigSaving(true);
    try {
      await api.updateCheckinConfig(checkinConfig);
      onToast?.("success", "签到配置已保存");
    } catch (err: any) {
      onToast?.("error", err.message || "保存失败");
    } finally {
      setCheckinConfigSaving(false);
    }
  }, [checkinConfig, onToast]);

  return {
    checkinConfig,
    setCheckinConfig,
    autoRefreshSaving,
    setAutoRefreshSaving,
    checkinConfigSaving,
    loadCheckinConfig,
    handleToggleAutoRefresh,
    handleChangeRefreshInterval,
    handleToggleAutoRenew,
    handleToggleAutoRestart,
    handleToggleLogWatchdog,
    handleChangeLogWatchdogInterval,
    handleToggleAutoCheckin,
    handleSaveCheckinConfig,
  };
}
