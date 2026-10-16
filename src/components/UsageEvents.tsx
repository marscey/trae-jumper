import { useState, useEffect, useCallback } from 'react';
import type { UsageEvent } from '../types';
import { getUsageEvents } from '../api';

// 开发环境（无 Tauri）检测：避免 invoke 调用阻塞渲染
const hasTauri = (): boolean =>
  typeof window !== 'undefined' &&
  // @ts-ignore
  (typeof window.__TAURI_INTERNALS__ !== 'undefined' || typeof window.__TAURI__ !== 'undefined');

interface UsageEventsProps {
  accountId: string;
  onError?: (error: string) => void;
}

type TimeFilter = 'today' | '7days' | '30days' | 'custom';

// product_type 到产品端名称的映射（对齐 Trae 平台）
function productTypeName(product_type_list: number[]): string {
  const t = product_type_list?.[0];
  switch (t) {
    case 1: return 'TraeCode';
    case 2: return 'TraeWork';
    default: return t ? `产品${t}` : '-';
  }
}

export function UsageEvents({ accountId, onError }: UsageEventsProps) {
  // ============ 单账号（详情）内部状态 ============
  const [events, setEvents] = useState<UsageEvent[]>([]);
  const [loading, setLoading] = useState(false);
  const [timeFilter, setTimeFilter] = useState<TimeFilter>('today');
  const [startDate, setStartDate] = useState('');
  const [endDate, setEndDate] = useState('');
  const [showDatePicker, setShowDatePicker] = useState(false);
  const [total, setTotal] = useState(0);
  const [dateError, setDateError] = useState('');

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
  const loadEvents = useCallback(async () => {
    if (!accountId) return;
    if (!hasTauri()) {
      setEvents([]);
      setTotal(0);
      return;
    }

    setLoading(true);
    try {
      const { startTime, endTime } = getTimeRange(timeFilter);
      console.log('[UsageEvents] loadEvents', { filter: timeFilter, startTime, endTime, accountId });
      const response = await getUsageEvents(accountId, startTime, endTime, 1, 20);
      console.log('[UsageEvents] response', { total: response.total, count: response.user_usage_group_by_sessions?.length });

      setEvents(response.user_usage_group_by_sessions || []);
      setTotal(response.total || 0);
    } catch (error) {
      console.error('Failed to load usage events:', error);
      onError?.('加载使用事件失败');
      setEvents([]);
      setTotal(0);
    } finally {
      setLoading(false);
    }
  }, [accountId, timeFilter, startDate, endDate]);

  useEffect(() => {
    loadEvents();
  }, [loadEvents]);

  const handleTimeFilterChange = (filter: TimeFilter) => {
    setDateError('');
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

  // 最终渲染数据
  const finalRows = events;
  const finalLoading = loading;
  const finalTotal = total;

  return (
    <div className="usage-events">
      <div className="usage-events-header">
        <div className="usage-events-filters">
          <div className="time-filter-buttons">
            <button
              className={`filter-btn ${timeFilter === 'today' ? 'active' : ''}`}
              onClick={() => handleTimeFilterChange('today')}
            >
              今天
            </button>
            <button
              className={`filter-btn ${timeFilter === '7days' ? 'active' : ''}`}
              onClick={() => handleTimeFilterChange('7days')}
            >
              7天
            </button>
            <button
              className={`filter-btn ${timeFilter === '30days' ? 'active' : ''}`}
              onClick={() => handleTimeFilterChange('30days')}
            >
              30天
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
              onChange={(e) => { setStartDate(e.target.value); setDateError(''); }}
              placeholder="开始日期"
            />
            <span>-</span>
            <input
              type="date"
              value={endDate}
              onChange={(e) => { setEndDate(e.target.value); setDateError(''); }}
              placeholder="结束日期"
            />
          </div>
          <button
            className="apply-btn"
            onClick={() => {
              setDateError('');
              if (!startDate || !endDate) {
                setDateError('请选择开始和结束日期');
                return;
              }
              if (new Date(startDate) > new Date(endDate)) {
                setDateError('开始日期不能晚于结束日期');
                return;
              }
              setTimeFilter('custom');
              setShowDatePicker(false);
            }}
          >
            应用
          </button>
          {dateError && <div className="date-picker-error">{dateError}</div>}
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
                <th>时间</th>
                <th>使用记录</th>
                <th>模型名称</th>
                <th>产品端</th>
                <th>积分消耗</th>
              </tr>
            </thead>
            <tbody>
              {finalRows.map((event) => (
                <tr key={event.session_id}>
                  <td>{formatTimestamp(event.usage_time)}</td>
                  <td className="usage-preview-cell" title={event.user_input_preview}>
                    {event.user_input_preview || '-'}
                  </td>
                  <td>{event.model_name || '-'}</td>
                  <td>{productTypeName(event.product_type_list)}</td>
                  <td className="credits-cell">
                    {event.credits_float > 0 ? event.credits_float.toFixed(2) : (event.amount_float > 0 ? event.amount_float.toFixed(2) : '0.00')}
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
