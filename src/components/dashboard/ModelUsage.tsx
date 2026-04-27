// ModelUsage widget — displays model breakdown with token usage and cost per model
import React, { useState } from 'react';
import { ChevronDown } from 'lucide-react';
import { TokenUsageAnalytics } from '../../stores/dashboardStore';
import './widgets.css';

interface ModelUsageProps {
  analytics: TokenUsageAnalytics | null;
}

interface ModelCostData {
  model: string;
  tokens: number;
  percent: number;
  cost: number;
  costPercent: number;
}

const ModelUsage: React.FC<ModelUsageProps> = ({ analytics }) => {
  const [showCostBreakdown, setShowCostBreakdown] = useState(false);

  if (!analytics) return null;

  // Mock cost per model (in USD per 1M tokens)
  const costPerMillion: Record<string, number> = {
    'claude-3-opus': 15.0,
    'claude-3-sonnet': 3.0,
    'gpt-4o': 5.0,
    'gpt-4-turbo': 10.0,
    'gemini-pro': 0.5,
  };

  const models = Object.entries(analytics.by_model)
    .sort(([, a], [, b]) => b - a)
    .slice(0, 5);

  const totalTokens = models.reduce((sum, [, tokens]) => sum + tokens, 0);

  // Calculate cost per model
  const modelCosts: ModelCostData[] = models.map(([model, tokens]) => {
    const costRate = costPerMillion[model] || 2.0;
    const cost = (tokens / 1000000) * costRate;
    return {
      model,
      tokens,
      percent: totalTokens > 0 ? (tokens / totalTokens) * 100 : 0,
      cost,
      costPercent: 0, // Will calculate after
    };
  });

  const totalCost = modelCosts.reduce((sum, m) => sum + m.cost, 0);
  modelCosts.forEach(m => {
    m.costPercent = totalCost > 0 ? (m.cost / totalCost) * 100 : 0;
  });

  return (
    <>
      <div className="widget model-usage">
        <div className="widget-header">
          <h3>🥧 Model Usage</h3>
          <span className="widget-label">Tokens</span>
        </div>
        <div className="widget-content">
          <div className="model-list">
            {modelCosts.map(({ model, percent }) => (
              <div key={model} className="model-item">
                <div className="model-bar-container">
                  <span className="model-name">{model}</span>
                  <div className="model-bar">
                    <div className="model-bar-fill" style={{ width: `${percent}%` }}></div>
                  </div>
                  <span className="model-percent">{percent.toFixed(1)}%</span>
                </div>
              </div>
            ))}
          </div>

          {/* Cost Breakdown Button */}
          <button
            onClick={() => setShowCostBreakdown(true)}
            style={{
              marginTop: '12px',
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
            <ChevronDown size={12} />
            <span>Cost per Model</span>
          </button>
        </div>
      </div>

      {/* Cost Breakdown Modal */}
      {showCostBreakdown && (
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
            maxWidth: '450px',
            width: '90%',
          }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '16px' }}>
              <h3 style={{ margin: 0, color: '#D6DEEB' }}>Cost per Model</h3>
              <button
                onClick={() => setShowCostBreakdown(false)}
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

            {/* Cost breakdown list */}
            {modelCosts.map(({ model, tokens, cost, costPercent }) => (
              <div key={model} style={{
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                padding: '12px',
                marginBottom: '8px',
                background: '#0B3A5C',
                borderRadius: '2px',
                border: '1px solid #1A527A',
              }}>
                <div>
                  <div style={{ color: '#D6DEEB', fontSize: '12px', fontWeight: 'bold' }}>
                    {model}
                  </div>
                  <div style={{ color: '#637777', fontSize: '10px', marginTop: '2px' }}>
                    {tokens.toLocaleString()} tokens
                  </div>
                </div>
                <div style={{ textAlign: 'right' }}>
                  <div style={{ color: '#ECC48D', fontSize: '12px', fontWeight: 'bold', marginBottom: '2px' }}>
                    ${cost.toFixed(2)}
                  </div>
                  <div style={{ color: '#637777', fontSize: '10px' }}>
                    {costPercent.toFixed(1)}% of total
                  </div>
                </div>
              </div>
            ))}

            <div style={{
              background: '#0B3A5C',
              padding: '12px',
              borderRadius: '2px',
              marginTop: '12px',
              marginBottom: '16px',
              fontSize: '12px',
            }}>
              <div style={{ color: '#637777', marginBottom: '4px' }}>Total Cost</div>
              <div style={{ color: '#ECC48D', fontWeight: 'bold', fontSize: '14px' }}>
                ${totalCost.toFixed(2)}
              </div>
            </div>

            <button
              onClick={() => setShowCostBreakdown(false)}
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

export default ModelUsage;
