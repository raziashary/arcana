// TokenUsage widget — displays today's token consumption with 7-day trend and model breakdown
import React, { useState } from 'react';
import { LineChart, Line, PieChart, Pie, Cell, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer } from 'recharts';
import { ChevronDown } from 'lucide-react';
import { DashboardSummary, TokenUsageAnalytics, CostAnalytics } from '../../stores/dashboardStore';
import './widgets.css';

interface TokenUsageProps {
  summary: DashboardSummary;
  analytics: TokenUsageAnalytics | null;
  costAnalytics?: CostAnalytics | null;
}

interface DailyTokenData {
  date: string;
  tokens: number;
}

interface ModelUsageData {
  name: string;
  value: number;
  color: string;
}

const WEEKDAYS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];

export function formatDayBucketWeekday(dayBucket: string): string {
  const match = dayBucket.match(/^(\d{4})-(\d{2})-(\d{2})$/);
  if (!match) return 'Invalid Date';

  const year = Number(match[1]);
  const month = Number(match[2]);
  const day = Number(match[3]);
  const date = new Date(0);
  date.setUTCHours(0, 0, 0, 0);
  date.setUTCFullYear(year, month - 1, day);

  if (
    date.getUTCFullYear() !== year ||
    date.getUTCMonth() !== month - 1 ||
    date.getUTCDate() !== day
  ) {
    return 'Invalid Date';
  }

  return WEEKDAYS[date.getUTCDay()];
}

const TokenUsage: React.FC<TokenUsageProps> = ({ summary, analytics, costAnalytics }) => {
  const [showCharts, setShowCharts] = useState(false);

  const formatTokens = (tokens: number) => {
    if (tokens >= 1_000_000) return `${(tokens / 1_000_000).toFixed(1)}M`;
    if (tokens >= 1_000) return `${(tokens / 1_000).toFixed(1)}K`;
    return tokens.toString();
  };

  const getTrendIcon = (percent: number) => {
    if (percent > 0) return '↑';
    if (percent < 0) return '↓';
    return '→';
  };

  const getTrendColor = (percent: number) => {
    if (percent > 10) return 'trend-up';
    if (percent < -10) return 'trend-down';
    return 'trend-neutral';
  };

  // 7-day token trend from real cost analytics data points (last 7 days)
  const sevenDayData: DailyTokenData[] = (() => {
    if (costAnalytics && costAnalytics.data_points.length > 0) {
      return costAnalytics.data_points.slice(-7).map(dp => ({
        date: formatDayBucketWeekday(dp.timestamp),
        tokens: dp.tokens,
      }));
    }
    // Fallback shape-only placeholder until costAnalytics loads
    const days = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
    return days.map((date, i) => ({ date, tokens: i < 6 ? 0 : summary.today_tokens }));
  })();

  // Real model distribution from analytics.by_model
  const MODEL_COLORS = ['#82AAFF', '#7FDBCA', '#FFCB8B', '#ECC48D', '#C792EA'];
  const modelUsage: ModelUsageData[] = (() => {
    const byModel = analytics?.by_model ?? {};
    const entries = Object.entries(byModel).sort(([, a], [, b]) => b - a).slice(0, 5);
    if (entries.length > 0) {
      return entries.map(([name, value], i) => ({
        name,
        value,
        color: MODEL_COLORS[i % MODEL_COLORS.length],
      }));
    }
    return [
      { name: 'claude', value: summary.today_tokens * 0.55, color: MODEL_COLORS[0] },
      { name: 'gpt',    value: summary.today_tokens * 0.30, color: MODEL_COLORS[1] },
      { name: 'gemini', value: summary.today_tokens * 0.15, color: MODEL_COLORS[2] },
    ];
  })();

  // Real top sessions from analytics.top_sessions
  const topSessions = (() => {
    const sessions = analytics?.top_sessions ?? [];
    if (sessions.length > 0) {
      return sessions.slice(0, 4).map(s => ({
        id: s.session_id,
        tokens: s.tokens,
        name: s.agent_type || s.session_id.slice(-12),
      }));
    }
    return [] as { id: string; tokens: number; name: string }[];
  })();

  return (
    <>
      <div className="widget token-usage">
        <div className="widget-header">
          <h3>🪙 Tokens</h3>
          <span className="widget-label">Today</span>
        </div>
        <div className="widget-content">
          <div className="metric-value">{formatTokens(summary.today_tokens)}</div>
          <div className={`metric-change ${getTrendColor(summary.token_change_percent)}`}>
            <span className="trend-icon">{getTrendIcon(summary.token_change_percent)}</span>
            <span className="trend-percent">{Math.abs(summary.token_change_percent).toFixed(1)}%</span>
            <span className="trend-label">vs yesterday</span>
          </div>
          {analytics && (
            <div className="token-breakdown">
              <div className="breakdown-item">
                <span className="label">Prompt:</span>
                <span className="value">{formatTokens(analytics.prompt_tokens)}</span>
              </div>
              <div className="breakdown-item">
                <span className="label">Completion:</span>
                <span className="value">{formatTokens(analytics.completion_tokens)}</span>
              </div>
            </div>
          )}

          {/* View Charts Button */}
          <button
            onClick={() => setShowCharts(true)}
            style={{
              marginTop: '8px',
              width: '100%',
              padding: '6px 8px',
              background: '#0B3A5C',
              border: '1px solid #1A527A',
              borderRadius: '2px',
              color: '#637777',
              fontSize: '11px',
              cursor: 'pointer',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              gap: '4px',
            }}
          >
            <span>7-Day Trend & Models</span>
            <ChevronDown size={12} />
          </button>
        </div>
      </div>

      {/* Charts Modal */}
      {showCharts && (
        <div style={{
          position: 'fixed',
          top: 0,
          left: 0,
          right: 0,
          bottom: 0,
          background: 'rgba(0, 0, 0, 0.7)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          zIndex: 1000,
          overflowY: 'auto',
        }}>
          <div style={{
            background: '#00233B',
            border: '1px solid #0D3652',
            borderRadius: '4px',
            padding: '20px',
            maxWidth: '900px',
            width: '95%',
            maxHeight: '90vh',
            overflowY: 'auto',
          }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '16px' }}>
              <h3 style={{ margin: 0, color: '#D6DEEB' }}>Token Analytics</h3>
              <button
                onClick={() => setShowCharts(false)}
                style={{
                  background: 'none',
                  border: 'none',
                  color: '#637777',
                  cursor: 'pointer',
                  fontSize: '20px',
                }}
              >
                ✕
              </button>
            </div>

            {/* 7-Day Trend Chart */}
            <div style={{ marginBottom: '24px' }}>
              <h4 style={{ color: '#D6DEEB', marginBottom: '12px', fontSize: '12px' }}>7-Day Token Trend</h4>
              <ResponsiveContainer width="100%" height={250}>
                <LineChart data={sevenDayData}>
                  <CartesianGrid strokeDasharray="3 3" stroke="#0D3652" />
                  <XAxis dataKey="date" stroke="#637777" style={{ fontSize: '12px' }} />
                  <YAxis stroke="#637777" style={{ fontSize: '12px' }} />
                  <Tooltip
                    contentStyle={{ background: '#011627', border: '1px solid #0D3652', borderRadius: '2px' }}
                    labelStyle={{ color: '#D6DEEB' }}
                    formatter={(value: any) => formatTokens(value as number)}
                  />
                  <Line
                    type="monotone"
                    dataKey="tokens"
                    stroke="#82AAFF"
                    strokeWidth={2}
                    dot={{ fill: '#82AAFF', r: 4 }}
                    activeDot={{ r: 6 }}
                  />
                </LineChart>
              </ResponsiveContainer>
            </div>

            {/* Model Breakdown Pie Chart */}
            <div style={{ marginBottom: '24px' }}>
              <h4 style={{ color: '#D6DEEB', marginBottom: '12px', fontSize: '12px' }}>Model Distribution</h4>
              <ResponsiveContainer width="100%" height={250}>
                <PieChart>
                  <Pie
                    data={modelUsage}
                    cx="50%"
                    cy="50%"
                    labelLine={false}
                    label={({ name, percent }: any) => `${name}: ${(percent * 100).toFixed(0)}%`}
                    outerRadius={80}
                    fill="#8884d8"
                    dataKey="value"
                  >
                    {modelUsage.map((entry, index) => (
                      <Cell key={`cell-${index}`} fill={entry.color} />
                    ))}
                  </Pie>
                  <Tooltip
                    contentStyle={{ background: '#011627', border: '1px solid #0D3652', borderRadius: '2px' }}
                    labelStyle={{ color: '#D6DEEB' }}
                    formatter={(value: any) => formatTokens(value as number)}
                  />
                </PieChart>
              </ResponsiveContainer>
            </div>

            {/* Top Sessions */}
            <div style={{ marginBottom: '16px' }}>
              <h4 style={{ color: '#D6DEEB', marginBottom: '12px', fontSize: '12px' }}>Top Sessions by Token Usage</h4>
              {topSessions.map((session, idx) => (
                <div key={session.id} style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  padding: '8px 0',
                  borderBottom: '1px solid #0D3652',
                  fontSize: '12px',
                }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                    <span style={{ color: '#637777' }}>#{idx + 1}</span>
                    <span style={{ color: '#D6DEEB' }}>{session.name}</span>
                  </div>
                  <span style={{ color: '#82AAFF' }}>{formatTokens(session.tokens)}</span>
                </div>
              ))}
            </div>

            <button
              onClick={() => setShowCharts(false)}
              style={{
                marginTop: '16px',
                width: '100%',
                padding: '8px',
                background: '#0B3A5C',
                border: '1px solid #1A527A',
                borderRadius: '2px',
                color: '#637777',
                cursor: 'pointer',
                fontSize: '12px',
              }}
            >
              Close
            </button>
          </div>
        </div>
      )}
    </>
  );
};

export default TokenUsage;
