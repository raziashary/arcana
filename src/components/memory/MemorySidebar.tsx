// Memory Sidebar — Phase 4.8
// Full 3-tier memory browser: Timeline (episodic), Facts (semantic),
// Patterns (procedural), and cross-tier Search.
// Shown as a collapsible right-panel or dedicated tab in the main UI.

import React, { useEffect, useState } from 'react';
import {
  Brain, Clock, BookOpen, Search, Trash2,
  Edit3, Plus, ChevronDown, ChevronRight, X,
} from 'lucide-react';
import { useMemoryStore, SemanticFact, ProceduralMemory } from '../../stores/memoryStore';
import { MemoryStats } from './MemoryStats';

// ─── Timeline tab ─────────────────────────────────────────────────────────────

const TimelineTab: React.FC<{ sessionId: string }> = ({ sessionId }) => {
  const { timeline, loadTimeline, loadingTimeline } = useMemoryStore();

  useEffect(() => {
    if (sessionId) loadTimeline(sessionId);
  }, [sessionId, loadTimeline]);

  if (loadingTimeline) return <div className="memory-pane__loading">Loading timeline…</div>;
  if (!timeline.length) return (
    <div className="memory-pane__empty">
      No episodic memories yet for this session. They are recorded automatically as you chat.
    </div>
  );

  return (
    <div className="memory-timeline">
      {timeline.map((entry) => (
        <div key={entry.id} className={`memory-timeline__entry memory-timeline__entry--${entry.role}`}>
          <div className="memory-timeline__meta">
            <span className={`memory-timeline__role memory-timeline__role--${entry.role}`}>
              {entry.role === 'user' ? 'You' : 'Agent'}
            </span>
            <span className="memory-timeline__turn">turn {entry.turn_index}</span>
            {entry.summary_type && (
              <span className="memory-timeline__badge memory-timeline__badge--summary">summary</span>
            )}
          </div>
          <p className="memory-timeline__content">
            {entry.content.length > 300
              ? `${entry.content.slice(0, 300)}…`
              : entry.content}
          </p>
          {(entry.cost_usd > 0 || entry.tokens_out > 0) && (
            <div className="memory-timeline__cost">
              {entry.tokens_in > 0 && <span>{entry.tokens_in.toLocaleString()} in</span>}
              {entry.tokens_out > 0 && <span>{entry.tokens_out.toLocaleString()} out</span>}
              {entry.cost_usd > 0 && <span>${entry.cost_usd.toFixed(4)}</span>}
            </div>
          )}
        </div>
      ))}
    </div>
  );
};

// ─── Facts tab ────────────────────────────────────────────────────────────────

const FactsTab: React.FC = () => {
  const { facts, loadFacts, loadingFacts, deactivateFact, updateFact } = useMemoryStore();
  const [editId, setEditId]     = useState<string | null>(null);
  const [editText, setEditText] = useState('');
  const [newFact, setNewFact]   = useState('');
  const { storeFact }           = useMemoryStore();

  useEffect(() => { loadFacts(); }, [loadFacts]);

  const handleEdit = (f: SemanticFact) => {
    setEditId(f.id);
    setEditText(f.fact_text);
  };

  const handleSaveEdit = async () => {
    if (!editId) return;
    await updateFact(editId, editText);
    setEditId(null);
    setEditText('');
  };

  const handleAddFact = async () => {
    if (!newFact.trim()) return;
    await storeFact(newFact.trim(), 'preference', undefined, []);
    setNewFact('');
  };

  if (loadingFacts) return <div className="memory-pane__loading">Loading facts…</div>;

  return (
    <div className="memory-facts">
      <div className="memory-facts__add">
        <input
          className="memory-facts__input"
          placeholder="Add a fact to remember…"
          value={newFact}
          onChange={(e) => setNewFact(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && handleAddFact()}
        />
        <button className="memory-facts__add-btn" onClick={handleAddFact} title="Add fact">
          <Plus size={14} />
        </button>
      </div>

      {!facts.length && (
        <div className="memory-pane__empty">
          No facts stored yet. Facts are extracted automatically or added with the
          <code> remember_this</code> tool.
        </div>
      )}

      {facts.map((f) => (
        <div key={f.id} className="memory-fact">
          <div className="memory-fact__header">
            <span className={`memory-fact__category memory-fact__category--${f.category}`}>
              {f.category}
            </span>
            <span className="memory-fact__confidence" title="Confidence score">
              {Math.round(f.confidence * 100)}%
            </span>
            <div className="memory-fact__actions">
              <button onClick={() => handleEdit(f)} title="Edit">
                <Edit3 size={12} />
              </button>
              <button onClick={() => deactivateFact(f.id)} title="Remove">
                <Trash2 size={12} />
              </button>
            </div>
          </div>
          {editId === f.id ? (
            <div className="memory-fact__edit">
              <textarea
                className="memory-fact__edit-area"
                title="Edit fact text"
                aria-label="Edit fact text"
                value={editText}
                onChange={(e) => setEditText(e.target.value)}
                rows={3}
              />
              <div className="memory-fact__edit-actions">
                <button className="memory-fact__save" onClick={handleSaveEdit}>Save</button>
                <button className="memory-fact__cancel" onClick={() => setEditId(null)}>Cancel</button>
              </div>
            </div>
          ) : (
            <p className="memory-fact__text">{f.fact_text}</p>
          )}
          {f.tags.length > 0 && (
            <div className="memory-fact__tags">
              {f.tags.map((t) => <span key={t} className="memory-fact__tag">{t}</span>)}
            </div>
          )}
        </div>
      ))}
    </div>
  );
};

// ─── Procedural tab ───────────────────────────────────────────────────────────

const ProceduralTab: React.FC = () => {
  const {
    topics, loadTopics, loadingTopics,
    topicContents, loadTopicContent, loadingTopicContent,
    memoryMdPreview, loadMemoryMdPreview,
    saveTopic, deleteTopic,
  } = useMemoryStore();

  const [expanded, setExpanded]       = useState<string | null>(null);
  const [newTopic, setNewTopic]       = useState('');
  const [editing, setEditing]         = useState<ProceduralMemory | null>(null);
  const [editContent, setEditContent] = useState('');

  // Phase 4.3 Loading Rules: MEMORY.md preview (first 200 lines) on mount,
  // topic content loaded on demand when a topic row is expanded.
  useEffect(() => {
    loadTopics();
    loadMemoryMdPreview();
  }, [loadTopics, loadMemoryMdPreview]);

  const handleToggle = async (topic: string) => {
    if (expanded === topic) {
      setExpanded(null);
      return;
    }
    setExpanded(topic);
    // Load content on demand if not yet cached.
    if (topicContents[topic] === undefined) {
      await loadTopicContent(topic);
    }
  };

  const handleNewTopic = async () => {
    if (!newTopic.trim()) return;
    const slug = newTopic.trim().toLowerCase().replace(/\s+/g, '-').replace(/[^a-z0-9-]/g, '');
    await saveTopic(slug, newTopic.trim(), `# ${newTopic.trim()}\n\n`, []);
    setNewTopic('');
  };

  const handleEdit = async (t: ProceduralMemory) => {
    // Ensure full content is loaded before opening editor.
    let content = topicContents[t.topic];
    if (content === undefined) {
      content = await loadTopicContent(t.topic);
    }
    setEditing(t);
    setEditContent(content);
  };

  const handleSaveEdit = async () => {
    if (!editing) return;
    await saveTopic(editing.topic, editing.title, editContent, editing.tags);
    setEditing(null);
    setEditContent('');
  };

  if (loadingTopics) return <div className="memory-pane__loading">Loading patterns…</div>;

  return (
    <div className="memory-procedural">
      {/* MEMORY.md index preview — first 200 lines loaded at startup */}
      {memoryMdPreview && (
        <details className="memory-procedural__md-preview">
          <summary className="memory-procedural__md-summary">MEMORY.md index</summary>
          <pre className="memory-procedural__md-content">{memoryMdPreview}</pre>
        </details>
      )}

      <div className="memory-procedural__add">
        <input
          className="memory-procedural__input"
          placeholder="New pattern topic name…"
          value={newTopic}
          onChange={(e) => setNewTopic(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && handleNewTopic()}
        />
        <button className="memory-procedural__add-btn" onClick={handleNewTopic} title="Create topic">
          <Plus size={14} />
        </button>
      </div>

      {!topics.length && (
        <div className="memory-pane__empty">
          No patterns stored yet. Use the <code>remember_this</code> tool or create a topic above.
        </div>
      )}

      {topics.map((t) => {
        const isExpanded = expanded === t.topic;
        const isEditing  = editing?.topic === t.topic;
        const content    = topicContents[t.topic];

        return (
          <div key={t.topic} className="memory-topic">
            <div className="memory-topic__header" onClick={() => handleToggle(t.topic)}>
              {isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
              <span className="memory-topic__title">{t.title}</span>
              <div className="memory-topic__actions" onClick={(e) => e.stopPropagation()}>
                <button onClick={() => handleEdit(t)} title="Edit">
                  <Edit3 size={12} />
                </button>
                <button onClick={() => deleteTopic(t.topic)} title="Delete">
                  <Trash2 size={12} />
                </button>
              </div>
            </div>

            {isExpanded && isEditing && (
              <div className="memory-topic__edit">
                <textarea
                  className="memory-topic__edit-area"
                  title="Edit topic content"
                  aria-label="Edit topic content"
                  value={editContent}
                  onChange={(e) => setEditContent(e.target.value)}
                  rows={10}
                />
                <div className="memory-topic__edit-actions">
                  <button className="memory-topic__save" onClick={handleSaveEdit}>Save</button>
                  <button className="memory-topic__cancel" onClick={() => setEditing(null)}>Cancel</button>
                </div>
              </div>
            )}

            {isExpanded && !isEditing && (
              loadingTopicContent && content === undefined
                ? <div className="memory-pane__loading">Loading…</div>
                : <pre className="memory-topic__content">{content ?? ''}</pre>
            )}
          </div>
        );
      })}
    </div>
  );
};

// ─── Search tab ───────────────────────────────────────────────────────────────

const SearchTab: React.FC<{ sessionId: string }> = ({ sessionId }) => {
  const { searchQuery, setSearchQuery, searchResults, loadingSearch, searchMemory } = useMemoryStore();
  const [debounced, setDebounced] = useState(searchQuery);

  useEffect(() => {
    const id = setTimeout(() => setDebounced(searchQuery), 300);
    return () => clearTimeout(id);
  }, [searchQuery]);

  useEffect(() => {
    if (debounced) searchMemory(sessionId, debounced);
  }, [debounced, sessionId, searchMemory]);

  return (
    <div className="memory-search">
      <div className="memory-search__bar">
        <Search size={14} className="memory-search__icon" />
        <input
          className="memory-search__input"
          placeholder="Search all memory tiers…"
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
        />
        {searchQuery && (
          <button className="memory-search__clear" title="Clear search" onClick={() => setSearchQuery('')}>
            <X size={12} />
          </button>
        )}
      </div>

      {loadingSearch && <div className="memory-pane__loading">Searching…</div>}

      {!loadingSearch && !searchResults.length && debounced && (
        <div className="memory-pane__empty">No results for "{debounced}"</div>
      )}

      {searchResults.map((r, i) => (
        <div key={`${r.id}-${i}`} className="memory-result">
          <div className="memory-result__header">
            <span
              className={`memory-result__tier memory-result__tier--${r.tier}`}
            >
              {r.tier}
            </span>
            <span className="memory-result__score">score {r.score}</span>
          </div>
          <p className="memory-result__content">{r.content}</p>
        </div>
      ))}
    </div>
  );
};

// ─── Sidebar root ─────────────────────────────────────────────────────────────

interface MemorySidebarProps {
  sessionId: string;
  onClose?: () => void;
}

export const MemorySidebar: React.FC<MemorySidebarProps> = ({ sessionId, onClose }) => {
  const { activeTab, setActiveTab } = useMemoryStore();

  const tabs = [
    { id: 'timeline',   label: 'Timeline',  icon: Clock },
    { id: 'facts',      label: 'Facts',     icon: Brain },
    { id: 'procedural', label: 'Patterns',  icon: BookOpen },
    { id: 'search',     label: 'Search',    icon: Search },
  ] as const;

  return (
    <aside className="memory-sidebar">
      <div className="memory-sidebar__header">
        <div className="memory-sidebar__title">
          <Brain size={16} />
          <span>Memory</span>
        </div>
        {onClose && (
          <button className="memory-sidebar__close" title="Close memory panel" onClick={onClose}>
            <X size={14} />
          </button>
        )}
      </div>

      <MemoryStats />

      <nav className="memory-sidebar__tabs">
        {tabs.map(({ id, label, icon: Icon }) => (
          <button
            key={id}
            className={`memory-sidebar__tab${activeTab === id ? ' memory-sidebar__tab--active' : ''}`}
            onClick={() => setActiveTab(id)}
          >
            <Icon size={12} />
            {label}
          </button>
        ))}
      </nav>

      <div className="memory-sidebar__pane">
        {activeTab === 'timeline'   && <TimelineTab sessionId={sessionId} />}
        {activeTab === 'facts'      && <FactsTab />}
        {activeTab === 'procedural' && <ProceduralTab />}
        {activeTab === 'search'     && <SearchTab sessionId={sessionId} />}
      </div>
    </aside>
  );
};
