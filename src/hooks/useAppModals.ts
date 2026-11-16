import { useState, type ReactNode } from "react";
import type { AccountWithUsage } from "../types";

/** 确认弹窗状态 */
export interface ConfirmModalState {
  isOpen: boolean;
  title: string;
  message: string;
  type: "danger" | "warning" | "info";
  icon?: string;
  confirmText?: string;
  onConfirm: () => void;
}

/** 信息展示弹窗的分区 */
export interface InfoModalSection {
  title?: string;
  content: ReactNode;
  type?: "text" | "code" | "list" | "hint";
}

/** 信息展示弹窗状态 */
export interface InfoModalState {
  isOpen: boolean;
  title: string;
  icon: string;
  sections: InfoModalSection[];
  confirmText: string;
  onConfirm: () => void;
  extraText?: string;
  onExtra?: () => void;
}

/** 右键菜单状态 */
export interface ContextMenuState {
  x: number;
  y: number;
  accountId: string;
}

/** 用量明细弹窗状态 */
export interface UsageModalState {
  accountId: string;
  accountName: string;
}

/** 更新 Token 弹窗状态 */
export interface UpdateTokenModalState {
  accountId: string;
  accountName: string;
}

/**
 * App 全局弹窗状态集中管理：确认弹窗 / 信息弹窗 / 右键菜单 /
 * 详情弹窗 / 用量弹窗 / 更新 Token 弹窗。
 */
export function useAppModals() {
  const [confirmModal, setConfirmModal] = useState<ConfirmModalState | null>(null);
  const [infoModal, setInfoModal] = useState<InfoModalState | null>(null);
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
  const [detailAccount, setDetailAccount] = useState<AccountWithUsage | null>(null);
  const [usageModal, setUsageModal] = useState<UsageModalState | null>(null);
  const [updateTokenModal, setUpdateTokenModal] = useState<UpdateTokenModalState | null>(null);

  return {
    confirmModal,
    setConfirmModal,
    infoModal,
    setInfoModal,
    contextMenu,
    setContextMenu,
    detailAccount,
    setDetailAccount,
    usageModal,
    setUsageModal,
    updateTokenModal,
    setUpdateTokenModal,
  };
}
