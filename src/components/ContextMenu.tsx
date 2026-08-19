import { useEffect, useRef } from "react";

interface ContextMenuProps {
  x: number;
  y: number;
  onClose: () => void;
  onViewDetail: () => void;
  onRefresh: () => void;
  onUpdateToken: () => void;
  onCopyToken: () => void;
  onSwitchAccount: () => void;
  onClaimGift: () => void;
  onDelete: () => void;
  isCurrent?: boolean; // 是否是当前使用的账号
  showClaimGift?: boolean; // 是否显示"获取礼包"（CN/WORK 积分体系下隐藏）
}

export function ContextMenu({
  x,
  y,
  onClose,
  onViewDetail,
  onRefresh,
  onUpdateToken,
  onCopyToken,
  onSwitchAccount,
  onClaimGift,
  onDelete,
  isCurrent = false,
  showClaimGift = true,
}: ContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    // 调整菜单位置，防止超出屏幕
    if (menuRef.current) {
      const menu = menuRef.current;
      const rect = menu.getBoundingClientRect();

      if (rect.right > window.innerWidth) {
        menu.style.left = `${x - rect.width}px`;
      }
      if (rect.bottom > window.innerHeight) {
        menu.style.top = `${y - rect.height}px`;
      }
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
        <div className="context-menu-item" onClick={onRefresh}>
          <span className="icon">🔄</span>
          刷新数据
        </div>
        <div className="context-menu-item" onClick={onUpdateToken}>
          <span className="icon">🔐</span>
          更新 Token
        </div>
        <div className="context-menu-item" onClick={onCopyToken}>
          <span className="icon">🔑</span>
          复制 Token
        </div>
        <div
          className={`context-menu-item ${isCurrent ? "disabled" : ""}`}
          onClick={isCurrent ? undefined : onSwitchAccount}
          title={isCurrent ? "当前已是此账号" : "切换到此账号"}
        >
          <span className="icon">{isCurrent ? "✓" : "🔀"}</span>
          {isCurrent ? "当前使用中" : "切换账号"}
        </div>
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
