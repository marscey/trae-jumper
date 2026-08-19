import { useState, useMemo } from "react";
import type { CreditSummary, UsageSummary, RewardCreditsEntry } from "../types";

interface DetailModalProps {
  isOpen: boolean;
  onClose: () => void;
  account: {
    id: string;
    name: string;
    email: string;
    avatar_url: string;
    plan_type: string;
    cookies?: string;
    jwt_token?: string | null;
  } | null;
  usage: UsageSummary | null;
  credits?: CreditSummary | null;
}

export function DetailModal({ isOpen, onClose, account, usage, credits }: DetailModalProps) {
  // ===== 所有 hooks 必须在组件最顶层调用（在任何条件 return 之前）=====
  const [copiedField, setCopiedField] = useState<string | null>(null);
  const [expandedGroups, setExpandedGroups] = useState<Record<string, boolean>>({});

  // 奖励积分分组 — 仅聚合"签到奖励"，其他奖励（如老用户福利）保持独立
  const groupedRewards = useMemo(() => {
    try {
      if (!credits?.reward_entries?.length) return null;

      const checkinItems: RewardCreditsEntry[] = [];
      const others: RewardCreditsEntry[] = [];

      for (const e of credits.reward_entries) {
        const title = (e.title || "").trim();
        // 仅聚合签到相关条目（标题包含"签到"）
        if (title.includes("签到")) {
          checkinItems.push(e);
        } else {
          others.push(e);
        }
      }

      const result: Array<{
        key: string;
        title: string;
        items: RewardCreditsEntry[];
        isGroup: boolean;
      }> = [];

      // 签到奖励聚合为一组
      if (checkinItems.length > 0) {
        result.push({
          key: "签到奖励",
          title: "签到奖励",
          items: checkinItems,
          isGroup: checkinItems.length > 1,
        });
      }

      // 其他奖励保持独立
      for (const e of others) {
        const key = e.title || "奖励";
        result.push({
          key: `${key}_${Math.random().toString(36).slice(2)}`,
          title: key,
          items: [e],
          isGroup: false,
        });
      }

      return result;
    } catch {
      return null;
    }
  }, [credits]);

  // 未打开或无账号 — 不渲染（hooks 已全部调用）
  if (!isOpen || !account) return null;

  const acct = account;

  // ===== 辅助函数 =====
  const formatDate = (timestamp: number) => {
    if (!timestamp) return "-";
    return new Date(timestamp * 1000).toLocaleString("zh-CN");
  };

  const formatNumber = (num: number) => {
    const v = Number.isFinite(num) ? num : 0;
    return v.toLocaleString("zh-CN", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  };

  const handleCopy = async (text: string, fieldName: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopiedField(fieldName);
      setTimeout(() => setCopiedField(null), 2000);
    } catch (err) {
      console.error("复制失败:", err);
    }
  };

  const toggleGroup = (key: string) => {
    setExpandedGroups((prev) => ({ ...prev, [key]: !prev[key] }));
  };

  // ===== 渲染主函数（JSX 部分用 try/catch 做防御性保护）=====
  try {
    return renderModal();
  } catch (err) {
    console.error("DetailModal render error:", err);
    return (
      <div className="modal-overlay" onClick={onClose}>
        <div className="modal-content detail-modal" onClick={(e) => e.stopPropagation()}>
          <div className="modal-header-fixed">
            <h2>账号详情</h2>
            <button className="modal-close-btn" onClick={onClose}>×</button>
          </div>
          <div className="modal-body-scrollable" style={{ padding: 24, color: "#8a8f9c" }}>
            <p>渲染出错，请刷新页面重试。</p>
            <p style={{ fontSize: 12, marginTop: 8 }}>{String(err)}</p>
          </div>
          <div className="modal-actions-fixed">
            <button onClick={onClose}>关闭</button>
          </div>
        </div>
      </div>
    );
  }

  function renderModal() {
    const isCredits = !!credits?.is_credits_billing;

    const planLabel = (() => {
      const raw = isCredits
        ? credits?.plan_name || "Credits"
        : usage?.plan_type || acct.plan_type || "Free";
      if (raw && String(raw).toLowerCase() === "free") return "免费";
      return raw;
    })();
    const resetOrExpireTime = isCredits
      ? credits?.plan_expire_time || 0
      : usage?.reset_time || 0;
    const resetOrExpireLabel = isCredits ? "有效期至" : "重置时间";

    const displayName = acct.name || acct.email || "未知账号";
    const displayEmail = acct.email || "-";
    const avatarLetter = (acct.email || acct.name || "?").charAt(0).toUpperCase();

    return (
      <div className="modal-overlay" onClick={onClose}>
        <div className="modal-content detail-modal" onClick={(e) => e.stopPropagation()}>
          <div className="modal-header-fixed">
            <h2>账号详情</h2>
            <button className="modal-close-btn" onClick={onClose}>
              <svg viewBox="0 0 24  24" fill="none" stroke="currentColor" strokeWidth="2" width="20" height="20">
                <line x1="18" y1="6" x2="6" y2="18"/>
                <line x1="6" y1="6" x2="18" y2="18"/>
              </svg>
            </button>
          </div>

          <div className="modal-body-scrollable">
            {/* 基本信息 — 卡片化 */}
            <div className="info-card">
              <div className="info-card-header">
                <div className="info-avatar">
                  {acct.avatar_url ? (
                    <img src={acct.avatar_url} alt={displayName} />
                  ) : (
                    <div className="avatar-placeholder">{avatarLetter}</div>
                  )}
                </div>
                <div className="info-card-title-wrap">
                  <div className="info-card-name">{displayName}</div>
                  <div className="info-card-email">{displayEmail}</div>
                </div>
              </div>
              <div className="info-card-grid">
                <div className="info-grid-item">
                  <span className="info-grid-label">套餐类型</span>
                  <span className="info-grid-value">{planLabel}</span>
                </div>
                <div className="info-grid-item">
                  <span className="info-grid-label">{resetOrExpireLabel}</span>
                  <span className="info-grid-value">{resetOrExpireTime ? formatDate(resetOrExpireTime) : "-"}</span>
                </div>
                <div className="info-grid-item">
                  <span className="info-grid-label">用户ID</span>
                  <div className="secret-value-wrap" style={{ gap: 6 }}>
                    <span className="info-grid-value" style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                      {acct.id}
                    </span>
                    <button
                      className="secret-copy-btn"
                      onClick={() => handleCopy(acct.id, "userId")}
                      title="复制用户ID"
                    >
                      {copiedField === "userId" ? "✓" : (
                        <svg viewBox="0 0 24  24" fill="none" stroke="currentColor" strokeWidth="2" width="16" height="16">
                          <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/>
                          <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
                        </svg>
                      )}
                    </button>
                  </div>
                </div>
                <div className="info-grid-item">
                  <span className="info-grid-label">邮箱</span>
                  <div className="secret-value-wrap" style={{ gap: 6 }}>
                    <span className="info-grid-value" style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                      {displayEmail}
                    </span>
                    <button
                      className="secret-copy-btn"
                      onClick={() => handleCopy(displayEmail, "email")}
                      title="复制邮箱"
                    >
                      {copiedField === "email" ? "✓" : (
                        <svg viewBox="0 0 24  24" fill="none" stroke="currentColor" strokeWidth="2" width="16" height="16">
                          <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/>
                          <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
                        </svg>
                      )}
                    </button>
                  </div>
                </div>
              </div>
            </div>

            {/* 积分体系 */}
            {isCredits && credits && (
              <>
                {/* Hero: 总可用积分 + 子类别合并 */}
                <div className="credit-hero">
                  <div className="credit-hero-top">
                    <div className="credit-hero-label">
                      <svg width="16" height="16" viewBox="0 0 24  24" fill="none" stroke="currentColor" strokeWidth="2">
                        <circle cx="12" cy="12" r="10"/>
                        <line x1="12" y1="8" x2="12" y2="12"/>
                        <line x1="12" y1="16" x2="12.01" y2="16"/>
                      </svg>
                      总可用积分
                    </div>
                    <div className="credit-hero-value">
                      <span className="credit-hero-icon">✦</span>
                      {formatNumber(credits.total_available)}
                      <span className="credit-hero-sub-total" style={{ fontSize: 18, fontWeight: 500, marginLeft: 4 }}>
                        / {formatNumber((credits.general?.total_limit ?? 0) + (credits.work_exclusive?.total_limit ?? 0))}
                      </span>
                    </div>
                  </div>
                  <div className="credit-hero-split">
                    <div className="credit-hero-sub">
                      <div className="credit-hero-sub-label">通用积分</div>
                      <div className="credit-hero-sub-value">
                        {formatNumber(credits.general?.left ?? 0)}
                        <span className="credit-hero-sub-total"> / {formatNumber(credits.general?.total_limit ?? 0)}</span>
                      </div>
                    </div>
                    <div className="credit-hero-divider"></div>
                    <div className="credit-hero-sub">
                      <div className="credit-hero-sub-label">Work 专属积分</div>
                      <div className="credit-hero-sub-value">
                        {formatNumber(credits.work_exclusive?.left ?? 0)}
                        <span className="credit-hero-sub-total"> / {formatNumber(credits.work_exclusive?.total_limit ?? 0)}</span>
                      </div>
                    </div>
                  </div>
                </div>

                {/* 奖励积分明细 */}
                {groupedRewards && groupedRewards.length > 0 ? (
                  <div className="credit-reward-section">
                    <h3 className="credit-reward-title">奖励积分</h3>
                    <div className="credit-reward-list">
                      {groupedRewards.map((group) => {
                        if (!group.isGroup) {
                          const e = group.items[0];
                          const left = Math.max(0, (e.total ?? 0) - (e.used ?? 0));
                          const isEmpty = left <= 0;
                          const scopeLabel = e.scope === 'work_exclusive' ? 'Work 专属积分' : '通用积分';
                          const expireStr = e.expire_time ? formatDate(e.expire_time) : '';
                          return (
                            <div key={group.key} className={`credit-reward-item ${isEmpty ? 'is-empty' : ''}`}>
                              <div className="credit-reward-info">
                                <div className="credit-reward-name">{e.title}</div>
                                <div className="credit-reward-meta">
                                  {scopeLabel}
                                  {expireStr ? ` · ${expireStr}到期` : ''}
                                </div>
                              </div>
                              <div className="credit-reward-amount">
                                {formatNumber(left)} / {formatNumber(e.total)}
                              </div>
                            </div>
                          );
                        }
                        const isOpen = !!expandedGroups[group.key];
                        const totalLeft = group.items.reduce((s, e) => s + Math.max(0, (e.total ?? 0) - (e.used ?? 0)), 0);
                        const isAllEmpty = totalLeft <= 0;
                        const firstItem = group.items[0];
                        const scopeLabel = firstItem.scope === 'work_exclusive' ? 'Work 专属积分' : '通用积分';
                        const expireStr = firstItem.expire_time ? formatDate(firstItem.expire_time) : '';
                        return (
                          <div key={group.key} className={`credit-reward-group ${isAllEmpty ? 'is-empty' : ''}`}>
                            <div
                              className="credit-reward-group-header"
                              onClick={() => toggleGroup(group.key)}
                            >
                              <div className="credit-reward-info">
                                <div className="credit-reward-name">{group.title} <span className="credit-reward-count">· 共 {group.items.length} 笔</span></div>
                                <div className="credit-reward-meta">
                                  {scopeLabel}
                                  {expireStr ? ` · ${expireStr}到期` : ''}
                                </div>
                              </div>
                              <div className="credit-reward-amount">
                                {formatNumber(totalLeft)} / {formatNumber(group.items.reduce((s, e) => s + (e.total ?? 0), 0))}
                                <span className={`credit-reward-chevron ${isOpen ? 'open' : ''}`}>
                                  <svg width="14" height="14" viewBox="0 0 24  24" fill="none" stroke="currentColor" strokeWidth="2">
                                    <polyline points="6 9 12 15 18 9"/>
                                  </svg>
                                </span>
                              </div>
                            </div>
                            {isOpen && (
                              <div className="credit-reward-children">
                                {group.items.map((e, idx) => {
                                  const left = Math.max(0, (e.total ?? 0) - (e.used ?? 0));
                                  const childEmpty = left <= 0;
                                  const childExpire = e.expire_time ? formatDate(e.expire_time) : '';
                                  return (
                                    <div key={idx} className={`credit-reward-child ${childEmpty ? 'is-empty' : ''}`}>
                                      <span className="credit-reward-child-name">{e.title}</span>
                                      <span className="credit-reward-child-meta">
                                        {childExpire ? `${childExpire}到期` : ''}
                                      </span>
                                      <span className="credit-reward-child-amount">
                                        {formatNumber(left)} / {formatNumber(e.total)}
                                      </span>
                                    </div>
                                  );
                                })}
                              </div>
                            )}
                          </div>
                        );
                      })}
                    </div>
                  </div>
                ) : null}

                {/* 通用积分详情 */}
                <div className="credit-detail-section">
                  <h3>通用积分明细</h3>
                  <div className="credit-detail-grid">
                    <div className="credit-detail-item">
                      <span className="credit-detail-label">总量</span>
                      <span className="credit-detail-value">{formatNumber(credits.general?.total_limit ?? 0)}</span>
                    </div>
                    <div className="credit-detail-item">
                      <span className="credit-detail-label">已使用</span>
                      <span className="credit-detail-value">{formatNumber(credits.general?.used ?? 0)}</span>
                    </div>
                    <div className="credit-detail-item highlight">
                      <span className="credit-detail-label">剩余</span>
                      <span className="credit-detail-value">{formatNumber(credits.general?.left ?? 0)}</span>
                    </div>
                  </div>
                </div>

                {/* Work 专属积分详情 */}
                <div className="credit-detail-section">
                  <h3>Work 专属积分明细</h3>
                  <div className="credit-detail-grid">
                    <div className="credit-detail-item">
                      <span className="credit-detail-label">总量</span>
                      <span className="credit-detail-value">{formatNumber(credits.work_exclusive?.total_limit ?? 0)}</span>
                    </div>
                    <div className="credit-detail-item">
                      <span className="credit-detail-label">已使用</span>
                      <span className="credit-detail-value">{formatNumber(credits.work_exclusive?.used ?? 0)}</span>
                    </div>
                    <div className="credit-detail-item highlight">
                      <span className="credit-detail-label">剩余</span>
                      <span className="credit-detail-value">{formatNumber(credits.work_exclusive?.left ?? 0)}</span>
                    </div>
                  </div>
                </div>

                {credits.plan_expire_time ? (
                  <div className="credit-plan-footer">
                    套餐 <strong>{credits.plan_name || "Credits"}</strong> · 有效期至 {formatDate(credits.plan_expire_time)}
                  </div>
                ) : null}
              </>
            )}

            {/* Token / Cookies — 移到最下面 */}
            {(acct.jwt_token || acct.cookies) && (
              <div className="secret-card">
                {acct.jwt_token && (
                  <div className="secret-row">
                    <div className="secret-label">
                      <svg width="14" height="14" viewBox="0 0 24  24" fill="none" stroke="currentColor" strokeWidth="2">
                        <rect x="3" y="11" width="18" height="11" rx="2" ry="2"/>
                        <path d="M7 11V7a5 5 0 0 1 10 0v4"/>
                      </svg>
                      Token
                    </div>
                    <div className="secret-value-wrap">
                      <code className="secret-code">{acct.jwt_token}</code>
                      <button
                        className="secret-copy-btn"
                        onClick={() => handleCopy(acct.jwt_token!, "token")}
                        title="复制 Token"
                      >
                        {copiedField === "token" ? "✓" : (
                          <svg viewBox="0 0 24  24" fill="none" stroke="currentColor" strokeWidth="2" width="16" height="16">
                            <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/>
                            <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
                          </svg>
                        )}
                      </button>
                    </div>
                  </div>
                )}
                {acct.cookies && (
                  <div className="secret-row">
                    <div className="secret-label">
                      <svg width="14" height="14" viewBox="0 0 24  24" fill="none" stroke="currentColor" strokeWidth="2">
                        <circle cx="12" cy="12" r="10"/>
                        <path d="M8 14s1.5 2 4 2 4-2 4-2"/>
                        <line x1="9" y1="9" x2="9.01" y2="9"/>
                        <line x1="15" y1="9" x2="15.01" y2="9"/>
                      </svg>
                      Cookies
                    </div>
                    <div className="secret-value-wrap">
                      <code className="secret-code">{acct.cookies}</code>
                      <button
                        className="secret-copy-btn"
                        onClick={() => handleCopy(acct.cookies!, "cookies")}
                        title="复制 Cookies"
                      >
                        {copiedField === "cookies" ? "✓" : (
                          <svg viewBox="0 0 24  24" fill="none" stroke="currentColor" strokeWidth="2" width="16" height="16">
                            <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/>
                            <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
                          </svg>
                        )}
                      </button>
                    </div>
                  </div>
                )}
              </div>
            )}

            {/* 旧配额体系（国际版） */}
            {!isCredits && usage && (
              <>
                <div className="detail-section">
                  <h3>Fast Request</h3>
                  <div className="detail-row">
                    <span className="detail-label">已使用</span>
                    <span className="detail-value">{formatNumber(usage.fast_request_used)}</span>
                  </div>
                  <div className="detail-row">
                    <span className="detail-label">总配额</span>
                    <span className="detail-value">{formatNumber(usage.fast_request_limit)}</span>
                  </div>
                  <div className="detail-row">
                    <span className="detail-label">剩余</span>
                    <span className="detail-value success">{formatNumber(usage.fast_request_left)}</span>
                  </div>
                </div>

                {usage.extra_fast_request_limit > 0 && (
                  <div className="detail-section">
                    <h3>额外礼包 {usage.extra_package_name && `(${usage.extra_package_name})`}</h3>
                    <div className="detail-row">
                      <span className="detail-label">已使用</span>
                      <span className="detail-value">{formatNumber(usage.extra_fast_request_used)}</span>
                    </div>
                    <div className="detail-row">
                      <span className="detail-label">总配额</span>
                      <span className="detail-value">{formatNumber(usage.extra_fast_request_limit)}</span>
                    </div>
                    <div className="detail-row">
                      <span className="detail-label">剩余</span>
                      <span className="detail-value success">{formatNumber(usage.extra_fast_request_left)}</span>
                    </div>
                    <div className="detail-row">
                      <span className="detail-label">过期时间</span>
                      <span className="detail-value">{formatDate(usage.extra_expire_time)}</span>
                    </div>
                  </div>
                )}

                <div className="detail-section">
                  <h3>其他配额</h3>
                  <div className="detail-row">
                    <span className="detail-label">Slow Request</span>
                    <span className="detail-value">
                      {formatNumber(usage.slow_request_used)} / {formatNumber(usage.slow_request_limit)}
                    </span>
                  </div>
                  <div className="detail-row">
                    <span className="detail-label">Advanced Model</span>
                    <span className="detail-value">
                      {formatNumber(usage.advanced_model_used)} / {formatNumber(usage.advanced_model_limit)}
                    </span>
                  </div>
                  <div className="detail-row">
                    <span className="detail-label">Autocomplete</span>
                    <span className="detail-value">
                      {formatNumber(usage.autocomplete_used)} / {formatNumber(usage.autocomplete_limit)}
                    </span>
                  </div>
                </div>
              </>
            )}
          </div>

          <div className="modal-actions-fixed">
            <button onClick={onClose}>关闭</button>
          </div>
        </div>
      </div>
    );
  }
}
