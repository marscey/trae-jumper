import type { CreditSummary, UsageSummary } from "../types";

interface AccountListItemProps {
  account: {
    id: string;
    name: string;
    email: string;
    avatar_url: string;
    plan_type: string;
    created_at: number;
    is_current?: boolean;
    token_expired_at?: string | null;
  };
  usage: UsageSummary | null;
  credits: CreditSummary | null;
  selected: boolean;
  onSelect: (id: string) => void;
  onContextMenu: (e: React.MouseEvent, id: string) => void;
  onViewDetail: (id: string) => void;
}

export function AccountListItem({ account, usage, credits, selected, onSelect, onContextMenu, onViewDetail }: AccountListItemProps) {
  const isCredits = !!credits?.is_credits_billing;

  const formatCredits = (v: number) => {
    const n = Number.isFinite(v) ? v : 0;
    return n.toLocaleString("zh-CN", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  };

  const totalUsed = isCredits
    ? (credits!.general?.used ?? 0) + (credits!.work_exclusive?.used ?? 0)
    : usage ? usage.fast_request_used + usage.extra_fast_request_used : 0;
  const totalLimit = isCredits
    ? (credits!.general?.total_limit ?? 0) + (credits!.work_exclusive?.total_limit ?? 0)
    : usage ? usage.fast_request_limit + usage.extra_fast_request_limit : 0;
  const totalLeft = isCredits
    ? (credits!.general?.left ?? 0) + (credits!.work_exclusive?.left ?? 0)
    : usage ? usage.fast_request_left + usage.extra_fast_request_left : 0;
  const usagePercent = totalLimit > 0 ? Math.round((totalUsed / totalLimit) * 100) : 0;

  const planLabel = (() => {
    const raw = isCredits ? credits?.plan_name || "Credits" : usage?.plan_type || account.plan_type || "Free";
    if (raw && String(raw).toLowerCase() === "free") return "免费";
    return raw;
  })();

  const getUsageColor = () => {
    if (usagePercent >= 80) return "var(--danger)";
    if (usagePercent >= 50) return "var(--warning)";
    return "var(--success)";
  };

  const getTokenStatus = (): "normal" | "expiring" | "expired" | "unknown" => {
    if (!account.token_expired_at) return "unknown";
    const expiry = new Date(account.token_expired_at).getTime();
    if (isNaN(expiry)) return "unknown";
    const now = Date.now();
    if (expiry < now) return "expired";
    if (expiry - now < 3600000) return "expiring";
    return "normal";
  };

  const tokenStatus = getTokenStatus();
  const statusText = tokenStatus === "expired" ? "已过期" : tokenStatus === "expiring" ? "即将过期" : "正常";
  const displayName = account.email || account.name;
  const avatarLetter = (account.email || account.name || "?").charAt(0).toUpperCase();

  return (
    <div
      className={`account-list-item ${selected ? "selected" : ""} ${account.is_current ? "current" : ""}`}
      onClick={() => onSelect(account.id)}
      onContextMenu={(e) => onContextMenu(e, account.id)}
    >
      <div className="list-item-checkbox" onClick={(e) => e.stopPropagation()}>
        <input
          type="checkbox"
          checked={selected}
          onChange={() => onSelect(account.id)}
        />
      </div>

      <div className="list-item-avatar">
        {account.avatar_url ? (
          <img src={account.avatar_url} alt={displayName} />
        ) : (
          <div className="avatar-placeholder">{avatarLetter}</div>
        )}
      </div>

      <div className="list-item-info" onClick={(e) => { e.stopPropagation(); onViewDetail(account.id); }} title="查看详情">
        <span className="list-item-email">
          {displayName}
          {account.is_current && (
            <span className="current-tag-inline" title="当前激活账号">
              <span className="current-tag-check">✓</span>
              当前
            </span>
          )}
        </span>
        <span className="list-item-sub">Trae 账号</span>
      </div>

      <div className="list-item-badges">
        <span className={`plan-badge ${planLabel.toLowerCase() === "free" ? "free" : ""}`}>{planLabel}</span>
        <span className={`status-tag ${tokenStatus === "expired" ? "expired" : tokenStatus === "expiring" ? "expiring" : "normal"}`}>
          <span className="status-dot"></span>
          {statusText}
        </span>
      </div>

      <div className="list-item-usage">
        <div className="usage-row-header">
          <span className="usage-row-header-label">积分</span>
          <span className="usage-row-header-pct" style={{ color: getUsageColor() }}>{usagePercent}%</span>
        </div>
        <div className="usage-bar-mini">
          <div
            className="usage-bar-fill-mini"
            style={{ width: `${Math.min(usagePercent, 100)}%`, background: getUsageColor() }}
          />
        </div>
        <div className="usage-row-bottom">
          <div className="usage-row-left">
            <span className="usage-left-primary" style={{ color: getUsageColor() }}>{formatCredits(totalLeft)}</span>
            <span className="usage-divider">/</span>
            <span className="usage-total">{formatCredits(totalLimit)}</span>
          </div>
          <div className="usage-row-right">
            已使用 <strong>{formatCredits(totalUsed)}</strong>
          </div>
        </div>
      </div>

      <div className="list-item-actions">
        <button
          className="action-btn"
          title="更多操作 (右键)"
          onClick={(e) => {
            e.stopPropagation();
            onContextMenu(e, account.id);
          }}
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
            <circle cx="12" cy="5" r="2"/>
            <circle cx="12" cy="12" r="2"/>
            <circle cx="12" cy="19" r="2"/>
          </svg>
        </button>
      </div>
    </div>
  );
}
