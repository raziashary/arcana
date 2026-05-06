// Memory stats panel — Phase 4.8
// Shows counts for all three tiers plus a last-updated timestamp.
// Used at the top of the MemorySidebar.

import React, { useEffect } from 'react';
import { Brain, Clock, Database, BookOpen, List } from 'lucide-react';
import { useMemoryStore } from '../../stores/memoryStore';

export const MemoryStats: React.FC = () => {
  const { stats, loadStats, loadingStats } = useMemoryStore();

  useEffect(() => {
    loadStats();
    const id = setInterval(loadStats, 30_000);
    return () => clearInterval(id);
  }, [loadStats]);

  if (loadingStats && !stats) {
    return (
      <div className="memory-stats memory-stats--loading">
        <Brain size={14} className="memory-stats__icon memory-stats__icon--spin" />
        <span>Loading memory…</span>
      </div>
    );
  }

  if (!stats) return null;

  const lastUpdated = stats.last_updated
    ? new Date(stats.last_updated).toLocaleTimeString()
    : 'Never';

  return (
    <div className="memory-stats">
      <div className="memory-stats__row">
        <div className="memory-stats__item" title="Episodic memories (conversation turns)">
          <Clock size={12} className="memory-stats__icon memory-stats__icon--episodic" />
          <span className="memory-stats__label">Episodic</span>
          <span className="memory-stats__count">{stats.episodic_count.toLocaleString()}</span>
        </div>
        <div className="memory-stats__item" title="Semantic facts extracted from conversations">
          <Brain size={12} className="memory-stats__icon memory-stats__icon--semantic" />
          <span className="memory-stats__label">Facts</span>
          <span className="memory-stats__count">{stats.semantic_count.toLocaleString()}</span>
        </div>
        <div className="memory-stats__item" title="Procedural patterns & learnings">
          <BookOpen size={12} className="memory-stats__icon memory-stats__icon--procedural" />
          <span className="memory-stats__label">Patterns</span>
          <span className="memory-stats__count">{stats.procedural_count.toLocaleString()}</span>
        </div>
        <div className="memory-stats__item" title="Sessions tracked">
          <List size={12} className="memory-stats__icon" />
          <span className="memory-stats__label">Sessions</span>
          <span className="memory-stats__count">{stats.session_count.toLocaleString()}</span>
        </div>
      </div>
      <div className="memory-stats__footer">
        <Database size={10} />
        <span>Last updated: {lastUpdated}</span>
      </div>
    </div>
  );
};
