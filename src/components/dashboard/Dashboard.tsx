// Dashboard — Main analytics and cost tracking interface
import React, { useEffect, useState } from 'react';
import { useDashboardStore } from '../../stores/dashboardStore';
import CostMeter from './CostMeter';
import TokenUsage from './TokenUsage';
import WorkerStatus from './WorkerStatus';
import SessionActivity from './SessionActivity';
import ModelUsage from './ModelUsage';
import WorkerLeaderboard from './WorkerLeaderboard';
import ApiFailures from './ApiFailures';
import './Dashboard.css';

const Dashboard: React.FC = () => {
  const {
    summary,
    costAnalytics,
    tokenUsage,
    apiFailures,
    costPrediction,
    dateRange,
    isLoading,
    error,
    refreshDashboard,
    setDateRange,
    exportCsv,
    exportJson,
    clearError,
  } = useDashboardStore();

  const [exportPath, setExportPath] = useState<string>('');
  const [showExportDialog, setShowExportDialog] = useState(false);

  useEffect(() => {
    refreshDashboard();
  }, [refreshDashboard]);

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
            <button className="btn-refresh" onClick={refreshDashboard} disabled={isLoading}>
              {isLoading ? 'Refreshing...' : 'Refresh'}
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
        {/* Top row: 4 metric cards */}
        <div className="metrics-row">
          {summary && <CostMeter summary={summary} prediction={costPrediction} />}
          {summary && <TokenUsage summary={summary} analytics={tokenUsage} />}
          {summary && <WorkerStatus summary={summary} />}
          {summary && <SessionActivity summary={summary} />}
        </div>

        {/* Middle row: Charts */}
        <div className="charts-row">
          {costAnalytics && <div className="chart-container cost-chart">
            <h3>Cost Trend (7 Days)</h3>
            {/* Chart will be rendered by CostMeter or separate component */}
          </div>}
          {tokenUsage && <ModelUsage analytics={tokenUsage} />}
        </div>

        {/* Bottom row: Leaderboard and Failures */}
        <div className="bottom-row">
          <WorkerLeaderboard />
          {apiFailures && <ApiFailures analytics={apiFailures} />}
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
