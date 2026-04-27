// WorkerStatus widget — displays active workers and health with quick actions
import React, { useState } from 'react';
import { Users, Plus } from 'lucide-react';
import { DashboardSummary } from '../../stores/dashboardStore';
import './widgets.css';

interface WorkerStatusProps {
  summary: DashboardSummary;
}

const WorkerStatus: React.FC<WorkerStatusProps> = ({ summary }) => {
  const [showWorkerList, setShowWorkerList] = useState(false);
  const [showAddWorker, setShowAddWorker] = useState(false);

  const healthStatus = summary.active_workers > 0 ? 'healthy' : 'idle';

  // Mock worker list
  const mockWorkers = [
    { id: 'worker-backend-01', name: 'Backend Worker 1', status: 'active', tasks: 5 },
    { id: 'worker-frontend-01', name: 'Frontend Worker 1', status: 'active', tasks: 3 },
    { id: 'worker-mcp-01', name: 'MCP Worker 1', status: 'idle', tasks: 0 },
    { id: 'supervisor', name: 'Supervisor', status: 'active', tasks: 2 },
  ];

  return (
    <>
      <div className="widget worker-status">
        <div className="widget-header">
          <h3>⚡ Workers</h3>
          <span className="widget-label">Active</span>
        </div>
        <div className="widget-content">
          <div className="metric-value">
            {summary.active_workers} / {summary.total_workers}
          </div>
          <div className={`health-indicator ${healthStatus}`}>
            <span className="health-dot"></span>
            <span className="health-text">{healthStatus}</span>
          </div>
          <div className="worker-info">
            <div className="info-item">
              <span className="label">Added today:</span>
              <span className="value">+{summary.workers_added_today}</span>
            </div>
          </div>

          {/* Quick Action Buttons */}
          <div style={{ display: 'flex', gap: '8px', marginTop: '12px' }}>
            <button
              onClick={() => setShowWorkerList(true)}
              style={{
                flex: 1,
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
              <Users size={12} />
              <span>All Workers</span>
            </button>
            <button
              onClick={() => setShowAddWorker(true)}
              style={{
                flex: 1,
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
              <Plus size={12} />
              <span>Add Worker</span>
            </button>
          </div>
        </div>
      </div>

      {/* Worker List Modal */}
      {showWorkerList && (
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
            maxWidth: '500px',
            width: '90%',
            maxHeight: '80vh',
            overflowY: 'auto',
          }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '16px' }}>
              <h3 style={{ margin: 0, color: '#D6DEEB' }}>All Workers</h3>
              <button
                onClick={() => setShowWorkerList(false)}
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

            {/* Worker list */}
            {mockWorkers.map((worker) => (
              <div key={worker.id} style={{
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
                    {worker.name}
                  </div>
                  <div style={{ color: '#637777', fontSize: '10px', marginTop: '2px' }}>
                    {worker.id}
                  </div>
                </div>
                <div style={{ textAlign: 'right' }}>
                  <div style={{
                    color: worker.status === 'active' ? '#7FDBCA' : '#637777',
                    fontSize: '11px',
                    fontWeight: 'bold',
                    marginBottom: '2px',
                  }}>
                    {worker.status}
                  </div>
                  <div style={{ color: '#637777', fontSize: '10px' }}>
                    {worker.tasks} tasks
                  </div>
                </div>
              </div>
            ))}

            <button
              onClick={() => setShowWorkerList(false)}
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

      {/* Add Worker Modal */}
      {showAddWorker && (
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
              <h3 style={{ margin: 0, color: '#D6DEEB' }}>Add New Worker</h3>
              <button
                onClick={() => setShowAddWorker(false)}
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

            <div style={{ marginBottom: '12px' }}>
              <label style={{ color: '#D6DEEB', fontSize: '12px', display: 'block', marginBottom: '4px' }}>
                Worker Name
              </label>
              <input
                type="text"
                placeholder="e.g., worker-backend-02"
                style={{
                  width: '100%',
                  padding: '8px',
                  background: '#0D2137',
                  border: '1px solid #0D3652',
                  borderRadius: '2px',
                  color: '#D6DEEB',
                  fontSize: '12px',
                  boxSizing: 'border-box',
                }}
              />
            </div>

            <div style={{ marginBottom: '16px' }}>
              <label style={{ color: '#D6DEEB', fontSize: '12px', display: 'block', marginBottom: '4px' }}>
                Worker Type
              </label>
              <select
                title="Select worker type"
                style={{
                  width: '100%',
                  padding: '8px',
                  background: '#0D2137',
                  border: '1px solid #0D3652',
                  borderRadius: '2px',
                  color: '#D6DEEB',
                  fontSize: '12px',
                }}
              >
                <option>backend</option>
                <option>frontend</option>
                <option>mcp</option>
                <option>platform</option>
              </select>
            </div>

            <div style={{ display: 'flex', gap: '8px' }}>
              <button
                onClick={() => setShowAddWorker(false)}
                style={{
                  flex: 1,
                  padding: '8px',
                  background: '#0B3A5C',
                  border: '1px solid #1A527A',
                  borderRadius: '2px',
                  color: '#637777',
                  cursor: 'pointer',
                  fontSize: '12px',
                }}
              >
                Cancel
              </button>
              <button
                style={{
                  flex: 1,
                  padding: '8px',
                  background: '#82AAFF',
                  border: '1px solid #82AAFF',
                  borderRadius: '2px',
                  color: '#011627',
                  cursor: 'pointer',
                  fontSize: '12px',
                  fontWeight: 'bold',
                }}
              >
                Create
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
};

export default WorkerStatus;
