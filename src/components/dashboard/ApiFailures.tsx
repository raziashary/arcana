// ApiFailures widget — displays recent API errors and failures with provider-specific rates
import React, { useState } from 'react';
import { ChevronDown } from 'lucide-react';
import { ApiFailureAnalytics } from '../../stores/dashboardStore';
import './widgets.css';

interface ApiFailuresProps {
  analytics: ApiFailureAnalytics | null;
}

interface ProviderFailureRate {
  provider: string;
  failures: number;
  rate: number; // percentage
  trend?: number; // % change
}

const ApiFailures: React.FC<ApiFailuresProps> = ({ analytics }) => {
  const [showDetails, setShowDetails] = useState(false);

  if (!analytics) return null;

  const recentFailures = analytics.recent_failures.slice(0, 5);

  // Mock provider-specific failure rates
  const providerRates: ProviderFailureRate[] = [
    { provider: 'Anthropic', failures: 2, rate: 0.5, trend: -15 },
    { provider: 'OpenAI', failures: 5, rate: 2.1, trend: 8 },
    { provider: 'Google', failures: 1, rate: 0.8, trend: -5 },
  ];

  const getErrorIcon = (errorType: string) => {
    switch (errorType) {
      case 'rate_limit':
        return '⏱️';
      case 'auth':
        return '🔐';
      case 'timeout':
        return '⏳';
      case 'model_error':
        return '❌';
      default:
        return '⚠️';
    }
  };

  const formatTime = (timestamp: string) => {
    const date = new Date(timestamp);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMins = Math.floor(diffMs / 60000);

    if (diffMins < 1) return 'just now';
    if (diffMins < 60) return `${diffMins}m ago`;
    const diffHours = Math.floor(diffMins / 60);
    if (diffHours < 24) return `${diffHours}h ago`;
    return date.toLocaleDateString();
  };

  const getTrendColor = (trend?: number) => {
    if (!trend) return '#637777';
    if (trend < 0) return '#7FDBCA'; // improving
    return '#FF6363'; // worsening
  };

  return (
    <>
      <div className="widget api-failures">
        <div className="widget-header">
          <h3>⚠️ API Failures</h3>
          <span className="failure-count">{analytics.total_failures}</span>
        </div>
        <div className="widget-content">
          {recentFailures.length > 0 ? (
            <div className="failures-list">
              {recentFailures.map((failure, idx) => (
                <div key={idx} className="failure-item">
                  <div className="failure-icon">{getErrorIcon(failure.error_type)}</div>
                  <div className="failure-details">
                    <div className="failure-provider">{failure.provider}</div>
                    <div className="failure-message">{failure.error_message}</div>
                    <div className="failure-time">{formatTime(failure.timestamp)}</div>
                  </div>
                  {failure.resolved && <span className="resolved-badge">✓ Resolved</span>}
                </div>
              ))}
            </div>
          ) : (
            <div className="no-failures">No recent failures</div>
          )}
          <div className="failure-stats">
            <div className="stat">
              <span className="label">Auto-retry success:</span>
              <span className="value">{(analytics.auto_retry_success_rate * 100).toFixed(1)}%</span>
            </div>
          </div>

          {/* Provider Rates Button */}
          <button
            onClick={() => setShowDetails(true)}
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
            <span>Provider Failure Rates</span>
            <ChevronDown size={12} />
          </button>
        </div>
      </div>

      {/* Provider Rates Modal */}
      {showDetails && (
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
        }}>
          <div style={{
            background: '#00233B',
            border: '1px solid #0D3652',
            borderRadius: '4px',
            padding: '20px',
            maxWidth: '400px',
            width: '90%',
          }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '16px' }}>
              <h3 style={{ margin: 0, color: '#D6DEEB' }}>Provider Failure Rates</h3>
              <button
                onClick={() => setShowDetails(false)}
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

            {/* Provider rates table */}
            <div style={{ marginBottom: '16px' }}>
              {providerRates.map((item) => (
                <div key={item.provider} style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'center',
                  padding: '12px 0',
                  borderBottom: '1px solid #0D3652',
                  fontSize: '12px',
                }}>
                  <div>
                    <div style={{ color: '#D6DEEB', marginBottom: '4px' }}>{item.provider}</div>
                    <div style={{ color: '#637777', fontSize: '10px' }}>
                      {item.failures} failures
                    </div>
                  </div>
                  <div style={{ textAlign: 'right' }}>
                    <div style={{ color: '#FF6363', marginBottom: '4px' }}>
                      {item.rate.toFixed(2)}% rate
                    </div>
                    {item.trend && (
                      <div style={{ color: getTrendColor(item.trend), fontSize: '10px' }}>
                        {item.trend < 0 ? '↓' : '↑'} {Math.abs(item.trend)}%
                      </div>
                    )}
                  </div>
                </div>
              ))}
            </div>

            {/* Overall stats */}
            <div style={{
              background: '#0B3A5C',
              padding: '12px',
              borderRadius: '2px',
              marginBottom: '16px',
              fontSize: '11px',
            }}>
              <div style={{ color: '#637777', marginBottom: '4px' }}>
                Last 24 hours
              </div>
              <div style={{ color: '#D6DEEB', fontWeight: 'bold' }}>
                {analytics.total_failures} total failures
              </div>
            </div>

            <button
              onClick={() => setShowDetails(false)}
              style={{
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

export default ApiFailures;
