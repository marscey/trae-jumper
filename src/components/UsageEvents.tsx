import { useState, useEffect, useMemo } from 'react';
import type { UsageEvent } from '../types';
import { getUsageEvents } from '../api';

// 开发环境（无 Tauri）检测：避免 invoke 调用阻塞渲染
const hasTauri = (): boolean =>
  typeof window !== 'undefined' &&
  // @ts-ignore
  (typeof window.__TAURI_INTERNALS__ !== 'undefined' || typeof window.__TAURI__ !== 'undefined');

// 详情视图：单账号模式 + 内部控制时间范围
interface SingleAccountUsageEventsProps {
  accountId: string;
  onError?: (error: string) => void;
  accounts?: undefined;
}

// 仪表盘视图：多账号汇总模式 + 内部控制时间范围
interface MultiAccountUsageEventsProps {
  accounts: Array<{ id: string; email?: string; name?: string; events?: UsageEvent[] | null }>;
  accountId?: undefined;
  onError?: (error: string) => void;
}

type UsageEventsProps = SingleAccountUsageEventsProps | MultiAccountUsageEventsProps;

type TimeFilter = 'today' | '7days' | '30days' | 'custom';

function accountLabel(acc: { email?: string; name?: string; id: string }) {
  return acc.email || acc.name || acc.id.slice(0, 8);
}

export function UsageEvents(props: UsageEventsProps) {
  // ============ 单账号（详情）内部状态 ============
  const [events, setEvents] = useState<UsageEvent[]>([]);
  const [loading, setLoading] = useState(false);
  const [timeFilter, setTimeFilter] = useState<TimeFilter>('7days');
  const [startDate, setStartDate] = useState('');
  const [endDate, setEndDate] = useState('');
  const [showDatePicker, setShowDatePicker] = useState(false);
  const [total, setTotal] = useState(0);

  const isMulti = 'accounts' in props && !!props.accounts;

  // 计算时间戳范围（秒）
  const getTimeRange = (filter: TimeFilter): { startTime: number; endTime: number } => {
    const now = new Date();
    const endTime = Math.floor(now.getTime() / 1000);
    let startTime = 0;

    switch (filter) {
      case 'today':
        const todayStart = new Date(now.getFullYear(), now.getMonth(), now.getDate());
        startTime = Math.floor(todayStart.getTime() / 1000);
        break;
      case '7days':
        const sevenDaysAgo = new Date(now);
        sevenDaysAgo.setDate(now.getDate() - 7);
        startTime = Math.floor(sevenDaysAgo.getTime() / 1000);
        break;
      case '30days':
        const thirtyDaysAgo = new Date(now);
        thirtyDaysAgo.setDate(now.getDate() - 30);
        startTime = Math.floor(thirtyDaysAgo.getTime() / 1000);
        break;
      case 'custom':
        if (startDate) {
          startTime = Math.floor(new Date(startDate).getTime() / 1000);
        }
        if (endDate) {
          const customEndDate = new Date(endDate);
          customEndDate.setHours(23, 59, 59, 999);
          return { startTime, endTime: Math.floor(customEndDate.getTime() / 1000) };
        }
        break;
    }

    return { startTime, endTime };
  };

  // 格式化时间戳为可读日期
  const formatTimestamp = (timestamp: number): string => {
    const date = new Date(timestamp * 1000);
    const year = date.getFullYear();
    const month = String(date.getMonth() + 1).padStart(2, '0');
    const day = String(date.getDate()).padStart(2, '0');
    const hours = String(date.getHours()).padStart(2, '0');
    const minutes = String(date.getMinutes()).padStart(2, '0');
    return `${year}/${month}/${day} ${hours}:${minutes}`;
  };

  // 加载使用事件（仅单账号模式使用，且需要 Tauri 环境可用）
  const loadEvents = async () => {
    if (isMulti || !props.accountId) return;
    if (!hasTauri()) {
      setEvents([]);
      setTotal(0);
      return;
    }

    setLoading(true);
    try {
      const { startTime, endTime } = getTimeRange(timeFilter);
      const response = await getUsageEvents(props.accountId, startTime, endTime, 1, 20);

      setEvents(response.user_usage_group_by_sessions || []);
      setTotal(response.total || 0);
    } catch (error) {
      console.error('Failed to load usage events:', error);
      props.onError?.('加载使用事件失败');
      setEvents([]);
      setTotal(0);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (!isMulti) {
      loadEvents();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isMulti ? undefined : props.accountId, timeFilter, startDate, endDate]);

  const handleTimeFilterChange = (filter: TimeFilter) => {
    setTimeFilter(filter);
    if (filter !== 'custom') {
      setShowDatePicker(false);
    }
  };

  const formatDateRange = () => {
    if (timeFilter === 'custom' && startDate && endDate) {
      return `${startDate} - ${endDate}`;
    }
    const { startTime, endTime } = getTimeRange(timeFilter);
    const start = new Date(startTime * 1000).toISOString().split('T')[0];
    const end = new Date(endTime * 1000).toISOString().split('T')[0];
    return `${start} - ${end}`;
  };

  // ============ 多账号模式：合并 events 并按内部 timeFilter 过滤 ============
  const mergedRows = useMemo<Array<UsageEvent & { _accountLabel?: string }>>(() => {
    if (!isMulti) return [];
    const { startTime } = getTimeRange(timeFilter);
    const out: Array<UsageEvent & { _accountLabel?: string }> = [];
    props.accounts.forEach((acc) => {
      (acc.events ?? []).forEach((e) => {
        if (startTime > 0 && e.usage_time < startTime) return;
        out.push({ ...e, _accountLabel: accountLabel(acc) });
      });
    });
    out.sort((a, b) => b.usage_time - a.usage_time);
    return out;
  }, [isMulti, props, timeFilter, startDate, endDate]);

  // 最终渲染数据
  const finalTitle = '账号使用情况';
  const finalRows = isMulti ? mergedRows : events;
  const finalLoading = !isMulti && loading;
  const finalTotal = isMulti ? mergedRows.length : total;

  return (
    <div className={`usage-events ${isMulti ? '' : 'stat-card'}`}>
      <div className="usage-events-header">
        <h2>{finalTitle}</h2>
        <div className="usage-events-filters">
          <div className="time-filter-buttons">
            <button
              className={`filter-btn ${timeFilter === 'today' ? 'active' : ''}`}
              onClick={() => handleTimeFilterChange('today')}
            >
              Today
            </button>
            <button
              className={`filter-btn ${timeFilter === '7days' ? 'active' : ''}`}
              onClick={() => handleTimeFilterChange('7days')}
            >
              7 days
            </button>
            <button
              className={`filter-btn ${timeFilter === '30days' ? 'active' : ''}`}
              onClick={() => handleTimeFilterChange('30days')}
            >
              30 days
            </button>
          </div>
          <button
            className="date-range-btn"
            onClick={() => setShowDatePicker(!showDatePicker)}
          >
            <span>{formatDateRange()}</span>
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
              <path d="M4 6l4 4 4-4" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
            </svg>
          </button>
        </div>
      </div>

      {showDatePicker && (
        <div className="date-picker-panel">
          <div className="date-inputs">
            <input
              type="date"
              value={startDate}
              onChange={(e) => setStartDate(e.target.value)}
              placeholder="开始日期"
            />
            <span>-</span>
            <input
              type="date"
              value={endDate}
              onChange={(e) => setEndDate(e.target.value)}
              placeholder="结束日期"
            />
          </div>
          <button
            className="apply-btn"
            onClick={() => {
              setTimeFilter('custom');
              setShowDatePicker(false);
            }}
          >
            应用
          </button>
        </div>
      )}

      <div className="usage-events-table-container">
        {finalLoading ? (
          <div className="loading-state">加载中...</div>
        ) : finalRows.length === 0 ? (
          <div className="empty-state">
            <p>暂无使用记录</p>
          </div>
        ) : (
          <table className="usage-events-table">
            <thead>
              <tr>
                <th>Time</th>
                {isMulti && <th>账号</th>}
                <th>Mode</th>
                <th>Model</th>
                <th>
                  Bill (USD)
                  <span className="info-icon" title="费用信息">
                    <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
                      <circle cx="7" cy="7" r="6" stroke="currentColor" strokeWidth="1.5"/>
                      <path d="M7 10V7M7 4h.01" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"/>
                    </svg>
                  </span>
                </th>
                <th>Request Cost</th>
                <th>Tokens</th>
              </tr>
            </thead>
            <tbody>
              {finalRows.slice(0, isMulti ? 50 : undefined).map((event) => (
                <tr key={(event as any)._accountLabel ? `${event.session_id}_${(event as any)._accountLabel}` : event.session_id}>
                  <td>{formatTimestamp(event.usage_time)}</td>
                  {isMulti && <td>{(event as any)._accountLabel ?? '-'}</td>}
                  <td>{event.mode || '-'}</td>
                  <td>{event.model_name}</td>
                  <td>{event.cost_money_float > 0 ? `$${event.cost_money_float.toFixed(4)}` : 'N/A'}</td>
                  <td>{event.amount_float}</td>
                  <td>
                    {event.extra_info.input_token + event.extra_info.output_token}
                    <span style={{ fontSize: '12px', color: '#94a3b8', marginLeft: '4px' }}>
                      ({event.extra_info.input_token}↑ {event.extra_info.output_token}↓)
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
      {finalTotal > 0 && (
        <div style={{ marginTop: '12px', fontSize: '14px', color: '#64748b', textAlign: 'right' }}>
          共 {finalTotal} 条记录
        </div>
      )}
    </div>
  );
}
