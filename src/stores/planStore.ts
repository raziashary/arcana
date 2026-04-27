import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export type PlanStatus = 'draft' | 'active' | 'completed' | 'abandoned';
export type PlanScope = 'personal' | 'fleet' | 'epic';
export type PlanItemStatus = 'pending' | 'in_progress' | 'done' | 'blocked';

export interface Plan {
  id: string;
  session_id?: string;
  title: string;
  description?: string;
  status: PlanStatus;
  created_at: string;
  updated_at: string;
  assigned_agent?: string;
  created_by: string;
  agent_scope: PlanScope;
  parent_plan_id?: string;
  task_id?: number;
}

export interface PlanItem {
  id: string;
  plan_id: string;
  parent_id?: string;
  title: string;
  description?: string;
  status: PlanItemStatus;
  order_index: number;
  agent_type?: string;
  estimated_tools?: string[];
  created_at: string;
  completed_at?: string;
  assigned_worker?: string;
  worktree_branch?: string;
  pr_url?: string;
  result_summary?: string;
}

export interface PlanTree {
  plan: Plan;
  items: PlanItem[];
}

interface PlanStore {
  activePlan: Plan | null;
  planHistory: Plan[];
  fleetPlans: Plan[];
  planTree: PlanTree | null;
  isLoading: boolean;

  // Core actions
  loadPlan: (planId: string) => Promise<void>;
  createPlan: (title: string, description: string, scope: PlanScope) => Promise<Plan>;
  updateStepStatus: (planId: string, stepIndex: number, status: PlanItemStatus) => Promise<void>;
  setActivePlan: (plan: Plan | null) => void;
  deletePlan: (planId: string) => Promise<void>;

  // Fleet actions
  claimPlanItem: (planId: string, workerId: string) => Promise<PlanItem | null>;
  createSubplan: (parentId: string, workerId: string, title: string, description: string) => Promise<Plan>;
  loadPlansForAgent: (agentId: string, scope: PlanScope) => Promise<void>;
  loadPlanTree: (planId: string) => Promise<void>;
  submitForQC: (planId: string, workerId: string, summary: string, prUrls: string[]) => Promise<void>;

  // Utility
  listPlans: (scope?: PlanScope, status?: PlanStatus) => Promise<Plan[]>;
}

export const usePlanStore = create<PlanStore>((set) => {
  // Setup event listeners
  listen('plan:created', (event: any) => {
    console.log('Plan created:', event.payload);
  });

  listen('plan:updated', (event: any) => {
    console.log('Plan updated:', event.payload);
  });

  listen('plan:step_completed', (event: any) => {
    console.log('Plan step completed:', event.payload);
  });

  listen('plan:item_claimed', (event: any) => {
    console.log('Plan item claimed:', event.payload);
  });

  listen('plan:subplan_created', (event: any) => {
    console.log('Subplan created:', event.payload);
  });

  return {
    activePlan: null,
    planHistory: [],
    fleetPlans: [],
    planTree: null,
    isLoading: false,

    setActivePlan: (plan) => set({ activePlan: plan }),

    loadPlan: async (planId: string) => {
      set({ isLoading: true });
      try {
        const plan = await invoke<Plan>('get_plan', { plan_id: planId });
        set({ activePlan: plan });
      } catch (error) {
        console.error('Failed to load plan:', error);
      } finally {
        set({ isLoading: false });
      }
    },

    createPlan: async (title: string, description: string, scope: PlanScope) => {
      set({ isLoading: true });
      try {
        const yaml_dsl = `action: create
title: "${title}"
description: "${description}"
scope: ${scope}`;

        const result = await invoke<any>('planner_compile', { yaml_dsl });
        const plan = result.plan || result;
        set((state) => ({
          activePlan: plan,
          planHistory: [...state.planHistory, plan],
        }));
        return plan;
      } catch (error) {
        console.error('Failed to create plan:', error);
        throw error;
      } finally {
        set({ isLoading: false });
      }
    },

    updateStepStatus: async (planId: string, stepIndex: number, status: PlanItemStatus) => {
      set({ isLoading: true });
      try {
        const yaml_dsl = `action: updateStep
plan_id: "${planId}"
step_index: ${stepIndex}
status: ${status}`;

        await invoke('planner_compile', { yaml_dsl });
      } catch (error) {
        console.error('Failed to update step:', error);
        throw error;
      } finally {
        set({ isLoading: false });
      }
    },

    deletePlan: async (planId: string) => {
      set({ isLoading: true });
      try {
        await invoke('delete_plan', { plan_id: planId });
        set((state) => ({
          activePlan: state.activePlan?.id === planId ? null : state.activePlan,
          planHistory: state.planHistory.filter((p) => p.id !== planId),
        }));
      } catch (error) {
        console.error('Failed to delete plan:', error);
        throw error;
      } finally {
        set({ isLoading: false });
      }
    },

    claimPlanItem: async (planId: string, workerId: string) => {
      set({ isLoading: true });
      try {
        const item = await invoke<PlanItem | null>('claim_plan_item', {
          plan_id: planId,
          worker_id: workerId,
        });
        return item;
      } catch (error) {
        console.error('Failed to claim item:', error);
        throw error;
      } finally {
        set({ isLoading: false });
      }
    },

    createSubplan: async (
      parentId: string,
      workerId: string,
      title: string,
      description: string
    ) => {
      set({ isLoading: true });
      try {
        const plan = await invoke<Plan>('create_subplan', {
          parent_plan_id: parentId,
          worker_id: workerId,
          title,
          description,
        });
        set((state) => ({
          fleetPlans: [...state.fleetPlans, plan],
        }));
        return plan;
      } catch (error) {
        console.error('Failed to create subplan:', error);
        throw error;
      } finally {
        set({ isLoading: false });
      }
    },

    loadPlansForAgent: async (agentId: string, scope: PlanScope) => {
      set({ isLoading: true });
      try {
        const plans = await invoke<Plan[]>('list_plans', {
          filter: { scope, assigned_agent: agentId },
        });
        set({ fleetPlans: plans });
      } catch (error) {
        console.error('Failed to load plans for agent:', error);
      } finally {
        set({ isLoading: false });
      }
    },

    loadPlanTree: async (planId: string) => {
      set({ isLoading: true });
      try {
        const tree = await invoke<PlanTree>('get_plan_tree', { plan_id: planId });
        set({ planTree: tree });
      } catch (error) {
        console.error('Failed to load plan tree:', error);
      } finally {
        set({ isLoading: false });
      }
    },

    submitForQC: async (planId: string, workerId: string, summary: string, prUrls: string[]) => {
      set({ isLoading: true });
      try {
        await invoke('submit_plan_for_qc', {
          plan_id: planId,
          worker_id: workerId,
          result_summary: summary,
          pr_urls: prUrls,
        });
      } catch (error) {
        console.error('Failed to submit for QC:', error);
        throw error;
      } finally {
        set({ isLoading: false });
      }
    },

    listPlans: async (scope?: PlanScope, status?: PlanStatus) => {
      set({ isLoading: true });
      try {
        const plans = await invoke<Plan[]>('list_plans', {
          filter: { scope, status },
        });
        return plans;
      } catch (error) {
        console.error('Failed to list plans:', error);
        return [];
      } finally {
        set({ isLoading: false });
      }
    },
  };
});
