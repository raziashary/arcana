// CostMeter widget — displays today's cost vs yesterday with trend, budget progress, and drill-down
import React, { useState } from 'react';
import { ChevronDown } from 'lucide-react';
import { DashboardSummary, CostPrediction, TokenUsageAnalytics } from '../../stores/dashboardStore';
import './widgets.css';

interface CostMeterProps {
  summary: DashboardSummary;
  prediction: CostPrediction | null;
  dailyBudget?: number;
  tokenUsage?: TokenUsageAnalytics | null;
}

interface CostBreakdown {
  provider: string;
  cost: number;
  percentage: number;
}

const CostMeter: React.FC<CostMeterProps> = ({ summary, prediction, dailyBudget = 50, tokenUsage }) => {
  const [showDrilldown, setShowDrilldown] = useState(false);
  
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

  // Calculate budget progress
  const budgetPercent = (summary.today_cost / dailyBudget) * 100;
  const budgetColor = budgetPercent > 90 ? '#FF6363' : budgetPercent > 75 ? '#FFCB8B' : '#7FDBCA';

  // Derive real provider breakdown from tokenUsage.by_provider.
  // Approximate cost rates (USD per 1M tokens) — used only for share computation, not absolute cost.
  const providerRates: Record<string, number> = {
    anthropic: 3.0, openai: 5.0, google: 0.5, mistral: 1.0, cohere: 1.0,
  };

  const providerBreakdown: CostBreakdown[] = (() => {
    const byProvider = tokenUsage?.by_provider ?? {};
    const entries = Object.entries(byProvider);
    if (entries.length > 0) {
      const weightedCosts = entries.map(([p, tokens]) => ({
        provider: p.charAt(0).toUpperCase() + p.slice(1),
        weight: (tokens / 1_000_000) * (providerRates[p.toLowerCase()] ?? 2.0),
      }));
      const totalWeight = weightedCosts.reduce((s, c) => s + c.weight, 0) || 1;
      return weightedCosts.map(c => ({
        provider: c.provider,
        cost: summary.today_cost * (c.weight / totalWeight),
        percentage: (c.weight / totalWeight) * 100,
      }));
    }
    // Fallback: proportional split until real provider data arrives
    return [
      { provider: 'Anthropic', cost: summary.today_cost * 0.65, percentage: 65 },
      { provider: 'OpenAI',    cost: summary.today_cost * 0.25, percentage: 25 },
      { provider: 'Google',    cost: summary.today_cost * 0.10, percentage: 10 },
    ];
  })();

  return (
    <>
      <div className="widget cost-meter">
        <div className="widget-header">
          <h3>💰 Cost</h3>
          <span className="widget-label">Today</span>
        </div>
        <div className="widget-content">
          <div className="metric-value">${summary.today_cost.toFixed(2)}</div>
          <div className={`metric-change ${getTrendColor(summary.cost_change_percent)}`}>
            <span className="trend-icon">{getTrendIcon(summary.cost_change_percent)}</span>
            <span className="trend-percent">{Math.abs(summary.cost_change_percent).toFixed(1)}%</span>
            <span className="trend-label">vs yesterday</span>
          </div>

          {/* Budget Progress Bar */}
          <div style={{ marginTop: '12px' }}>
            <div style={{ fontSize: '10px', color: '#637777', marginBottom: '4px' }}>
              Budget: ${summary.today_cost.toFixed(2)} / ${dailyBudget.toFixed(2)}
            </div>
            <div style={{
              width: '100%',
              height: '6px',
              background: '#0D3652',
              borderRadius: '2px',
              overflow: 'hidden',
            }}>
              <div style={{
                width: `${Math.min(budgetPercent, 100)}%`,
                height: '100%',
                background: budgetColor,
                transition: 'width 0.3s ease',
              }} />
            </div>
          </div>

          {prediction && (
            <div className="metric-projection">
              <div className="projection-label">Projected monthly:</div>
              <div className="projection-value">${prediction.projected_monthly_cost.toFixed(2)}</div>
            </div>
          )}

          {/* Drill-down Button */}
          <button
            onClick={() => setShowDrilldown(true)}
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
            <span>Breakdown by Provider</span>
            <ChevronDown size={12} />
          </button>
        </div>
      </div>

      {/* Drill-down Modal */}
      {showDrilldown && (
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
              <h3 style={{ margin: 0, color: '#D6DEEB' }}>Cost Breakdown</h3>
              <button
                onClick={() => setShowDrilldown(false)}
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

            {/* Provider breakdown table */}
            <div style={{ marginBottom: '16px' }}>
              {providerBreakdown.map((item) => (
                <div key={item.provider} style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  padding: '8px 0',
                  borderBottom: '1px solid #0D3652',
                  fontSize: '12px',
                }}>
                  <span style={{ color: '#D6DEEB' }}>{item.provider}</span>
                  <div style={{ display: 'flex', gap: '12px', alignItems: 'center' }}>
                    <span style={{ color: '#82AAFF', minWidth: '60px', textAlign: 'right' }}>
                      ${item.cost.toFixed(2)}
                    </span>
                    <span style={{ color: '#637777', minWidth: '40px', textAlign: 'right' }}>
                      {item.percentage}%
                    </span>
                  </div>
                </div>
              ))}
            </div>

            {/* Total */}
            <div style={{
              display: 'flex',
              justifyContent: 'space-between',
              padding: '8px 0',
              fontSize: '12px',
              fontWeight: 'bold',
              color: '#ECC48D',
            }}>
              <span>Total</span>
              <span>${summary.today_cost.toFixed(2)}</span>
            </div>

            <button
              onClick={() => setShowDrilldown(false)}
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

export default CostMeter;
