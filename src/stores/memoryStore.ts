// Memory system Zustand store — Phase 4.8 (frontend)
// Mirrors the Rust memory module's 3-tier architecture and provides
// reactive state for the MemorySidebar and MemoryStats components.

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';

// ─── Types ───────────────────────────────────────────────────────────────────

export type MemoryTier = 'episodic' | 'semantic' | 'procedural';

export interface EpisodicMemory {
  id:           string;
  session_id:   string;
  turn_index:   number;
  role:         'user' | 'agent';
  content:      string;
  model:        string | null;
  cost_usd:     number;
  tokens_in:    number;
  tokens_out:   number;
  thinking:     string | null;
  tool_calls:   unknown | null;
  working_dir:  string | null;
  summary_type: string | null;
  metadata:     unknown | null;
  created_at:   string;
}

export interface SemanticFact {
  id:             string;
  fact_text:      string;
  category:       string;
  confidence:     number;
  source_type:    string;
  source_session: string | null;
  working_dir:    string | null;
  tags:           string[];
  is_active:      boolean;
  created_at:     string;
  updated_at:     string;
}

export interface ProceduralMemory {
  id:         string;
  topic:      string;
  title:      string;
  content:    string;
  tags:       string[];
  created_at: string;
  updated_at: string;
}

export interface RecallResult {
  tier:       MemoryTier;
  id:         string;
  content:    string;
  score:      string;
  created_at: string;
}

export interface MemoryStats {
  episodic_count:   number;
  semantic_count:   number;
  procedural_count: number;
  session_count:    number;
  last_updated:     string | null;
}

// ─── Store state ─────────────────────────────────────────────────────────────

interface MemoryState {
  // Data
  timeline:       EpisodicMemory[];
  facts:          SemanticFact[];
  topics:         ProceduralMemory[];
  topicContents:  Record<string, string>;  // slug → full markdown, loaded on demand
  memoryMd:       string;
  memoryMdPreview: string;                 // first 200 lines, loaded at startup
  searchResults:  RecallResult[];
  stats:          MemoryStats | null;

  // UI state
  activeTab:           'timeline' | 'facts' | 'procedural' | 'search';
  searchQuery:         string;
  loadingTimeline:     boolean;
  loadingFacts:        boolean;
  loadingTopics:       boolean;
  loadingTopicContent: boolean;
  loadingSearch:       boolean;
  loadingStats:        boolean;
  error:               string | null;

  // Actions
  loadTimeline:        (sessionId: string) => Promise<void>;
  loadFacts:           () => Promise<void>;
  loadTopics:          () => Promise<void>;
  loadTopicContent:    (topic: string) => Promise<string>;
  loadMemoryMdPreview: () => Promise<void>;
  loadStats:           () => Promise<void>;
  searchMemory:        (sessionId: string, query: string) => Promise<void>;
  storeFact:           (factText: string, category: string, sessionId?: string, tags?: string[]) => Promise<string>;
  updateFact:          (id: string, newText: string, confidence?: number) => Promise<void>;
  deactivateFact:      (id: string) => Promise<void>;
  saveTopic:           (topic: string, title: string, content: string, tags?: string[]) => Promise<void>;
  deleteTopic:         (topic: string) => Promise<void>;
  setActiveTab:        (tab: MemoryState['activeTab']) => void;
  setSearchQuery:      (q: string) => void;
  clearError:          () => void;
}

// ─── Store ───────────────────────────────────────────────────────────────────

export const useMemoryStore = create<MemoryState>((set, get) => ({
  timeline:         [],
  facts:            [],
  topics:           [],
  topicContents:    {},
  memoryMd:         '',
  memoryMdPreview:  '',
  searchResults:    [],
  stats:            null,

  activeTab:           'timeline',
  searchQuery:         '',
  loadingTimeline:     false,
  loadingFacts:        false,
  loadingTopics:       false,
  loadingTopicContent: false,
  loadingSearch:       false,
  loadingStats:        false,
  error:               null,

  loadTimeline: async (sessionId) => {
    set({ loadingTimeline: true, error: null });
    try {
      const timeline = await invoke<EpisodicMemory[]>('get_session_timeline', { sessionId });
      set({ timeline });
    } catch (e) {
      set({ error: String(e) });
    } finally {
      set({ loadingTimeline: false });
    }
  },

  loadFacts: async () => {
    set({ loadingFacts: true, error: null });
    try {
      const facts = await invoke<SemanticFact[]>('recall_facts', { limit: 50 });
      set({ facts });
    } catch (e) {
      set({ error: String(e) });
    } finally {
      set({ loadingFacts: false });
    }
  },

  loadTopics: async () => {
    set({ loadingTopics: true, error: null });
    try {
      // list_memory_topics returns metadata; content is stripped in list view
      // and loaded on demand via loadTopicContent.
      const topics = await invoke<ProceduralMemory[]>('list_memory_topics');
      set({ topics });
    } catch (e) {
      set({ error: String(e) });
    } finally {
      set({ loadingTopics: false });
    }
  },

  loadTopicContent: async (topic) => {
    const cached = get().topicContents[topic];
    if (cached !== undefined) return cached;
    set({ loadingTopicContent: true });
    try {
      const mem = await invoke<ProceduralMemory>('get_memory_topic', { topic });
      const content = mem.content;
      set((s) => ({ topicContents: { ...s.topicContents, [topic]: content } }));
      return content;
    } catch (e) {
      set({ error: String(e) });
      return '';
    } finally {
      set({ loadingTopicContent: false });
    }
  },

  loadMemoryMdPreview: async () => {
    try {
      const full: string = await invoke<string>('get_memory_md');
      // Limit to first 200 lines — topic files load on demand.
      const preview = full.split('\n').slice(0, 200).join('\n');
      set({ memoryMd: full, memoryMdPreview: preview });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  loadStats: async () => {
    set({ loadingStats: true });
    try {
      const stats = await invoke<MemoryStats>('get_memory_stats');
      set({ stats });
    } catch (e) {
      set({ error: String(e) });
    } finally {
      set({ loadingStats: false });
    }
  },

  searchMemory: async (sessionId, query) => {
    if (!query.trim()) { set({ searchResults: [] }); return; }
    set({ loadingSearch: true, error: null });
    try {
      const results = await invoke<RecallResult[]>('search_memory', {
        sessionId, query, limit: 20,
      });
      set({ searchResults: results });
    } catch (e) {
      set({ error: String(e) });
    } finally {
      set({ loadingSearch: false });
    }
  },

  storeFact: async (factText, category, sessionId, tags) => {
    const id = await invoke<string>('store_fact_cmd', {
      factText, category, sessionId, tags: tags ?? [],
    });
    await get().loadFacts();
    return id;
  },

  updateFact: async (id, newText, confidence) => {
    await invoke('update_fact_cmd', { id, newText, confidence });
    await get().loadFacts();
  },

  deactivateFact: async (id) => {
    await invoke('deactivate_fact', { id });
    set((s) => ({ facts: s.facts.filter((f) => f.id !== id) }));
  },

  saveTopic: async (topic, title, content, tags) => {
    await invoke('save_memory_topic', { topic, title, content, tags: tags ?? [] });
    await get().loadTopics();
  },

  deleteTopic: async (topic) => {
    await invoke('delete_memory_topic', { topic });
    set((s) => ({ topics: s.topics.filter((t) => t.topic !== topic) }));
  },

  setActiveTab:   (activeTab)   => set({ activeTab }),
  setSearchQuery: (searchQuery) => set({ searchQuery }),
  clearError:     ()            => set({ error: null }),
}));
