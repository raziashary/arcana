import React from 'react';
import { Plan, PlanScope, PlanStatus } from '../../stores/planStore';
import './PlanCard.css';

interface PlanCardProps {
  plan: Plan;
  onClick: (plan: Plan) => void;
}

const statusDotColor = (status: PlanStatus): string => {
  switch (status) {
    case 'draft':
      return 'dot-dim';
    case 'active':
      return 'dot-green';
    case 'completed':
      return 'dot-blue';
    case 'abandoned':
      return 'dot-dim';
    default:
      return 'dot-dim';
  }
};

const scopeBadgeColor = (scope: PlanScope): string => {
  switch (scope) {
    case 'personal':
      return 'scope-personal';
    case 'fleet':
      return 'scope-fleet';
    case 'epic':
      return 'scope-epic';
    default:
      return 'scope-personal';
  }
};

const agentBadgeColor = (agent?: string): string => {
  if (!agent) return 'agent-pa';
  if (agent === 'supervisor') return 'agent-supervisor';
  if (agent.startsWith('worker')) return 'agent-worker';
  if (agent === 'qc') return 'agent-qc';
  return 'agent-pa';
};

export const PlanCard: React.FC<PlanCardProps> = ({ plan, onClick }) => {
  const createdDate = new Date(plan.created_at);
  const now = new Date();
  const diffMs = now.getTime() - createdDate.getTime();
  const diffMins = Math.floor(diffMs / 60000);
  const diffHours = Math.floor(diffMs / 3600000);
  const diffDays = Math.floor(diffMs / 86400000);

  let timeStr = '';
  if (diffMins < 1) timeStr = 'just now';
  else if (diffMins < 60) timeStr = `${diffMins}m ago`;
  else if (diffHours < 24) timeStr = `${diffHours}h ago`;
  else timeStr = `${diffDays}d ago`;

  return (
    <div className="plan-card" onClick={() => onClick(plan)}>
      <div className="plan-card-header">
        <div className="plan-card-title-row">
          <span className={`status-dot ${statusDotColor(plan.status)}`}></span>
          <h3 className="plan-card-title">{plan.title}</h3>
        </div>
        <div className="plan-card-badges">
          <span className={`scope-badge ${scopeBadgeColor(plan.agent_scope)}`}>
            {plan.agent_scope}
          </span>
          {plan.assigned_agent && (
            <span className={`agent-badge ${agentBadgeColor(plan.assigned_agent)}`}>
              {plan.assigned_agent}
            </span>
          )}
        </div>
      </div>

      <div className="plan-card-meta">
        <span className="plan-card-time">{timeStr}</span>
        <span className="plan-card-status">{plan.status}</span>
      </div>

      {plan.parent_plan_id && (
        <div className="plan-card-parent">
          <span className="parent-indicator">↳ Nested plan</span>
        </div>
      )}
    </div>
  );
};
