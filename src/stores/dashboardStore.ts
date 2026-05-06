// Dashboard store — Zustand state management for analytics and dashboard data
import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

// Matches CostUpdatePayload in events.rs (EVT_COST_UPDATE = "cost:update")
interface CostUpdatePayload {
  session_id:  string;
  tokens_in:   number;
  tokens_out:  number;
  cost_usd:    number;
  total_today: number;
}

export interface BudgetAlert {
  id: string;
  timestamp: string;
  type: 'warning' | 'critical';
  message: string;
  current_cost: number;
  budget_limit: number;
}

export interface DashboardState {
  // Summary data
  summary: DashboardSummary | null;
  costAnalytics: CostAnalytics | null;
  tokenUsage: TokenUsageAnalytics | null;
  apiFailures: ApiFailureAnalytics | null;
  costPrediction: CostPrediction | null;

  // UI state
  dateRange: { start: string; end: string };
  isLoading: boolean;
  error: string | null;

  // Budget & Alerts
  dailyBudget: number;
  budgetAlerts: BudgetAlert[];
  realtimeEnabled: boolean;

  // Actions
  refreshDashboard: () => Promise<void>;
  setCostAnalytics: (start: string, end: string) => Promise<void>;
  setTokenUsage: (start: string, end: string) => Promise<void>;
  setApiFailures: (hours: number) => Promise<void>;
  setPrediction: () => Promise<void>;
  setDateRange: (start: string, end: string) => void;
  exportCsv: (start: string, end: string, path: string) => Promise<void>;
  exportJson: (start: string, end: string, path: string) => Promise<void>;
  setDailyBudget: (amount: number) => void;
  addBudgetAlert: (alert: BudgetAlert) => void;
  clearBudgetAlerts: () => void;
  startRealtimeUpdates: () => Promise<void>;
  stopRealtimeUpdates: () => void;
  clearError: () => void;
}

export interface DashboardSummary {
  today_cost: number;
  yesterday_cost: number;
  cost_change_percent: number;
  today_tokens: number;
  yesterday_tokens: number;
  token_change_percent: number;
  active_workers: number;
  total_workers: number;
  workers_added_today: number;
  active_sessions: number;
  sessions_today: number;
  messages_today: number;
  tool_calls_today: number;
}

export interface CostAnalytics {
  date_range: string;
  granularity: string;
  data_points: CostDataPoint[];
  total_cost: number;
  average_daily_cost: number;
  peak_cost_day: string | null;
  lowest_cost_day: string | null;
}

export interface CostDataPoint {
  timestamp: string;
  cost_usd: number;
  tokens: number;
  sessions: number;
}

export interface TokenUsageAnalytics {
  date_range: string;
  total_tokens: number;
  prompt_tokens: number;
  completion_tokens: number;
  thinking_tokens: number | null;
  by_agent_type: Record<string, number>;
  by_model: Record<string, number>;
  by_provider: Record<string, number>;
  top_sessions: SessionTokenUsage[];
}

export interface SessionTokenUsage {
  session_id: string;
  tokens: number;
  cost_usd: number;
  agent_type: string;
}

export interface ApiFailureAnalytics {
  date_range: string;
  total_failures: number;
  failures_by_provider: Record<string, number>;
  failures_by_type: Record<string, number>;
  recent_failures: ApiFailureRecord[];
  auto_retry_success_rate: number;
}

export interface ApiFailureRecord {
  id: number | null;
  timestamp: string;
  provider: string;
  error_type: string;
  error_message: string;
  request_id: string | null;
  retry_count: number;
  resolved: boolean;
}

export interface CostPrediction {
  current_daily_average: number;
  projected_monthly_cost: number;
  confidence_percent: number;
  based_on_days: number;
}

export interface ExportResult {
  format: string;
  path: string;
  rows_exported: number;
  file_size_bytes: number;
}

let realtimeUnlisteners: Array<() => void> = [];

export const useDashboardStore = create<DashboardState>((set, get) => ({
  summary: null,
  costAnalytics: null,
  tokenUsage: null,
  apiFailures: null,
  costPrediction: null,
  dateRange: {
    start: new Date(Date.now() - 7 * 24 * 60 * 60 * 1000).toISOString().split('T')[0],
    end: new Date().toISOString().split('T')[0],
  },
  isLoading: false,
  error: null,
  dailyBudget: 50,
  budgetAlerts: [],
  realtimeEnabled: false,

  refreshDashboard: async () => {
    set({ isLoading: true, error: null });
    try {
      const summary = await invoke<DashboardSummary>('get_dashboard_summary').catch(() => ({
        today_cost: 0,
        yesterday_cost: 0,
        cost_change_percent: 0,
        today_tokens: 0,
        yesterday_tokens: 0,
        token_change_percent: 0,
        active_workers: 0,
        total_workers: 0,
        workers_added_today: 0,
        active_sessions: 0,
        sessions_today: 0,
        messages_today: 0,
        tool_calls_today: 0,
      }));
      
      const { start, end } = get().dateRange;
      const costAnalytics = await invoke<CostAnalytics>('get_cost_analytics', {
        start,
        end,
        granularity: 'daily',
      }).catch(() => ({
        date_range: `${start} to ${end}`,
        granularity: 'daily',
        data_points: [],
        total_cost: 0,
        average_daily_cost: 0,
        peak_cost_day: null,
        lowest_cost_day: null,
      }));
      
      const tokenUsage = await invoke<TokenUsageAnalytics>('get_token_usage', {
        start,
        end,
      }).catch(() => ({
        date_range: `${start} to ${end}`,
        total_tokens: 0,
        prompt_tokens: 0,
        completion_tokens: 0,
        thinking_tokens: null,
        by_agent_type: {},
        by_model: {},
        by_provider: {},
        top_sessions: [],
      }));
      
      const apiFailures = await invoke<ApiFailureAnalytics>('get_api_failures', {
        hours: 24,
      }).catch(() => ({
        date_range: 'Last 24 hours',
        total_failures: 0,
        failures_by_provider: {},
        failures_by_type: {},
        recent_failures: [],
        auto_retry_success_rate: 0,
      }));
      
      const costPrediction = await invoke<CostPrediction>('predict_monthly_cost').catch(() => ({
        current_daily_average: 0,
        projected_monthly_cost: 0,
        confidence_percent: 0,
        based_on_days: 0,
      }));

      set({
        summary,
        costAnalytics,
        tokenUsage,
        apiFailures,
        costPrediction,
        isLoading: false,
      });
    } catch (error) {
      console.error('Dashboard refresh error:', error);
      set({
        error: error instanceof Error ? error.message : 'Failed to refresh dashboard',
        isLoading: false,
      });
    }
  },

  setCostAnalytics: async (start: string, end: string) => {
    set({ isLoading: true, error: null });
    try {
      const costAnalytics = await invoke<CostAnalytics>('get_cost_analytics', {
        start,
        end,
        granularity: 'daily',
      });
      set({ costAnalytics, isLoading: false });
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to fetch cost analytics',
        isLoading: false,
      });
    }
  },

  setTokenUsage: async (start: string, end: string) => {
    set({ isLoading: true, error: null });
    try {
      const tokenUsage = await invoke<TokenUsageAnalytics>('get_token_usage', {
        start,
        end,
      });
      set({ tokenUsage, isLoading: false });
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to fetch token usage',
        isLoading: false,
      });
    }
  },

  setApiFailures: async (hours: number) => {
    set({ isLoading: true, error: null });
    try {
      const apiFailures = await invoke<ApiFailureAnalytics>('get_api_failures', {
        hours,
      });
      set({ apiFailures, isLoading: false });
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to fetch API failures',
        isLoading: false,
      });
    }
  },

  setPrediction: async () => {
    set({ isLoading: true, error: null });
    try {
      const costPrediction = await invoke<CostPrediction>('predict_monthly_cost');
      set({ costPrediction, isLoading: false });
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to fetch cost prediction',
        isLoading: false,
      });
    }
  },

  setDateRange: (start: string, end: string) => {
    set({ dateRange: { start, end } });
    // Refresh all data against the new range immediately
    get().refreshDashboard();
  },

  exportCsv: async (start: string, end: string, path: string) => {
    set({ isLoading: true, error: null });
    try {
      await invoke<ExportResult>('export_costs_csv', { start, end, path });
      set({ isLoading: false });
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to export CSV',
        isLoading: false,
      });
    }
  },

  exportJson: async (start: string, end: string, path: string) => {
    set({ isLoading: true, error: null });
    try {
      await invoke<ExportResult>('export_costs_json', { start, end, path });
      set({ isLoading: false });
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to export JSON',
        isLoading: false,
      });
    }
  },

  setDailyBudget: (amount: number) => {
    set({ dailyBudget: amount });
  },

  addBudgetAlert: (alert: BudgetAlert) => {
    const current = get().budgetAlerts;
    set({ budgetAlerts: [...current, alert] });
  },

  clearBudgetAlerts: () => {
    set({ budgetAlerts: [] });
  },

  startRealtimeUpdates: async () => {
    if (get().realtimeEnabled) return;

    try {
      // Listen for cost update events — event name matches EVT_COST_UPDATE in events.rs
      const costUnlistener = await listen<CostUpdatePayload>('cost:update', (event) => {
        const { summary, dailyBudget } = get();

        // Optimistically update summary so widgets reflect the new spend immediately
        // without waiting for the next full refreshDashboard() poll.
        if (summary) {
          const newTodayCost = event.payload.total_today;
          const newTodayTokens = summary.today_tokens
            + event.payload.tokens_in
            + event.payload.tokens_out;
          const costChangePct = summary.yesterday_cost > 0
            ? ((newTodayCost - summary.yesterday_cost) / summary.yesterday_cost) * 100
            : 0;
          const tokenChangePct = summary.yesterday_tokens > 0
            ? ((newTodayTokens - summary.yesterday_tokens) / summary.yesterday_tokens) * 100
            : 0;
          set({
            summary: {
              ...summary,
              today_cost: newTodayCost,
              today_tokens: newTodayTokens,
              cost_change_percent: costChangePct,
              token_change_percent: tokenChangePct,
            },
          });
        }

        // Budget alert check
        if (event.payload.total_today > dailyBudget) {
          const alert: BudgetAlert = {
            id: `alert-${Date.now()}`,
            timestamp: new Date().toISOString(),
            type: event.payload.total_today > dailyBudget * 1.1 ? 'critical' : 'warning',
            message: `Daily cost exceeded: $${event.payload.total_today.toFixed(2)} / $${dailyBudget.toFixed(2)}`,
            current_cost: event.payload.total_today,
            budget_limit: dailyBudget,
          };
          get().addBudgetAlert(alert);
        }
      });

      // Refresh dashboard whenever an agent (Mode 1 PA) finishes a turn
      const agentDoneUnlistener = await listen('agent:done', () => {
        get().refreshDashboard();
      });

      // Refresh dashboard when a Mode 2 LivingAgent tick completes
      const tickUnlistener = await listen<{ kind: string }>('task:changed', (event) => {
        if (event.payload.kind === 'worker_tick_complete') {
          get().refreshDashboard();
        }
      });

      realtimeUnlisteners = [costUnlistener, agentDoneUnlistener, tickUnlistener];
      set({ realtimeEnabled: true });
    } catch (error) {
      console.error('Failed to start realtime updates:', error);
    }
  },

  stopRealtimeUpdates: () => {
    realtimeUnlisteners.forEach(fn => fn());
    realtimeUnlisteners = [];
    set({ realtimeEnabled: false });
  },

  clearError: () => {
    set({ error: null });
  },
}));
