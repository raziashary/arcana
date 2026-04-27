import React, { useState } from 'react';
import { Plan } from '../../stores/planStore';
import './PlanTreeView.css';

interface PlanTreeViewProps {
  rootPlan: Plan;
  childPlans: Plan[];
  onSelectPlan: (plan: Plan) => void;
}

export const PlanTreeView: React.FC<PlanTreeViewProps> = ({
  rootPlan,
  childPlans,
  onSelectPlan,
}) => {
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set([rootPlan.id]));

  const toggleExpanded = (planId: string) => {
    const newExpanded = new Set(expandedIds);
    if (newExpanded.has(planId)) {
      newExpanded.delete(planId);
    } else {
      newExpanded.add(planId);
    }
    setExpandedIds(newExpanded);
  };

  const renderPlanNode = (plan: Plan, level: number = 0) => {
    const children = childPlans.filter((p) => p.parent_plan_id === plan.id);
    const isExpanded = expandedIds.has(plan.id);
    const hasChildren = children.length > 0;

    return (
      <div key={plan.id} className="plan-tree-node" style={{ marginLeft: `${level * 20}px` }}>
        <div className="plan-tree-node-header" onClick={() => onSelectPlan(plan)}>
          {hasChildren && (
            <button
              className={`plan-tree-expand-btn ${isExpanded ? 'expanded' : ''}`}
              onClick={(e) => {
                e.stopPropagation();
                toggleExpanded(plan.id);
              }}
            >
              ▶
            </button>
          )}
          {!hasChildren && <span className="plan-tree-spacer"></span>}

          <span className={`plan-tree-scope-dot plan-scope-${plan.agent_scope}`}></span>
          <span className="plan-tree-title">{plan.title}</span>
          <span className={`plan-tree-status plan-status-${plan.status}`}>
            {plan.status}
          </span>
        </div>

        {hasChildren && isExpanded && (
          <div className="plan-tree-children">
            {children.map((child) => renderPlanNode(child, level + 1))}
          </div>
        )}
      </div>
    );
  };

  return (
    <div className="plan-tree-view">
      <h3 className="plan-tree-title-heading">Plan Hierarchy</h3>
      <div className="plan-tree-container">{renderPlanNode(rootPlan)}</div>
    </div>
  );
};
