import { useEffect } from "react";
import type { AccountBrief } from "../types";
import type { AddToastFn } from "../hooks/useToast";
import { useSettingsConfig } from "./settings/useSettingsConfig";
import { ClientSection } from "./settings/ClientSection";
import { GeneralSection } from "./settings/GeneralSection";
import { RenewSection } from "./settings/RenewSection";
import { CheckinSection } from "./settings/CheckinSection";
import { DataSection } from "./settings/DataSection";

interface SettingsProps {
  onToast?: AddToastFn;
  onExport?: () => void;
  onImport?: () => void;
  onClearData?: () => void;
  /** 客户端切换后回调，用于全局同步窗口标题等 */
  onClientChange?: (clientName: string) => void;
  /** 账号状态变更后回调（切换客户端并同步当前账号后触发）。参数为同步后的当前账号（null=未检测到已登录已知账号） */
  onAccountsChanged?: (current: AccountBrief | null) => void;
}

/**
 * 设置页组装壳：按分区拼装（客户端 / 通用设置 / 续签配置 / 签到配置 / 数据管理）。
 * 共享的签到配置状态由 useSettingsConfig 统一持有。
 */
export function Settings({ onToast, onExport, onImport, onClearData, onClientChange, onAccountsChanged }: SettingsProps) {
  const cfg = useSettingsConfig(onToast);

  // 挂载时加载签到配置（客户端相关信息由 ClientSection 自行加载）
  useEffect(() => {
    cfg.loadCheckinConfig();
  }, []);

  return (
    <div className="settings-page">
      <GeneralSection cfg={cfg} />

      <ClientSection
        onToast={onToast}
        onClientChange={onClientChange}
        onAccountsChanged={onAccountsChanged}
      />

      <RenewSection cfg={cfg} />

      <CheckinSection cfg={cfg} onToast={onToast} />

      <DataSection onExport={onExport} onImport={onImport} onClearData={onClearData} />
    </div>
  );
}
