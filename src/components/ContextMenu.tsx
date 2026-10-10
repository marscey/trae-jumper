import { useEffect, useRef } from "react";

interface ContextMenuProps {
  x: number;
  y: number;
  onClose: () => void;
  onViewDetail: () => void;
  onViewUsage: () => void;
  onRefresh: () => void;
  onUpdateToken: () => void;
  onSwitchAccount: () => void;
  onClaimGift: () => void;
  onCheckin: () => void;
  onViewCheckinHeaders: () => void;
  onDelete: () => void;
  isCurrent?: boolean; // 是否是当前使用的账号
  showClaimGift?: boolean; // 是否显示"获取礼包"（CN/WORK 积分体系下隐藏）
}

export function ContextMenu({
  x,
  y,
  onClose,
  onViewDetail,
  onViewUsage,
  onRefresh,
  onUpdateToken,
  onSwitchAccount,
  onClaimGift,
  onCheckin,
  onViewCheckinHeaders,
  onDelete,
  isCurrent = false,
  showClaimGift = true,
}: ContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    // 边界钳制：确保菜单完整显示在视口内（小窗口/边缘点击时）
    if (menuRef.current) {
      const menu = menuRef.current;
      const rect = menu.getBoundingClientRect();
      const margin = 8;

      let left = x;
      let top = y;

      // 右侧溢出 → 向左收
      if (left + rect.width > window.innerWidth - margin) {
        left = window.innerWidth - rect.width - margin;
      }
      // 底部溢出 → 向上收
      if (top + rect.height > window.innerHeight - margin) {
        top = window.innerHeight - rect.height - margin;
      }
      // 钳制最小坐标，避免被顶出视口外
      if (left < margin) left = margin;
      if (top < margin) top = margin;

      menu.style.left = `${left}px`;
      menu.style.top = `${top}px`;
    }
  }, [x, y]);

  return (
    <>
      <div className="context-menu-overlay" onClick={onClose} />
      <div
        ref={menuRef}
        className="context-menu"
        style={{ left: x, top: y }}
      >
        <div className="context-menu-item" onClick={onViewDetail}>
          <span className="icon">👁</span>
          查看详情
        </div>
        <div className="context-menu-item" onClick={onViewUsage}>
          <span className="icon">📊</span>
          账号用量
        </div>
        <div className="context-menu-item" onClick={onRefresh}>
          <span className="icon">🔄</span>
          刷新数据
        </div>
        <div className="context-menu-item" onClick={onUpdateToken}>
          <span className="icon">🔐</span>
          更新 Token
        </div>
        <div
          className={`context-menu-item ${isCurrent ? "disabled" : ""}`}
          onClick={isCurrent ? undefined : onSwitchAccount}
          title={isCurrent ? "当前已是此账号" : "切换到此账号"}
        >
          <span className="icon">{isCurrent ? "✓" : "🔀"}</span>
          {isCurrent ? "当前使用中" : "切换账号"}
        </div>
        <div className="context-menu-item" onClick={onCheckin}>
          <span className="icon">📅</span>
          每日签到
        </div>
        <div className="context-menu-item" onClick={onViewCheckinHeaders}>
          <span className="icon">🧾</span>
          签到请求头
        </div>
        <div className="context-menu-divider" />
        {showClaimGift && (
          <div className="context-menu-item" onClick={onClaimGift}>
            <span className="icon">🎁</span>
            获取礼包
          </div>
        )}
        <div className="context-menu-divider" />
        <div className="context-menu-item danger" onClick={onDelete}>
          <span className="icon">🗑</span>
          删除账号
        </div>
      </div>
    </>
  );
}
