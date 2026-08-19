import type { CreditSummary, UsageSummary } from "../types";

interface AccountCardProps {
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

export function AccountCard({ account, usage, credits, selected, onSelect, onContextMenu, onViewDetail }: AccountCardProps) {
  const isCredits = !!credits?.is_credits_billing;

  const formatCredits = (v: number) => {
    const n = Number.isFinite(v) ? v : 0;
    return n.toLocaleString("zh-CN", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  };

  const formatDate = (timestamp: number) => {
    if (!timestamp) return "-";
    const d = new Date(timestamp * 1000);
    return `${d.getFullYear()}/${d.getMonth() + 1}/${d.getDate()}`;
  };

  const formatCreatedDate = (timestamp: number) => {
    if (!timestamp) return "-";
    const d = new Date(timestamp * 1000);
    return `${d.getFullYear()}/${d.getMonth() + 1}/${d.getDate()}`;
  };

  // 计算日期距今天数（负值表示已过期）
  const getDaysLeft = (timestamp: number): number => {
    if (!timestamp) return Number.MAX_SAFE_INTEGER; // 永久/未知视为安全
    const now = Date.now();
    const diff = (timestamp * 1000) - now;
    return Math.ceil(diff / (24 * 60 * 60 * 1000));
  };

  // 根据剩余天数返回颜色类名
  // 已过期 (<0) / 紧急 (0-3) / 临近 (4-7) / 安全 (>7 或 永久)
  const getExpiryClass = (timestamp: number): string => {
    const days = getDaysLeft(timestamp);
    if (days < 0) return "expiry-expired";
    if (days <= 3) return "expiry-urgent";
    if (days <= 7) return "expiry-near";
    return "expiry-safe";
  };

  const getUsageColor = (pct: number) => {
    if (pct >= 80) return "var(--danger)";
    if (pct >= 50) return "var(--warning)";
    return "var(--success)";
  };

  const tokenStatus = ((): "normal" | "expiring" | "expired" | "unknown" => {
    if (!account.token_expired_at) return "unknown";
    const expiry = new Date(account.token_expired_at).getTime();
    if (isNaN(expiry)) return "unknown";
    const now = Date.now();
    if (expiry < now) return "expired";
    if (expiry - now < 3600000) return "expiring";
    return "normal";
  })();

  const statusText = tokenStatus === "expired" ? "已过期" : tokenStatus === "expiring" ? "即将过期" : "正常";
  const statusClass = tokenStatus === "expired" ? "expired" : tokenStatus === "expiring" ? "expiring" : "normal";

  const planLabel = (() => {
    const raw = isCredits ? credits?.plan_name || "Credits" : usage?.plan_type || account.plan_type || "Free";
    if (raw && String(raw).toLowerCase() === "free") return "免费";
    return raw;
  })();

  const { totalUsed, totalLimit, totalLeft, usagePercent } = isCredits
    ? (() => {
        const used = (credits!.general?.used ?? 0) + (credits!.work_exclusive?.used ?? 0);
        const limit = (credits!.general?.total_limit ?? 0) + (credits!.work_exclusive?.total_limit ?? 0);
        const left = (credits!.general?.left ?? 0) + (credits!.work_exclusive?.left ?? 0);
        const pct = limit > 0 ? Math.round((used / limit) * 100) : 0;
        return { totalUsed: used, totalLimit: limit, totalLeft: left, usagePercent: pct };
      })()
    : (() => {
        const used = usage ? usage.fast_request_used + usage.extra_fast_request_used : 0;
        const limit = usage ? usage.fast_request_limit + usage.extra_fast_request_limit : 0;
        const left = usage ? usage.fast_request_left + usage.extra_fast_request_left : 0;
        const pct = limit > 0 ? Math.round((used / limit) * 100) : 0;
        return { totalUsed: used, totalLimit: limit, totalLeft: left, usagePercent: pct };
      })();

  // 计算所有积分项的到期信息
  const expiryInfo = (() => {
    if (!isCredits || !credits) return null;

    const entries: Array<{ time: number; left: number }> = [];

    // 通用积分
    if (credits.general?.nearest_expire_time) {
      entries.push({
        time: credits.general.nearest_expire_time,
        left: credits.general.left ?? 0,
      });
    }
    // Work 专属积分
    if (credits.work_exclusive?.nearest_expire_time) {
      entries.push({
        time: credits.work_exclusive.nearest_expire_time,
        left: credits.work_exclusive.left ?? 0,
      });
    }
    // 套餐到期
    if (credits.plan_expire_time) {
      entries.push({
        time: credits.plan_expire_time,
        left: credits.total_available ?? 0,
      });
    }
    // 奖励积分条目
    for (const e of credits.reward_entries || []) {
      if (e.expire_time) {
        entries.push({
          time: e.expire_time,
          left: Math.max(0, (e.total ?? 0) - (e.used ?? 0)),
        });
      }
    }

    if (entries.length === 0) return null;

    // 按时间升序排列
    entries.sort((a, b) => a.time - b.time);

    return {
      nearest: entries[0],         // 最近到期
      last: entries[entries.length - 1], // 最后到期
    };
  })();

  // 旧配额体系的重置时间
  const resetTime = !isCredits ? (usage?.reset_time || 0) : 0;

  const displayName = account.email || account.name;
  const avatarLetter = (account.email || account.name || "?").charAt(0).toUpperCase();
  const barColor = getUsageColor(usagePercent);

  return (
    <div
      className={`account-card ${selected ? "selected" : ""} ${account.is_current ? "current" : ""}`}
      onClick={() => onSelect(account.id)}
      onContextMenu={(e) => onContextMenu(e, account.id)}
    >
      <div className="card-header">
        <div className="card-checkbox" onClick={(e) => e.stopPropagation()}>
          <input
            type="checkbox"
            checked={selected}
            onChange={() => onSelect(account.id)}
          />
        </div>

        <div className="card-avatar">
          {account.avatar_url ? (
            <img src={account.avatar_url} alt={displayName} />
          ) : (
            <div className="avatar-placeholder">{avatarLetter}</div>
          )}
        </div>

        <div className="card-info" onClick={(e) => { e.stopPropagation(); onViewDetail(account.id); }} title="查看详情">
          <div className="card-email">
            {displayName}
            {account.is_current && (
              <span className="current-tag-inline" title="当前激活账号">
                <span className="current-tag-check">✓</span>
                当前
              </span>
            )}
          </div>
          <div className="card-name">Trae 账号</div>
        </div>

        <div className="card-badges">
          <span className={`plan-badge ${planLabel.toLowerCase() === "free" ? "free" : ""}`}>
            {planLabel}
          </span>
          <span className={`status-tag ${statusClass}`}>
            <span className="status-dot"></span>
            {statusText}
          </span>
        </div>
      </div>

      <div className="card-usage">
        <div className="usage-header-row">
          <div className="usage-header-label">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <circle cx="12" cy="12" r="10"/>
              <path d="M12 6v6l4 2"/>
            </svg>
            积分
          </div>
          <div className="usage-header-pct" style={{ color: barColor }}>{usagePercent}%</div>
        </div>
        <div className="usage-bar">
          <div
            className="usage-bar-fill"
            style={{ width: `${Math.min(usagePercent, 100)}%`, background: barColor }}
          />
        </div>
        <div className="usage-bottom-row">
          <div className="usage-left-group">
            <span className="usage-left-primary" style={{ color: barColor }}>
              {formatCredits(totalLeft)}
            </span>
            <span className="usage-divider">/</span>
            <span className="usage-total">{formatCredits(totalLimit)}</span>
          </div>
          <div className="usage-right-used">
            已使用 <strong>{formatCredits(totalUsed)}</strong>
          </div>
        </div>
      </div>

      <div className="card-meta">
        <span className="meta-item">
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <rect x="3" y="4" width="18" height="18" rx="2" ry="2"/>
            <line x1="16" y1="2" x2="16" y2="6"/>
            <line x1="8" y1="2" x2="8" y2="6"/>
            <line x1="3" y1="10" x2="21" y2="10"/>
          </svg>
          添加于 {formatCreatedDate(account.created_at)}
        </span>
        {isCredits && expiryInfo ? (
          <>
            <span className="meta-item-sep">·</span>
            <span className="meta-item">
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <circle cx="12" cy="12" r="10"/>
                <polyline points="12 6 12 12 16 14"/>
              </svg>
              <span className="credit-number">{formatCredits(expiryInfo.nearest.left)}</span>
              积分将于
              <span className={getExpiryClass(expiryInfo.nearest.time)}>
                {formatDate(expiryInfo.nearest.time)}
              </span>
              到期
            </span>
            <span className="meta-item-sep">·</span>
            <span className="meta-item">
              最后到期{" "}
              <span className={getExpiryClass(expiryInfo.last.time)}>
                {formatDate(expiryInfo.last.time)}
              </span>
            </span>
          </>
        ) : !isCredits && resetTime ? (
          <>
            <span className="meta-item-sep">·</span>
            <span className="meta-item">
              重置 {formatDate(resetTime)}
            </span>
          </>
        ) : null}
      </div>
    </div>
  );
}
