import React from 'react';
import { Plan } from '../../stores/planStore';
import './PlanDisplay.css';

interface PlanDisplayProps {
  plan: Plan;
  onJumpToPlan: (plan: Plan) => void;
}

export const PlanDisplay: React.FC<PlanDisplayProps> = ({ plan, onJumpToPlan }) => {
  return (
    <div className="plan-display">
      <div className="plan-display-card">
        <div className="plan-display-header">
          <div className="plan-display-title-section">
            <h4 className="plan-display-title">{plan.title}</h4>
            <span className={`plan-display-scope plan-scope-${plan.agent_scope}`}>
              {plan.agent_scope}
            </span>
          </div>
          <span className={`plan-display-status plan-status-${plan.status}`}>
            {plan.status}
          </span>
        </div>

        {plan.description && (
          <div className="plan-display-description">
            <p>{plan.description}</p>
          </div>
        )}

        <div className="plan-display-meta">
          <span className="meta-item">
            <strong>Created by:</strong> {plan.created_by}
          </span>
          {plan.assigned_agent && (
            <span className="meta-item">
              <strong>Agent:</strong> {plan.assigned_agent}
            </span>
          )}
        </div>

        <button
          className="plan-display-button"
          onClick={() => onJumpToPlan(plan)}
        >
          View Full Plan →
        </button>
      </div>
    </div>
  );
};
