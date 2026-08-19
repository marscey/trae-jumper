import { useMemo } from "react";
import { UsageEvents } from "../components/UsageEvents";
import type {
  AccountBrief,
  CreditSummary,
  UsageSummary,
} from "../types";

// 仪表盘账号简要：AccountBrief + 额外附加的 usage/credits 字段
type DashboardAccount = AccountBrief & {
  usage?: UsageSummary | null;
  credits?: CreditSummary | null;
};

type DashboardProps = {
  accounts: DashboardAccount[];
};

function formatNum(v: number | null | undefined, fractionDigits = 0): string {
  if (v == null || isNaN(v)) return fractionDigits > 0 ? "0.00" : "0";
  return v.toLocaleString("zh-CN", {
    minimumFractionDigits: fractionDigits,
    maximumFractionDigits: fractionDigits,
  });
}

function formatCredits(v: number | null | undefined): string {
  return formatNum(v, 2);
}

function categoryUsed(c: CreditSummary["general"] | null | undefined): number {
  if (!c) return 0;
  return c.used ?? 0;
}
function accountPlanLabel(a: DashboardAccount): string {
  if (a.credits?.is_credits_billing && a.credits.plan_name) return a.credits.plan_name;
  if (a.usage?.plan_type) return a.usage.plan_type;
  return a.plan_type || "Free";
}
function accountAvatarLetter(a: DashboardAccount): string {
  const src = a.name || a.email || a.id.slice(0, 1);
  return src.trim().charAt(0).toUpperCase();
}
function accountUsed(a: DashboardAccount): number {
  if (a.credits?.is_credits_billing) {
    return categoryUsed(a.credits.general) + categoryUsed(a.credits.work_exclusive);
  }
  if (a.usage) return a.usage.fast_request_used + a.usage.extra_fast_request_used;
  return 0;
}
function accountTotal(a: DashboardAccount): number {
  if (a.credits?.is_credits_billing) {
    return (a.credits.general?.total_limit ?? 0) + (a.credits.work_exclusive?.total_limit ?? 0);
  }
  if (a.usage) return a.usage.fast_request_limit + a.usage.extra_fast_request_limit;
  return 0;
}
function accountLeft(a: DashboardAccount): number {
  if (a.credits?.is_credits_billing) {
    return (a.credits.general?.left ?? 0) + (a.credits.work_exclusive?.left ?? 0);
  }
  if (a.usage) return a.usage.fast_request_left + a.usage.extra_fast_request_left;
  return 0;
}

function StatIcon({ type }: { type: "total" | "used" | "left" | "avg" }) {
  switch (type) {
    case "total":
      return (
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round">
          <path d="M3 3h18v4H3zM3 10h18v4H3zM3 17h18v4H3z" />
          <circle cx="7.5" cy="5" r="0.5" fill="currentColor" />
          <circle cx="7.5" cy="12" r="0.5" fill="currentColor" />
          <circle cx="7.5" cy="19" r="0.5" fill="currentColor" />
        </svg>
      );
    case "used":
      return (
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round">
          <polyline points="23 6 13.5 15.5 8.5 10.5 1 18" />
          <polyline points="17 6 23 6 23 12" />
        </svg>
      );
    case "left":
      return (
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round">
          <polyline points="20 6 9 17 4 12" />
        </svg>
      );
    case "avg":
      return (
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round">
          <circle cx="12" cy="12" r="9" />
          <polyline points="12 7 12 12 15 14" />
        </svg>
      );
  }
}

function DonutChart({
  used,
  total,
  usedLabel = "已使用",
  leftLabel = "剩余",
  centerValue,
  centerLabel,
}: {
  used: number;
  total: number;
  usedLabel?: string;
  leftLabel?: string;
  centerValue: string;
  centerLabel: string;
}) {
  const size = 180;
  const stroke = 28;
  const r = (size - stroke) / 2;
  const c = 2 * Math.PI * r;
  const usedPct = total > 0 ? Math.min(1, used / total) : 0;
  const usedDash = c * usedPct;
  return (
    <div className="pie-chart-container" style={{ display: "flex", flexDirection: "column", alignItems: "center" }}>
      <div style={{ position: "relative", width: size, height: size }}>
        <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} aria-label="使用量分布环形图">
          <circle
            cx={size / 2}
            cy={size / 2}
            r={r}
            fill="none"
            stroke="#e5e7eb"
            strokeWidth={stroke}
          />
          <circle
            cx={size / 2}
            cy={size / 2}
            r={r}
            fill="none"
            stroke="#0ea5e9"
            strokeWidth={stroke}
            strokeLinecap="butt"
            strokeDasharray={`${usedDash} ${c}`}
            transform={`rotate(-90 ${size / 2} ${size / 2})`}
            style={{ transition: "stroke-dasharray 0.6s ease" }}
          />
        </svg>
        <div className="pie-center-text">
          <span className="pie-value">{centerValue}</span>
          <span className="pie-label">{centerLabel}</span>
        </div>
      </div>
      <div className="chart-legend">
        <div className="legend-item">
          <span className="legend-dot" style={{ background: "#0ea5e9" }} />
          <span>{usedLabel} ({formatCredits(used)})</span>
        </div>
        <div className="legend-item">
          <span className="legend-dot" style={{ background: "#e5e7eb" }} />
          <span>{leftLabel} ({formatCredits(total - used)})</span>
        </div>
      </div>
    </div>
  );
}

function PlanPie({
  planCount,
  totalAccounts,
}: {
  planCount: Array<{ name: string; count: number; color: string }>;
  totalAccounts: number;
}) {
  const size = 200;
  if (totalAccounts === 0 || planCount.length === 0) {
    return (
      <div className="chart-empty">
        <div className="empty-chart-icon">🥧</div>
        <p>暂无套餐数据</p>
      </div>
    );
  }
  // 水平饼图样式：中心定位，SVG 尺寸同原图（约 260x200，饼在右侧）
  const stroke = 90;
  const cx = size / 2;
  const cy = size / 2;
  const r = (size - stroke) / 2;
  const c = 2 * Math.PI * r;

  let startAngle = -90;
  const slices: React.ReactElement[] = [];
  const labels: React.ReactElement[] = [];

  planCount.forEach((p, i) => {
    const ratio = p.count / totalAccounts;
    const angle = ratio * 360;
    const dash = c * ratio;
    const gap = c - dash;
    slices.push(
      <circle
        key={`s-${i}`}
        cx={cx}
        cy={cy}
        r={r}
        fill="none"
        stroke={p.color}
        strokeWidth={stroke}
        strokeDasharray={`${dash} ${gap}`}
        transform={`rotate(${startAngle} ${cx} ${cy})`}
      />
    );
    const labelAngleDeg = startAngle + angle / 2;
    const labelAngleRad = ((labelAngleDeg) * Math.PI) / 180;
    const lx = cx + (r + 20) * Math.cos(labelAngleRad);
    const ly = cy + (r + 20) * Math.sin(labelAngleRad);
    labels.push(
      <g key={`l-${i}`} transform={`translate(${lx}, ${ly})`}>
        <text
          x={0}
          y={0}
          textAnchor="middle"
          fontSize={13}
          fill="#0f172a"
          fontWeight={600}
        >
          {p.name} {(ratio * 100).toFixed(0)}%
        </text>
      </g>
    );
    startAngle += angle;
  });

  // 若只有一类，简化显示
  if (planCount.length === 1) {
    const only = planCount[0];
    return (
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-around", gap: 12 }}>
        <div style={{ textAlign: "right", color: "#0ea5e9", fontSize: 14, fontWeight: 600 }}>
          {only.name} 100%
        </div>
        <div>
          <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`}>
            <circle
              cx={cx}
              cy={cy}
              r={r}
              fill="none"
              stroke={only.color}
              strokeWidth={stroke}
            />
          </svg>
        </div>
      </div>
    );
  }

  return (
    <svg width={size + 80} height={size + 40} viewBox={`-40 -20 ${size + 80} ${size + 40}`}>
      {slices}
      {labels}
    </svg>
  );
}

const PLAN_COLORS = ["#0ea5e9", "#43e97b", "#f59e0b", "#8b5cf6", "#ef4444", "#06b6d4"];

export function Dashboard({ accounts }: DashboardProps) {
  // === 统计 ===
  const stats = useMemo(() => {
    const totalAccounts = accounts.length;
    let usedSum = 0;
    let totalSum = 0;
    let leftSum = 0;

    const planMap = new Map<string, number>();
    accounts.forEach((a) => {
      usedSum += accountUsed(a);
      totalSum += accountTotal(a);
      leftSum += accountLeft(a);
      const pl = accountPlanLabel(a);
      planMap.set(pl, (planMap.get(pl) ?? 0) + 1);
    });

    const usedPct = totalSum > 0 ? Math.min(100, Math.round((usedSum / totalSum) * 100)) : 0;
    const leftPct = totalSum > 0 ? Math.min(100, 100 - usedPct) : 100;
    const avgPerAccount = totalAccounts > 0 ? leftSum / totalAccounts : 0;

    const planCount = Array.from(planMap.entries())
      .sort((a, b) => b[1] - a[1])
      .map(([name, count], i) => ({ name, count, color: PLAN_COLORS[i % PLAN_COLORS.length] }));

    return {
      totalAccounts,
      usedSum,
      totalSum,
      leftSum,
      usedPct,
      leftPct,
      avgPerAccount,
      planCount,
    };
  }, [accounts]);

  return (
    <div style={{ padding: "24px" }}>
      {/* 四张统计卡 */}
      <div className="stats-row">
        <div className="stat-card gradient-purple">
          <div className="stat-card-content">
            <div className="stat-card-info">
              <div className="stat-card-label">
                {accounts.some((a) => a.credits?.is_credits_billing) ? "总积分" : "总配额"}
              </div>
              <div className="stat-card-value">{formatCredits(stats.totalSum)}</div>
              <div className="stat-card-change">
                {accounts.some((a) => a.credits?.is_credits_billing) ? "通用+Work 积分合计" : "Fast Requests"}
              </div>
            </div>
            <div className="stat-card-icon">
              <StatIcon type="total" />
            </div>
          </div>
        </div>

        <div className="stat-card gradient-blue">
          <div className="stat-card-content">
            <div className="stat-card-info">
              <div className="stat-card-label">已使用</div>
              <div className="stat-card-value">{formatCredits(stats.usedSum)}</div>
              <div className="stat-card-change">{stats.usedPct}% 使用率</div>
            </div>
            <div className="stat-card-icon">
              <StatIcon type="used" />
            </div>
          </div>
        </div>

        <div className="stat-card gradient-green">
          <div className="stat-card-content">
            <div className="stat-card-info">
              <div className="stat-card-label">剩余可用</div>
              <div className="stat-card-value">{formatCredits(stats.leftSum)}</div>
              <div className="stat-card-change">{stats.leftPct}% 剩余</div>
            </div>
            <div className="stat-card-icon">
              <StatIcon type="left" />
            </div>
          </div>
        </div>

        <div className="stat-card gradient-orange">
          <div className="stat-card-content">
            <div className="stat-card-info">
              <div className="stat-card-label">平均使用</div>
              <div className="stat-card-value">{formatCredits(stats.avgPerAccount)}</div>
              <div className="stat-card-change">每账号</div>
            </div>
            <div className="stat-card-icon">
              <StatIcon type="avg" />
            </div>
          </div>
        </div>
      </div>

      {/* 饼图两列 */}
      <div className="charts-grid-2col">
        <div className="chart-card">
          <div className="chart-header">
            <h3>使用量分布</h3>
            <span className="chart-badge">{stats.usedPct}%</span>
          </div>
          <div className="chart-body">
            <DonutChart
              used={stats.usedSum}
              total={stats.totalSum}
              centerValue={formatCredits(stats.leftSum)}
              centerLabel="剩余"
            />
          </div>
        </div>

        <div className="chart-card">
          <div className="chart-header">
            <h3>套餐分布</h3>
          </div>
          <div className="chart-body" style={{ display: "flex", alignItems: "center", justifyContent: "center" }}>
            <PlanPie planCount={stats.planCount} totalAccounts={stats.totalAccounts} />
          </div>
        </div>
      </div>

      {/* 账号使用情况（表格） */}
      <div className="chart-card full-width" style={{ marginBottom: 24 }}>
        <UsageEvents accounts={accounts} />
      </div>

      {/* 账号概览 */}
      {accounts.length > 0 && (
        <div className="accounts-preview">
          <div className="preview-header">
            <h3>账号概览</h3>
            <span className="preview-count">{accounts.length} 个账号</span>
          </div>
          <div className="preview-list">
            {accounts.map((a) => {
              const used = accountUsed(a);
              const total = accountTotal(a);
              const pct = total > 0 ? Math.min(100, Math.round((used / total) * 100)) : 0;
              const color =
                pct >= 80 ? "#ef4444" : pct >= 50 ? "#f59e0b" : "#0ea5e9";
              return (
                <div key={a.id} className="preview-item">
                  <div className="preview-avatar">
                    {a.avatar_url ? (
                      <img
                        src={a.avatar_url}
                        alt=""
                        style={{ width: 40, height: 40, borderRadius: "50%", objectFit: "cover" }}
                        onError={(e) => {
                          (e.currentTarget as HTMLImageElement).style.display = "none";
                        }}
                      />
                    ) : null}
                    {!a.avatar_url && accountAvatarLetter(a)}
                  </div>
                  <div className="preview-info">
                    <span className="preview-name">{a.name || a.email || a.id}</span>
                    <span className="preview-plan">{accountPlanLabel(a)}</span>
                  </div>
                  <div className="preview-usage">
                    <div className="preview-progress">
                      <div
                        className="preview-progress-fill"
                        style={{ width: `${pct}%`, background: color }}
                      />
                    </div>
                    <span className="preview-percent">{pct}%</span>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}
