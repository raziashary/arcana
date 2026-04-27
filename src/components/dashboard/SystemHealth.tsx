// SystemHealth widget — displays system health monitoring (sidecars, providers, tools)
import React, { useState } from 'react';
import { AlertCircle, CheckCircle, Clock } from 'lucide-react';
import './widgets.css';

interface SystemComponent {
  name: string;
  status: 'healthy' | 'degraded' | 'down';
  uptime_percent: number;
  last_check: string;
  response_time_ms?: number;
}

interface SystemHealthData {
  timestamp: string;
  overall_status: 'healthy' | 'degraded' | 'down';
  components: SystemComponent[];
  incident_count_24h: number;
  mttr_minutes: number; // Mean Time To Recovery
}

const SystemHealth: React.FC = () => {
  const [showDetails, setShowDetails] = useState(false);

  // Mock system health data
  const healthData: SystemHealthData = {
    timestamp: new Date().toISOString(),
    overall_status: 'healthy',
    components: [
      {
        name: 'Memory MCP Server',
        status: 'healthy',
        uptime_percent: 99.9,
        last_check: '2 seconds ago',
        response_time_ms: 45,
      },
      {
        name: 'Core Orchestrator',
        status: 'healthy',
        uptime_percent: 99.8,
        last_check: '3 seconds ago',
        response_time_ms: 120,
      },
      {
        name: 'Anthropic API',
        status: 'healthy',
        uptime_percent: 99.95,
        last_check: '1 second ago',
        response_time_ms: 280,
      },
      {
        name: 'OpenAI API',
        status: 'degraded',
        uptime_percent: 98.5,
        last_check: '5 seconds ago',
        response_time_ms: 850,
      },
      {
        name: 'Google API',
        status: 'healthy',
        uptime_percent: 99.7,
        last_check: '2 seconds ago',
        response_time_ms: 320,
      },
    ],
    incident_count_24h: 2,
    mttr_minutes: 15,
  };

  const getStatusIcon = (status: string) => {
    switch (status) {
      case 'healthy':
        return <CheckCircle size={14} style={{ color: '#7FDBCA' }} />;
      case 'degraded':
        return <AlertCircle size={14} style={{ color: '#FFCB8B' }} />;
      case 'down':
        return <AlertCircle size={14} style={{ color: '#FF6363' }} />;
      default:
        return null;
    }
  };

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'healthy':
        return '#7FDBCA';
      case 'degraded':
        return '#FFCB8B';
      case 'down':
        return '#FF6363';
      default:
        return '#637777';
    }
  };

  const healthyCount = healthData.components.filter(c => c.status === 'healthy').length;
  const degradedCount = healthData.components.filter(c => c.status === 'degraded').length;

  return (
    <>
      <div className="widget system-health">
        <div className="widget-header">
          <h3>🏥 System Health</h3>
          <span
            style={{
              display: 'inline-block',
              width: '8px',
              height: '8px',
              borderRadius: '50%',
              background: getStatusColor(healthData.overall_status),
              marginLeft: '8px',
            }}
          />
        </div>
        <div className="widget-content">
          <div style={{ marginBottom: '12px' }}>
            <div style={{ fontSize: '12px', color: '#637777', marginBottom: '4px' }}>
              Component Status
            </div>
            <div style={{ display: 'flex', gap: '12px', fontSize: '12px' }}>
              <div>
                <span style={{ color: '#7FDBCA', fontWeight: 'bold' }}>{healthyCount}</span>
                <span style={{ color: '#637777' }}> healthy</span>
              </div>
              {degradedCount > 0 && (
                <div>
                  <span style={{ color: '#FFCB8B', fontWeight: 'bold' }}>{degradedCount}</span>
                  <span style={{ color: '#637777' }}> degraded</span>
                </div>
              )}
            </div>
          </div>

          <div style={{ marginBottom: '12px' }}>
            <div style={{ fontSize: '12px', color: '#637777', marginBottom: '4px' }}>
              Incidents (24h)
            </div>
            <div style={{ fontSize: '14px', color: '#D6DEEB', fontWeight: 'bold' }}>
              {healthData.incident_count_24h}
            </div>
          </div>

          <div style={{ marginBottom: '12px' }}>
            <div style={{ fontSize: '12px', color: '#637777', marginBottom: '4px' }}>
              Avg Recovery Time
            </div>
            <div style={{ fontSize: '14px', color: '#D6DEEB', fontWeight: 'bold' }}>
              {healthData.mttr_minutes}m
            </div>
          </div>

          {/* View Details Button */}
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
            <span>Component Details</span>
          </button>
        </div>
      </div>

      {/* Details Modal */}
      {showDetails && (
        <div
          style={{
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
            overflowY: 'auto',
          }}
        >
          <div
            style={{
              background: '#00233B',
              border: '1px solid #0D3652',
              borderRadius: '4px',
              padding: '20px',
              maxWidth: '500px',
              width: '95%',
              maxHeight: '80vh',
              overflowY: 'auto',
            }}
          >
            <div
              style={{
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                marginBottom: '16px',
              }}
            >
              <h3 style={{ margin: 0, color: '#D6DEEB' }}>System Components</h3>
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

            {/* Components list */}
            <div style={{ marginBottom: '16px' }}>
              {healthData.components.map((component) => (
                <div
                  key={component.name}
                  style={{
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'center',
                    padding: '12px',
                    marginBottom: '8px',
                    background: '#0B3A5C',
                    borderRadius: '2px',
                    border: `1px solid ${getStatusColor(component.status)}33`,
                  }}
                >
                  <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                    {getStatusIcon(component.status)}
                    <div>
                      <div style={{ color: '#D6DEEB', fontSize: '12px', fontWeight: 'bold' }}>
                        {component.name}
                      </div>
                      <div style={{ color: '#637777', fontSize: '10px', marginTop: '2px' }}>
                        {component.last_check}
                      </div>
                    </div>
                  </div>
                  <div style={{ textAlign: 'right' }}>
                    <div
                      style={{
                        color: getStatusColor(component.status),
                        fontSize: '11px',
                        fontWeight: 'bold',
                        marginBottom: '2px',
                      }}
                    >
                      {component.uptime_percent}% uptime
                    </div>
                    {component.response_time_ms && (
                      <div
                        style={{
                          color: '#637777',
                          fontSize: '10px',
                          display: 'flex',
                          alignItems: 'center',
                          gap: '2px',
                          justifyContent: 'flex-end',
                        }}
                      >
                        <Clock size={10} />
                        {component.response_time_ms}ms
                      </div>
                    )}
                  </div>
                </div>
              ))}
            </div>

            {/* Summary stats */}
            <div
              style={{
                background: '#0B3A5C',
                padding: '12px',
                borderRadius: '2px',
                marginBottom: '16px',
                fontSize: '11px',
              }}
            >
              <div style={{ color: '#637777', marginBottom: '4px' }}>
                Last 24 hours
              </div>
              <div style={{ color: '#D6DEEB', fontWeight: 'bold', marginBottom: '4px' }}>
                {healthData.incident_count_24h} incidents
              </div>
              <div style={{ color: '#637777', fontSize: '10px' }}>
                Avg recovery: {healthData.mttr_minutes} minutes
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

export default SystemHealth;
