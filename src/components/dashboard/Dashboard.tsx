// Dashboard — Main analytics and cost tracking interface
import React, { useEffect, useState } from 'react';
import { BarChart, Bar, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer } from 'recharts';
import { useDashboardStore } from '../../stores/dashboardStore';
import CostMeter from './CostMeter';
import TokenUsage from './TokenUsage';
import WorkerStatus from './WorkerStatus';
import SessionActivity from './SessionActivity';
import ModelUsage from './ModelUsage';
import WorkerLeaderboard from './WorkerLeaderboard';
import ApiFailures from './ApiFailures';
import { ErrorBoundary } from '../ErrorBoundary';
import './Dashboard.css';

// Doc 02 Phase 3.2 — humanise a millisecond timestamp into "Xs ago" /
// "Xm ago" / "Xh ago". Resolution caps at hours because the dashboard
// polls every 60s — any age beyond that strongly implies the polling
// fallback is broken too.
function formatAgo(ts: number): string {
  const diffSec = Math.max(0, Math.floor((Date.now() - ts) / 1_000))
  if (diffSec < 5)   return 'just now'
  if (diffSec < 60)  return `${diffSec}s ago`
  const diffMin = Math.floor(diffSec / 60)
  if (diffMin < 60)  return `${diffMin}m ago`
  const diffHr = Math.floor(diffMin / 60)
  return `${diffHr}h ago`
}

// Doc 02 Phase 3.3 — uniform "No data yet" placeholder rendered when a
// widget's payload is null (refresh failed) instead of forcing the widget
// to fabricate zeros. Visually distinct from a real $0.00 reading.
function EmptyWidget({ label }: { label: string }) {
  return (
    <div style={{
      background: 'var(--bg-surface)',
      border:     '1px dashed var(--border-1)',
      borderRadius: 3,
      padding:    '14px 18px',
      color:      'var(--text-3)',
      fontFamily: 'monospace',
      fontSize:   '11px',
      minHeight:  60,
      display:    'flex',
      alignItems: 'center',
      justifyContent: 'center',
      gap:        6,
    }}>
      <span style={{ opacity: 0.6 }}>—</span>
      <span>{label}: no data yet</span>
    </div>
  )
}

const Dashboard: React.FC = () => {
  const {
    summary,
    costAnalytics,
    tokenUsage,
    apiFailures,
    costPrediction,
    dailyBudget,
    loadDailyCap,
    dateRange,
    isLoading,
    error,
    lastRefreshedAt,
    refreshDashboard,
    startRealtimeUpdates,
    stopRealtimeUpdates,
    setDateRange,
    exportCsv,
    exportJson,
    clearError,
  } = useDashboardStore();

  // Doc 02 Phase 3.2 — re-render once a second so the "Last updated"
  // label stays current without the user touching anything.
  const [, setNow] = useState(Date.now());
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 1_000);
    return () => clearInterval(id);
  }, []);

  const [exportPath, setExportPath] = useState<string>('');
  const [showExportDialog, setShowExportDialog] = useState(false);

  // Mount once: load data, start realtime listeners, and add a 60s polling
  // fallback for Mode 2 LivingAgent ticks that don't emit agent:done.
  useEffect(() => {
    refreshDashboard();
    // P2-50. The ceiling that actually enforces, rather than the hardcoded 50
    // this component was silently falling back to — it never passed the prop at
    // all, so CostMeter used its OWN default and the figure on screen was
    // unrelated to the cap that stops work.
    loadDailyCap();
    startRealtimeUpdates();

    const interval = setInterval(() => {
      refreshDashboard();
    }, 60_000);

    return () => {
      clearInterval(interval);
      stopRealtimeUpdates();
    };
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const handleDateRangeChange = (days: number) => {
    const end = new Date();
    const start = new Date(end.getTime() - days * 24 * 60 * 60 * 1000);
    setDateRange(
      start.toISOString().split('T')[0],
      end.toISOString().split('T')[0]
    );
  };

  const handleExportCsv = async () => {
    if (!exportPath) return;
    try {
      await exportCsv(dateRange.start, dateRange.end, exportPath);
      setShowExportDialog(false);
      setExportPath('');
    } catch (err) {
      console.error('Export failed:', err);
    }
  };

  const handleExportJson = async () => {
    if (!exportPath) return;
    try {
      await exportJson(dateRange.start, dateRange.end, exportPath);
      setShowExportDialog(false);
      setExportPath('');
    } catch (err) {
      console.error('Export failed:', err);
    }
  };

  return (
    <div className="dashboard-container">
      <div className="dashboard-header">
        <h1>Dashboard</h1>
        <div className="dashboard-controls">
          <div className="date-range-picker">
            <button
              className={`range-btn ${dateRange.start === new Date(Date.now() - 1 * 24 * 60 * 60 * 1000).toISOString().split('T')[0] ? 'active' : ''}`}
              onClick={() => handleDateRangeChange(1)}
            >
              Today
            </button>
            <button
              className={`range-btn ${dateRange.start === new Date(Date.now() - 7 * 24 * 60 * 60 * 1000).toISOString().split('T')[0] ? 'active' : ''}`}
              onClick={() => handleDateRangeChange(7)}
            >
              7 Days
            </button>
            <button
              className={`range-btn ${dateRange.start === new Date(Date.now() - 30 * 24 * 60 * 60 * 1000).toISOString().split('T')[0] ? 'active' : ''}`}
              onClick={() => handleDateRangeChange(30)}
            >
              30 Days
            </button>
            <input
              type="date"
              value={dateRange.start}
              onChange={(e) => setDateRange(e.target.value, dateRange.end)}
              className="date-input"
              title="Start date"
              aria-label="Start date"
            />
            <span className="date-separator">to</span>
            <input
              type="date"
              value={dateRange.end}
              onChange={(e) => setDateRange(dateRange.start, e.target.value)}
              className="date-input"
              title="End date"
              aria-label="End date"
            />
          </div>
          <div className="dashboard-actions">
            {/* Doc 02 Phase 3.2 — last-refreshed-at label so the user can
                tell whether the Refresh click round-tripped. */}
            {lastRefreshedAt !== null && (
              <span
                style={{
                  fontSize: '11px',
                  color: 'var(--text-3)',
                  fontFamily: 'monospace',
                  marginRight: 8,
                }}
                title={new Date(lastRefreshedAt).toISOString()}
              >
                ↺ {formatAgo(lastRefreshedAt)}
              </span>
            )}
            <button
              className="btn-refresh"
              onClick={() => { if (!isLoading) refreshDashboard() }}
              disabled={isLoading}
              style={{ cursor: isLoading ? 'wait' : 'pointer' }}
              title={isLoading ? 'Refresh in flight…' : 'Reload all widgets'}
            >
              {isLoading ? 'Refreshing…' : 'Refresh'}
            </button>
            <button className="btn-export" onClick={() => setShowExportDialog(true)}>
              Export
            </button>
          </div>
        </div>
      </div>

      {error && (
        <div className="error-banner">
          <p>{error}</p>
          <button onClick={clearError}>Dismiss</button>
        </div>
      )}

      <div className="dashboard-grid">
        {/* Top row: 4 metric cards. Doc 02 Phase 3.3 — every widget
            renders an explicit "No data yet" card when its source payload
            is null (refresh failed) instead of fabricating a misleading
            $0.00 / 0 tokens / 0 workers reading. */}
        <div className="metrics-row">
          {summary
            ? <ErrorBoundary label="Cost Meter" inline><CostMeter summary={summary} prediction={costPrediction} tokenUsage={tokenUsage} dailyBudget={dailyBudget} /></ErrorBoundary>
            : <EmptyWidget label="Cost" />}
          {summary
            ? <ErrorBoundary label="Token Usage" inline><TokenUsage summary={summary} analytics={tokenUsage} costAnalytics={costAnalytics} /></ErrorBoundary>
            : <EmptyWidget label="Tokens" />}
          {summary
            ? <ErrorBoundary label="Worker Status" inline><WorkerStatus summary={summary} /></ErrorBoundary>
            : <EmptyWidget label="Workers" />}
          {summary
            ? <ErrorBoundary label="Session Activity" inline><SessionActivity summary={summary} /></ErrorBoundary>
            : <EmptyWidget label="Sessions" />}
        </div>

        {/* Middle row: Charts */}
        <div className="charts-row">
          {!costAnalytics && <EmptyWidget label="Cost trend" />}
          {costAnalytics && costAnalytics.data_points.length === 0 && (
            <EmptyWidget label="Cost trend" />
          )}
          {costAnalytics && costAnalytics.data_points.length > 0 && (
            <ErrorBoundary label="Cost Chart" inline>
              <div className="chart-container cost-chart">
                <h3>Cost Trend ({costAnalytics.data_points.length} Days)</h3>
                <ResponsiveContainer width="100%" height={180}>
                  <BarChart data={costAnalytics.data_points} margin={{ top: 4, right: 8, left: 0, bottom: 0 }}>
                    <CartesianGrid strokeDasharray="3 3" stroke="#0D3652" />
                    <XAxis
                      dataKey="timestamp"
                      stroke="#637777"
                      tick={{ fontSize: 10 }}
                      tickFormatter={(t: string) =>
                        new Date(t).toLocaleDateString('en', { month: 'short', day: 'numeric' })
                      }
                    />
                    <YAxis
                      stroke="#637777"
                      tick={{ fontSize: 10 }}
                      tickFormatter={(v: number) => `$${v.toFixed(2)}`}
                    />
                    <Tooltip
                      contentStyle={{ background: '#011627', border: '1px solid #0D3652', borderRadius: '2px' }}
                      labelStyle={{ color: '#D6DEEB', fontSize: '11px' }}
                      labelFormatter={(t: string) => new Date(t).toLocaleDateString()}
                      formatter={(v: unknown) => [`$${(v as number).toFixed(4)}`, 'Cost']}
                    />
                    <Bar dataKey="cost_usd" fill="#ECC48D" radius={[2, 2, 0, 0]} />
                  </BarChart>
                </ResponsiveContainer>
              </div>
            </ErrorBoundary>
          )}
          {tokenUsage
            ? <ErrorBoundary label="Model Usage" inline><ModelUsage analytics={tokenUsage} /></ErrorBoundary>
            : <EmptyWidget label="Model usage" />}
        </div>

        {/* Bottom row: Leaderboard and Failures */}
        <div className="bottom-row">
          <ErrorBoundary label="Worker Leaderboard" inline><WorkerLeaderboard /></ErrorBoundary>
          {apiFailures
            ? <ErrorBoundary label="API Failures" inline><ApiFailures analytics={apiFailures} /></ErrorBoundary>
            : <EmptyWidget label="API failures" />}
        </div>
      </div>

      {/* Export Dialog */}
      {showExportDialog && (
        <div className="modal-overlay" onClick={() => setShowExportDialog(false)}>
          <div className="modal-content" onClick={(e) => e.stopPropagation()}>
            <h2>Export Analytics</h2>
            <p>Choose format and location to export data from {dateRange.start} to {dateRange.end}</p>
            <input
              type="text"
              placeholder="Enter file path (e.g., /home/user/analytics.csv)"
              value={exportPath}
              onChange={(e) => setExportPath(e.target.value)}
              className="export-path-input"
            />
            <div className="modal-actions">
              <button className="btn-primary" onClick={handleExportCsv}>
                Export as CSV
              </button>
              <button className="btn-primary" onClick={handleExportJson}>
                Export as JSON
              </button>
              <button className="btn-secondary" onClick={() => setShowExportDialog(false)}>
                Cancel
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default Dashboard;
