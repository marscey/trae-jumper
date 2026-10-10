import type { CheckinStatusResult, CreditSummary, UsageSummary } from "../types";

interface AccountListItemProps {
  account: {
    id: string;
    name: string;
    email: string;
    avatar_url: string;
    plan_type: string;
    created_at: number;
    is_current?: boolean;
    is_client_active?: boolean;
    active_in_clients?: string[];
    has_cookies?: boolean;
    token_expired_at?: string | null;
    login_source?: string;
    login_type?: string;
    checkin_status?: CheckinStatusResult;
  };
  usage: UsageSummary | null;
  credits: CreditSummary | null;
  creditsLoading?: boolean;
  selected: boolean;
  onSelect: (id: string) => void;
  onContextMenu: (e: React.MouseEvent, id: string) => void;
  onViewDetail: (id: string) => void;
  onRefresh?: (id: string) => void;
  refreshing?: boolean;
  onRenewClient?: (id: string) => void;
  renewingClient?: boolean;
}

// 登录来源展示名（用于列表标签）
function sourceLabel(src?: string): string {
  switch (src) {
    case "client_import": return "客户端导入";
    case "webview": return "WebView登录";
    case "cookie": return "Cookie登录";
    case "manual_token": return "手动Token";
    default: return "未分类";
  }
}
// 登录态类型展示名：原生OAuth / 切号注入·来源 / 仅库内·来源
function loginTypeLabel(src?: string, type?: string): string {
  if (type === "native_o_auth") return "原生OAuth";
  if (type === "injected") return `切号注入·${sourceLabel(src)}`;
  return sourceLabel(src);
}
// 登录态大类样式（低饱和淡色区分，不抢状态色）：
// 切号注入=淡蓝 / 原生OAuth=淡紫 / 纯来源=灰
function loginTypeClass(type?: string): string {
  if (type === "injected") return "login-source-tag injected";
  if (type === "native_o_auth") return "login-source-tag native";
  return "login-source-tag";
}

export function AccountListItem({ account, usage, credits, creditsLoading, selected, onSelect, onContextMenu, onViewDetail, onRefresh, refreshing, onRenewClient, renewingClient }: AccountListItemProps) {
  const isCredits = !!credits?.is_credits_billing;

  // 将客户端显示名映射为短标签
  const shortClientName = (name: string): string => {
    if (name.includes("TraeCode")) return "TraeCode";
    if (name.includes("TraeWork") || name.includes("SOLO")) return "TraeWork";
    if (name.includes("国际")) return "Trae国际";
    return name;
  };
  const activeClients = (account.active_in_clients || []).map(shortClientName);
  const currentTagText = activeClients.length > 0 ? `${activeClients.join(" & ")} 当前` : "";

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

  const getTokenStatus = (): "normal" | "expiring" | "expired" | "unknown" | "client-active" => {
    // 客户端登录中的账号：单活跃 Token 下客户端持有有效会话，
    // TraeJumper 侧 Token 失效属预期让位行为，不应显示"已过期"
    if (account.is_client_active) return "client-active";
    if (!account.token_expired_at) return "unknown";
    const expiry = new Date(account.token_expired_at).getTime();
    if (isNaN(expiry)) return "unknown";
    const now = Date.now();
    if (expiry < now) return "expired";
    if (expiry - now < 3600000) return "expiring";
    return "normal";
  };

  // Token 过期的相对时长描述（仅 expiring/expired 时展示，健康账号保持简洁）
  const getTokenExpiryHint = (): string => {
    if (tokenStatus !== "expiring" && tokenStatus !== "expired") return "";
    if (!account.token_expired_at) return "";
    const expiry = new Date(account.token_expired_at).getTime();
    if (isNaN(expiry)) return "";
    const diffMs = expiry - Date.now();
    const absMin = Math.floor(Math.abs(diffMs) / 60000);
    if (absMin < 60) return diffMs >= 0 ? `剩 ${absMin}分钟` : `${absMin}分钟前`;
    const hours = absMin / 60;
    if (hours < 24) return diffMs >= 0 ? `剩 ${hours.toFixed(1)}h` : `${hours.toFixed(1)}h 前`;
    const days = hours / 24;
    return diffMs >= 0 ? `剩 ${days.toFixed(1)}天` : `${days.toFixed(1)}天前`;
  };

  const tokenStatus = getTokenStatus();
  const expiryHint = getTokenExpiryHint();
  const statusText = tokenStatus === "expired" ? "已过期" : tokenStatus === "expiring" ? "即将过期" : tokenStatus === "client-active" ? "客户端登录中" : "正常";
  const expiryTooltip = account.token_expired_at
    ? `过期时间: ${new Date(account.token_expired_at).toLocaleString("zh-CN")}`
    : "无过期时间信息";
  const displayName = account.email || account.name;
  const avatarLetter = (account.email || account.name || "?").charAt(0).toUpperCase();

  return (
    <div
      className={`account-list-item ${selected ? "selected" : ""} ${activeClients.length > 0 && tokenStatus !== "expired" ? "current" : ""} ${tokenStatus === "expired" ? "expired" : ""}`}
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
          {currentTagText && tokenStatus !== "expired" && (
            <span className="current-tag-inline" title={activeClients.join("、")}>
              <span className="current-tag-check">✓</span>
              {currentTagText}
            </span>
          )}
        </span>
        <span className="list-item-sub">Trae 账号</span>
        <span className={loginTypeClass(account.login_type)} title={`登录态类型：${loginTypeLabel(account.login_source, account.login_type)}`}>
          {loginTypeLabel(account.login_source, account.login_type)}
        </span>
      </div>

      <div className="list-item-badges">
        <span className={`plan-badge ${planLabel.toLowerCase() === "free" ? "free" : ""}`}>{planLabel}</span>
        <span className={`status-tag ${tokenStatus === "expired" ? "expired" : tokenStatus === "expiring" ? "expiring" : tokenStatus === "client-active" ? "client-active" : "normal"}`} title={expiryTooltip}>
          <span className="status-dot"></span>
          {statusText}
          {expiryHint && <span className="status-hint"> · {expiryHint}</span>}
        </span>
        {account.checkin_status && account.checkin_status.code === 0 && tokenStatus !== "expired" && (
          account.checkin_status.checked_in ? (
            <span className="checkin-tag checked" title={`今日已签到 +${account.checkin_status.credits || 200}`}>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" width="10" height="10">
                <path d="M20 6L9 17l-5-5"/>
              </svg>
              已签到
            </span>
          ) : (
            <span className="checkin-tag unchecked" title="今日尚未签到">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" width="10" height="10">
                <rect x="3" y="4" width="18" height="18" rx="2"/>
                <line x1="16" y1="2" x2="16" y2="6"/>
                <line x1="8" y1="2" x2="8" y2="6"/>
                <line x1="3" y1="10" x2="21" y2="10"/>
              </svg>
              待签到
            </span>
          )
        )}
      </div>

      <div className="list-item-usage">
        {creditsLoading && !credits && !usage ? (
          // 积分数据加载占位，避免显示成"无数据/0%"
          <div className="usage-loading-placeholder compact">
            <div className="usage-loading-shimmer" />
          </div>
        ) : (
          <>
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
          </>
        )}
      </div>

      <div className="list-item-actions">
        {onRefresh && account.has_cookies && !account.is_client_active && (
          <button
            className={`action-btn refresh-btn-list ${refreshing ? "loading" : ""}`}
            title="手动续签 Token"
            onClick={(e) => { e.stopPropagation(); onRefresh(account.id); }}
            disabled={refreshing}
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M23 4v6h-6"/>
              <path d="M1 20v-6h6"/>
              <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/>
            </svg>
          </button>
        )}
        {onRenewClient && account.is_client_active && account.has_cookies && (
          <button
            className={`action-btn ${renewingClient ? "loading" : ""}`}
            title="续签并写回客户端（手动测试：立即 GetUserToken 续签并写入客户端存储）"
            onClick={(e) => { e.stopPropagation(); onRenewClient(account.id); }}
            disabled={renewingClient}
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M23 4v6h-6"/>
              <path d="M1 20v-6h6"/>
              <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/>
              <path d="M3 12h18" />
              <path d="M12 3v18" />
            </svg>
          </button>
        )}
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
