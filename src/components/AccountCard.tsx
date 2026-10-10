import type { CheckinStatusResult, CreditSummary, UsageSummary } from "../types";

interface AccountCardProps {
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
    last_cookie_renewal_at?: number | null;
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

// 登录来源展示名（用于标签）
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

export function AccountCard({ account, usage, credits, creditsLoading, selected, onSelect, onContextMenu, onViewDetail, onRefresh, refreshing, onRenewClient, renewingClient }: AccountCardProps) {
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

  const formatDate = (timestamp: number) => {
    if (!timestamp) return "-";
    const d = new Date(timestamp * 1000);
    const pad = (n: number) => String(n).padStart(2, "0");
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
  };

  // 仅日期格式：yyyy-MM-dd（用于"最后到期"）
  const formatDateOnly = (timestamp: number) => {
    if (!timestamp) return "-";
    const d = new Date(timestamp * 1000);
    const pad = (n: number) => String(n).padStart(2, "0");
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  };

  // 短格式：MM-dd HH:mm（用于"续签于"）
  const formatShort = (timestamp: number) => {
    if (!timestamp) return "-";
    const d = new Date(timestamp * 1000);
    const pad = (n: number) => String(n).padStart(2, "0");
    return `${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
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

  // token 专用颜色：token 时效为 8 小时（cookies 续签签发），
  // 按小时判断——剩余不足 3 小时才变红，其余绿色
  const getTokenExpiryClass = (timestamp: number): string => {
    if (!timestamp) return "";
    const hoursLeft = (timestamp * 1000 - Date.now()) / (60 * 60 * 1000);
    if (hoursLeft < 0) return "expiry-expired";
    if (hoursLeft < 3) return "expiry-urgent";
    return "expiry-safe";
  };

  const getUsageColor = (pct: number) => {
    if (pct >= 80) return "var(--danger)";
    if (pct >= 50) return "var(--warning)";
    return "var(--success)";
  };

  const tokenStatus = ((): "normal" | "expiring" | "expired" | "unknown" | "client-active" => {
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
  })();

  const statusText = tokenStatus === "expired" ? "已过期" : tokenStatus === "expiring" ? "即将过期" : tokenStatus === "client-active" ? "客户端登录中" : "正常";
  const statusClass = tokenStatus === "expired" ? "expired" : tokenStatus === "expiring" ? "expiring" : tokenStatus === "client-active" ? "client-active" : "normal";

  // Token 过期的相对时长描述（仅 expiring/expired 时展示，健康账号保持简洁）
  const expiryHint = ((): string => {
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
  })();
  const expiryTooltip = account.token_expired_at
    ? `过期时间: ${new Date(account.token_expired_at).toLocaleString("zh-CN")}`
    : "无过期时间信息";

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

  // 计算最近到期且有剩余的积分明细
  // 注意：只使用 reward_entries 明细 —— 因为只有明细的 expire_time 与剩余(total-used)是一一对应的
  // 大类 general/work_exclusive.nearest_expire_time 指向"该类下最早到期的子笔(可能已用完)"，
  // 而大类 left 是整类总剩余，二者语义不匹配，严禁组合使用，否则会出现
  // "1287.04积分将于 9/2 到期"这种错误（9/2 到期的那笔实际剩余为 0）
  const expiryInfo = (() => {
    if (!isCredits || !credits) return null;

    const entries: Array<{ time: number; left: number }> = [];

    // 奖励积分明细：逐笔到期时间 ↔ 逐笔剩余，语义严格匹配
    for (const e of credits.reward_entries || []) {
      if (e.expire_time) {
        const left = Math.max(0, (e.total ?? 0) - (e.used ?? 0));
        if (left > 0) {
          entries.push({ time: e.expire_time, left });
        }
      }
    }

    if (entries.length === 0) return null;

    // 按到期时间升序排列：最近到期排最前
    entries.sort((a, b) => a.time - b.time);

    return {
      nearest: entries[0],                     // 最近到期（且剩余>0）
      last: entries[entries.length - 1],       // 最远到期（且剩余>0）
    };
  })();

  // 账号总可用积分 = 通用 + Work 专属 + 奖励（用于判断是否"无可用积分"）
  const totalAvailableCredits = isCredits
    ? (totalLeft || 0) + (credits?.reward_total_left ?? 0)
    : 0;

  // 旧配额体系的重置时间
  const resetTime = !isCredits ? (usage?.reset_time || 0) : 0;

  // token/cookie 时间（完整格式 yyyy-MM-dd HH:mm:ss）：
  // token 过期时间 = token_expired_at（RFC3339 字符串，转秒）；
  // 续签时间 = last_cookie_renewal_at（上次用 cookies 续签的秒级时间戳）
  const tokenExpiryTime = account.token_expired_at
    ? Math.floor(new Date(account.token_expired_at).getTime() / 1000)
    : 0;
  const cookieRenewalTime = account.last_cookie_renewal_at || 0;

  const displayName = account.email || account.name;
  const avatarLetter = (account.email || account.name || "?").charAt(0).toUpperCase();
  const barColor = getUsageColor(usagePercent);

  return (
    <div
      className={`account-card ${selected ? "selected" : ""} ${activeClients.length > 0 && tokenStatus !== "expired" ? "current" : ""} ${tokenStatus === "expired" ? "expired" : ""}`}
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
            {currentTagText && tokenStatus !== "expired" && (
              <span className="current-tag-inline" title={activeClients.join("、")}>
                <span className="current-tag-check">✓</span>
                {currentTagText}
              </span>
            )}
          </div>
          <div className="card-name">
            Trae 账号
            <span className={loginTypeClass(account.login_type)} title={`登录态类型：${loginTypeLabel(account.login_source, account.login_type)}`}>
              {loginTypeLabel(account.login_source, account.login_type)}
            </span>
          </div>
        </div>

        <div className="card-badges">
          <span className={`plan-badge ${planLabel.toLowerCase() === "free" ? "free" : ""}`}>
            {planLabel}
          </span>
          <span className={`status-tag ${statusClass}`} title={expiryTooltip}>
            <span className="status-dot"></span>
            {statusText}
            {expiryHint && <span className="status-hint"> · {expiryHint}</span>}
          </span>
          {onRefresh && account.has_cookies && !account.is_client_active && (
            <button
              className={`refresh-btn ${refreshing ? "loading" : ""}`}
              title="手动续签 Token"
              onClick={(e) => { e.stopPropagation(); onRefresh(account.id); }}
              disabled={refreshing}
            >
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
                <path d="M23 4v6h-6"/>
                <path d="M1 20v-6h6"/>
                <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/>
              </svg>
              {refreshing ? "续签中" : "续签"}
            </button>
          )}
          {onRenewClient && account.is_client_active && account.has_cookies && (
            <button
              className={`refresh-btn ${renewingClient ? "loading" : ""}`}
              title="续签并写回客户端（手动测试：立即 GetUserToken 续签并写入客户端存储，不重启客户端）"
              onClick={(e) => { e.stopPropagation(); onRenewClient(account.id); }}
              disabled={renewingClient}
            >
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
                <path d="M23 4v6h-6"/>
                <path d="M1 20v-6h6"/>
                <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/>
              </svg>
              {renewingClient ? "写回中" : "写回客户端"}
            </button>
          )}
          {account.checkin_status && tokenStatus !== "expired" && (
            account.checkin_status.code === 0 ? (
              account.checkin_status.checked_in ? (
                <span className="checkin-tag checked" title="今日已完成签到">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" width="10" height="10">
                    <path d="M20 6L9 17l-5-5"/>
                  </svg>
                  已签到 +{account.checkin_status.credits || 200}
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
            ) : (
              <span className="checkin-tag error" title={`签到状态获取失败：${account.checkin_status.message}`}>
                签到状态未知
              </span>
            )
          )}
        </div>
      </div>

      <div className="card-usage">
        {creditsLoading && !credits && !usage ? (
          // 积分数据加载占位，避免显示成"无数据"
          <div className="usage-loading-placeholder">
            <div className="usage-loading-shimmer" />
            <div className="usage-loading-shimmer short" />
          </div>
        ) : (
          <>
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
          </>
        )}
      </div>

      <div className="card-meta">
        {isCredits && expiryInfo ? (
          totalAvailableCredits > 0 ? (
            <>
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
                <span>
                  {formatDateOnly(expiryInfo.last.time)}
                </span>
              </span>
            </>
          ) : (
            <>
              <span className="meta-item">
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <circle cx="12" cy="12" r="10"/>
                  <path d="M12 6v6l4 2"/>
                </svg>
                <span className="no-available-credits">无可用积分</span>
              </span>
            </>
          )
        ) : !isCredits && resetTime ? (
          <>
            <span className="meta-item">
              重置 {formatDate(resetTime)}
            </span>
          </>
        ) : null}
        <span className="meta-item-sep">|</span>
        <span className="meta-item">
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <rect x="3" y="11" width="18" height="11" rx="2" ry="2"/>
            <path d="M7 11V7a5 5 0 0 1 10 0v4"/>
          </svg>
          token 将于
          <span className={tokenExpiryTime ? getTokenExpiryClass(tokenExpiryTime) : ""}>
            {tokenExpiryTime ? formatDate(tokenExpiryTime) : "未知"}
          </span>
          过期
        </span>
        <span className="meta-item-sep">·</span>
        <span className="meta-item">
          cookie 续签于{" "}
          <span>
            {cookieRenewalTime ? formatShort(cookieRenewalTime) : "尚未续签"}
          </span>
        </span>
      </div>
    </div>
  );
}
