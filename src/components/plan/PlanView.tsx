import React, { useState } from 'react';
import { Plan, PlanItem, PlanItemStatus, usePlanStore } from '../../stores/planStore';
import './PlanView.css';

interface PlanViewProps {
  plan: Plan;
  items: PlanItem[];
  onClose: () => void;
}

const statusIcon = (status: PlanItemStatus): string => {
  switch (status) {
    case 'pending':
      return '⭕';
    case 'in_progress':
      return '🔄';
    case 'done':
      return '✅';
    case 'blocked':
      return '🚫';
    default:
      return '⭕';
  }
};

export const PlanView: React.FC<PlanViewProps> = ({ plan, items, onClose }) => {
  const [expandedItems, setExpandedItems] = useState<Set<string>>(new Set());
  const { updateStepStatus } = usePlanStore();

  const toggleExpanded = (itemId: string) => {
    const newExpanded = new Set(expandedItems);
    if (newExpanded.has(itemId)) {
      newExpanded.delete(itemId);
    } else {
      newExpanded.add(itemId);
    }
    setExpandedItems(newExpanded);
  };

  const handleStatusChange = async (itemId: string, newStatus: PlanItemStatus) => {
    const itemIndex = items.findIndex((i) => i.id === itemId);
    if (itemIndex >= 0) {
      await updateStepStatus(plan.id, itemIndex, newStatus);
    }
  };

  const completedCount = items.filter((i) => i.status === 'done').length;
  const progressPercent = items.length > 0 ? (completedCount / items.length) * 100 : 0;

  return (
    <div className="plan-view">
      <div className="plan-view-header">
        <div className="plan-view-title-section">
          <h2 className="plan-view-title">{plan.title}</h2>
          <span className={`plan-view-status plan-status-${plan.status}`}>{plan.status}</span>
        </div>
        <button className="plan-view-close" onClick={onClose}>
          ✕
        </button>
      </div>

      {plan.description && (
        <div className="plan-view-description">
          <p>{plan.description}</p>
        </div>
      )}

      <div className="plan-view-progress">
        <div className="progress-bar">
          <div className="progress-fill" style={{ width: `${progressPercent}%` }}></div>
        </div>
        <span className="progress-text">
          {completedCount} of {items.length} steps completed
        </span>
      </div>

      <div className="plan-view-metadata">
        <span className="metadata-item">
          <strong>Scope:</strong> {plan.agent_scope}
        </span>
        {plan.assigned_agent && (
          <span className="metadata-item">
            <strong>Agent:</strong> {plan.assigned_agent}
          </span>
        )}
        <span className="metadata-item">
          <strong>Created by:</strong> {plan.created_by}
        </span>
      </div>

      <div className="plan-view-items">
        <h3 className="items-title">Steps</h3>
        {items.length === 0 ? (
          <div className="no-items">No steps yet</div>
        ) : (
          <div className="items-list">
            {items.map((item) => (
              <div key={item.id} className="plan-item">
                <div
                  className="plan-item-header"
                  onClick={() => toggleExpanded(item.id)}
                >
                  <span className="item-status-icon">{statusIcon(item.status)}</span>
                  <span className="item-order">{item.order_index + 1}.</span>
                  <span className="item-title">{item.title}</span>
                  {item.assigned_worker && (
                    <span className="item-worker-badge">{item.assigned_worker}</span>
                  )}
                  <span className="item-expand-icon">
                    {expandedItems.has(item.id) ? '▼' : '▶'}
                  </span>
                </div>

                {expandedItems.has(item.id) && (
                  <div className="plan-item-details">
                    {item.description && (
                      <div className="item-description">
                        <p>{item.description}</p>
                      </div>
                    )}

                    {item.estimated_tools && item.estimated_tools.length > 0 && (
                      <div className="item-tools">
                        <strong>Estimated tools:</strong>
                        <div className="tools-list">
                          {item.estimated_tools.map((tool) => (
                            <span key={tool} className="tool-tag">
                              {tool}
                            </span>
                          ))}
                        </div>
                      </div>
                    )}

                    {item.worktree_branch && (
                      <div className="item-worktree">
                        <strong>Branch:</strong> <code>{item.worktree_branch}</code>
                      </div>
                    )}

                    {item.pr_url && (
                      <div className="item-pr">
                        <strong>PR:</strong>{' '}
                        <a href={item.pr_url} target="_blank" rel="noopener noreferrer">
                          {item.pr_url}
                        </a>
                      </div>
                    )}

                    {item.result_summary && (
                      <div className="item-result">
                        <strong>Result:</strong> {item.result_summary}
                      </div>
                    )}

                    <div className="item-actions">
                      <select
                        value={item.status}
                        onChange={(e) =>
                          handleStatusChange(item.id, e.target.value as PlanItemStatus)
                        }
                        className="status-select"
                        aria-label={`Change status for ${item.title}`}
                      >
                        <option value="pending">Pending</option>
                        <option value="in_progress">In Progress</option>
                        <option value="done">Done</option>
                        <option value="blocked">Blocked</option>
                      </select>
                    </div>
                  </div>
                )}
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
};
