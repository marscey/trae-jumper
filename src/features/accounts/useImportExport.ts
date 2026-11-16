import { useState, useCallback } from "react";
import * as api from "../../api";
import type { AccountWithUsage } from "../../types";
import type { AddToastFn } from "../../hooks/useToast";
import type { ConfirmModalState, InfoModalState } from "../../hooks/useAppModals";
import type { SetStateFn } from "./types";

interface UseImportExportParams {
  accounts: AccountWithUsage[];
  loadAccounts: (retries?: number) => Promise<void>;
  addToast: AddToastFn;
  setConfirmModal: SetStateFn<ConfirmModalState | null>;
  setInfoModal: SetStateFn<InfoModalState | null>;
  setSelectedIds: SetStateFn<Set<string>>;
}

/**
 * 账号数据管理：导出说明与导出、导入说明与导入、清空所有数据。
 */
export function useImportExport({
  accounts,
  loadAccounts,
  addToast,
  setConfirmModal,
  setInfoModal,
  setSelectedIds,
}: UseImportExportParams) {
  const [importing, setImporting] = useState(false);

  // 显示导出说明
  const handleShowExportInfo = useCallback(() => {
    if (accounts.length === 0) {
      addToast("warning", "没有账号可以导出");
      return;
    }

    setInfoModal({
      isOpen: true,
      title: "导出账号说明",
      icon: "📤",
      sections: [
        {
          title: "📄 导出格式",
          content: "JSON 文件 (.json)",
          type: "text"
        },
        {
          title: "📁 保存位置",
          content: "浏览器默认下载文件夹\n文件名格式：trae-accounts-YYYY-MM-DD.json",
          type: "text"
        },
        {
          title: "📋 文件内容",
          content: `<ul>
<li>基础信息（用户名、头像、Email 等）</li>
<li>登录来源标记（OAuth / Cookie / 客户端导入 / 手动 Token）</li>
<li>凭证数据（JWT Token、Cookies）</li>
<li>OAuth Native 完整凭证链（Refresh Token + EC P-256 密钥对）</li>
</ul>`,
          type: "list"
        },
        {
          title: "✅ 导出后可以",
          content: `<ul>
<li>备份账号数据</li>
<li>迁移到其他设备</li>
<li>恢复误删的账号</li>
</ul>`,
          type: "list"
        },
        {
          title: "🚨 安全警告",
          content: `<ul>
<li><strong>包含 OAuth EC P-256 签名私钥</strong>（可伪造身份签发新 Token）</li>
<li><strong>包含 Refresh Token</strong>（有效期 180 天）</li>
<li><strong>包含登录凭证</strong>（JWT、Cookies）</li>
<li><strong>不要分享给任何人、不要上传到云盘或 Git</strong></li>
<li>迁移完成后建议立即销毁导出文件</li>
</ul>`,
          type: "list"
        },
        {
          content: `当前将导出 ${accounts.length} 个账号`,
          type: "text"
        }
      ],
      confirmText: "开始导出",
      onConfirm: () => {
        setInfoModal(null);
        handleExportAccounts();
      }
    });
  }, [accounts, addToast, setInfoModal]);

  // 导出账号
  const handleExportAccounts = useCallback(async () => {
    try {
      const data = await api.exportAccounts();
      const blob = new Blob([data], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      const fileName = `trae-accounts-${new Date().toISOString().split("T")[0]}.json`;
      a.download = fileName;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
      addToast("success", `已导出 ${accounts.length} 个账号到下载文件夹：${fileName}`);
    } catch (err: any) {
      addToast("error", err.message || "导出失败");
    }
  }, [accounts, addToast]);

  // 显示导入说明
  const handleShowImportInfo = useCallback(() => {
    setInfoModal({
      isOpen: true,
      title: "导入账号说明",
      icon: "📥",
      sections: [
        {
          title: "📄 文件格式",
          content: "JSON 文件 (.json)",
          type: "text"
        },
        {
          title: "📋 文件结构示例",
          content: `[
  {
    "name": "用户名",
    "email": "邮箱地址",
    "avatar_url": "头像URL",
    "user_id": "Trae 用户ID",
    "tenant_id": "租户ID",
    "region": "CN",
    "plan_type": "Free",
    "jwt_token": "JWT Token 字符串",
    "cookies": "Web Session Cookies",
    "login_source": "webview_oauth",
    "oauth_enabled": true,
    "oauth_refresh_token": "OAuth Refresh Token",
    "oauth_private_key_pem": "EC P-256 私钥 (PEM)",
    "oauth_public_key_pem": "EC P-256 公钥 (PEM)",
    "oauth_device_id": "DeviceID",
    "oauth_machine_id": "MachineID",
    "oauth_host": "https://api.trae.com.cn"
  }
]`,
          type: "code"
        },
        {
          title: "✅ 导入步骤",
          content: `<ul>
<li>确认后选择 JSON 文件</li>
<li>系统自动验证格式并恢复登录来源 + OAuth 凭证</li>
<li>导入所有有效账号（重复账号跳过）</li>
</ul>`,
          type: "list"
        },
        {
          title: "⚠️ 注意事项",
          content: `<ul>
<li>仅支持本应用导出的格式（flat JSON array）</li>
<li>OAuth Native 账号的 EC 私钥会一并导入，保证自管续签能力</li>
<li>旧格式导出文件（无 login_source / oauth_* 字段）会自动按凭证特征推断来源</li>
<li>迁移完成后建议立即销毁导出文件（含敏感凭证）</li>
</ul>`,
          type: "list"
        }
      ],
      confirmText: "选择文件",
      onConfirm: () => {
        setInfoModal(null);
        handleImportAccounts();
      }
    });
  }, [setInfoModal]);

  // 导入账号
  const handleImportAccounts = useCallback(() => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".json";
    input.onchange = async (e) => {
      const file = (e.target as HTMLInputElement).files?.[0];
      if (!file) return;

      try {
        setImporting(true);
        const text = await file.text();
        const count = await api.importAccounts(text);
        addToast("success", `成功导入 ${count} 个账号`);
        await loadAccounts();
      } catch (err: any) {
        addToast("error", err.message || "导入失败");
      } finally {
        setImporting(false);
      }
    };
    input.click();
  }, [addToast, loadAccounts]);

  // 清空所有数据（弹窗确认）
  const handleClearData = useCallback(() => {
    if (accounts.length === 0) {
      addToast("warning", "没有数据可清空");
      return;
    }

    setConfirmModal({
      isOpen: true,
      title: "清空所有数据",
      message: `确定要清空所有账号数据吗？\n\n此操作将删除全部 ${accounts.length} 个账号的数据，包括：\n• 账号信息\n• Token 和 Cookies\n• 使用量统计\n\n此操作不可恢复，请谨慎操作！`,
      type: "danger",
      onConfirm: async () => {
        setConfirmModal(null);
        try {
          const count = await api.clearAllAccounts();
          setSelectedIds(new Set());
          addToast("success", `已清空 ${count} 个账号数据`);
          await loadAccounts();
        } catch (err: any) {
          addToast("error", err.message || "清空数据失败");
        }
      },
    });
  }, [accounts, addToast, setConfirmModal, setSelectedIds, loadAccounts]);

  return {
    importing,
    handleShowExportInfo,
    handleExportAccounts,
    handleShowImportInfo,
    handleImportAccounts,
    handleClearData,
  };
}
