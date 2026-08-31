// RecipeEditor — full YAML editor modal for creating and editing recipes.
// Phase 2.2: Recipe CRUD + backup/restore versioning.
// Phase 2.3: AI-assisted YAML generation via generate_recipe_yaml backend command.
// See: DEVELOPMENT_PLAN.md Phase 2.3 — Recipe Auto-Generation

import { useState, useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { RotateCcw, Wand2, ChevronDown, ChevronUp } from 'lucide-react'

// ─── Types ────────────────────────────────────────────────────────────────────

export interface Recipe {
  version:         string
  id:              string
  title:           string
  description:     string | null
  schedule:        string | null
  permission_mode: 'autonomous' | 'supervised'
  agent_role:      string | null
  model:           string | null
  is_system:       boolean
  instructions:    string
}

interface RecipeBackup {
  recipe_id:  string
  timestamp:  string
  filename:   string
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

const DEFAULT_INSTRUCTIONS =
  'You are an AutoDev worker agent.\n\n## Responsibilities\n1. Poll for assigned tasks\n2. Execute them\n3. Update status when done'

const WORKER_TYPES = [
  'supervisor', 'backend', 'frontend', 'platform',
  'mcp', 'cleanup', 'qc-agent', 'custom',
]

function slugify(name: string): string {
  return name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '')
}

// Naive YAML parser for our specific recipe schema (avoids external deps)
function parseRecipeYaml(yaml: string): Partial<Recipe> {
  const result: Partial<Recipe> = {}
  const lines = yaml.split('\n')
  let inInstructions = false
  const instrLines: string[] = []

  for (const line of lines) {
    if (line.startsWith('instructions:')) {
      inInstructions = true
      const after = line.replace('instructions:', '').trim()
      if (after && after !== '|') instrLines.push(after)
      continue
    }
    if (inInstructions) {
      if (line.startsWith('  ') || line.startsWith('\t')) {
        instrLines.push(line.replace(/^( {2}|\t)/, ''))
      } else {
        inInstructions = false
      }
    }
    if (inInstructions) continue

    const m = line.match(/^([a-z_]+):\s*['"|]?(.+)?['"|]?\s*$/)
    if (!m) continue
    const [, key, raw] = m
    const val = (raw ?? '').replace(/['"|]/g, '').trim()
    if      (key === 'title')           result.title           = val
    else if (key === 'id')              result.id              = val
    else if (key === 'description')     result.description     = val
    else if (key === 'schedule')        result.schedule        = val
    else if (key === 'permission_mode') result.permission_mode = val as 'autonomous' | 'supervised'
    else if (key === 'agent_role')      result.agent_role      = val
    else if (key === 'model')           result.model           = val
  }
  if (instrLines.length > 0) result.instructions = instrLines.join('\n')
  return result
}

// ─── RecipeEditor ─────────────────────────────────────────────────────────────

interface RecipeEditorProps {
  recipe:   Recipe | null    // null = create new
  onClose:  () => void
  onSaved:  () => void
}

export default function RecipeEditor({ recipe, onClose, onSaved }: RecipeEditorProps) {
  const isNew = recipe === null

  // Working copy fields for the structured form
  const [id,             setId]          = useState(recipe?.id          ?? '')
  const [title,          setTitle]       = useState(recipe?.title        ?? '')
  const [description,    setDescription] = useState(recipe?.description  ?? '')
  const [schedule,       setSchedule]    = useState(recipe?.schedule     ?? '')
  const [permMode,       setPermMode]    = useState<'autonomous' | 'supervised'>(
    recipe?.permission_mode ?? 'autonomous'
  )
  const [agentRole,      setAgentRole]   = useState(recipe?.agent_role   ?? '')
  const [model,          setModel]       = useState(recipe?.model        ?? '')
  const [instructions,   setInstructions]= useState(recipe?.instructions ?? DEFAULT_INSTRUCTIONS)

  // Auto-derive slug from title when creating
  useEffect(() => {
    if (isNew && title) {
      setId(slugify(title))
    }
  }, [title, isNew])

  const [saving,   setSaving]  = useState(false)
  const [deleting, setDeleting]= useState(false)
  const [error,    setError]   = useState<string | null>(null)
  const [backups,  setBackups] = useState<RecipeBackup[]>([])
  const [showHistory, setShowHistory] = useState(false)
  const [restoring,   setRestoring]   = useState<string | null>(null)
  const [forceEdit,   setForceEdit]   = useState(false)

  // AI generation panel state
  const [showGenPanel,   setShowGenPanel]   = useState(isNew)
  const [genWorkerType,  setGenWorkerType]  = useState('custom')
  const [genDesc,        setGenDesc]        = useState('')
  const [generating,     setGenerating]     = useState(false)
  const [generatedYaml,  setGeneratedYaml]  = useState<string | null>(null)
  const [genError,       setGenError]       = useState<string | null>(null)

  // Load backup history when editing
  useEffect(() => {
    if (!isNew && recipe?.id) {
      invoke<RecipeBackup[]>('list_recipe_backups', { recipeId: recipe.id })
        .then(setBackups)
        .catch(() => setBackups([]))
    }
  }, [isNew, recipe?.id])

  const buildRecipe = (): Recipe => ({
    version:         recipe?.version ?? '1.0.0',
    id,
    title,
    description:     description.trim() || null,
    schedule:        schedule.trim()    || null,
    permission_mode: permMode,
    agent_role:      agentRole.trim()   || null,
    model:           model.trim()       || null,
    is_system:       recipe?.is_system  ?? false,
    instructions:    instructions.trim(),
  })

  const handleSave = async () => {
    if (!id.trim() || !title.trim() || !instructions.trim()) {
      setError('ID, title, and instructions are required')
      return
    }
    setSaving(true)
    setError(null)
    try {
      const payload = buildRecipe()
      if (isNew) {
        await invoke('create_recipe', { recipe: payload })
      } else {
        await invoke('update_recipe', { recipe: payload, force: forceEdit })
      }
      onSaved()
      onClose()
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setSaving(false)
    }
  }

  const handleDelete = async () => {
    if (!recipe) return
    if (!confirm(`Delete recipe "${recipe.title}"? A backup will be created first.`)) return
    setDeleting(true)
    setError(null)
    try {
      await invoke('delete_recipe', { recipeId: recipe.id })
      onSaved()
      onClose()
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setDeleting(false)
    }
  }

  const handleRestore = async (backup: RecipeBackup) => {
    if (!recipe) return
    if (!confirm(`Restore backup from ${backup.timestamp}? Current version will be backed up first.`)) return
    setRestoring(backup.filename)
    try {
      await invoke('restore_recipe_backup', { recipeId: recipe.id, backupFilename: backup.filename })
      onSaved()
      onClose()
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setRestoring(null)
    }
  }

  const handleGenerate = async () => {
    setGenerating(true)
    setGenError(null)
    setGeneratedYaml(null)
    try {
      const yaml = await invoke<string>('generate_recipe_yaml', {
        workerType:  genWorkerType,
        name:        title || genWorkerType,
        description: genDesc,
      })
      setGeneratedYaml(yaml)
    } catch (e) {
      setGenError(e instanceof Error ? e.message : String(e))
    } finally {
      setGenerating(false)
    }
  }

  const applyGenerated = () => {
    if (!generatedYaml) return
    const parsed = parseRecipeYaml(generatedYaml)
    if (parsed.title       && isNew)  setTitle(parsed.title)
    if (parsed.id          && isNew)  setId(parsed.id)
    if (parsed.description)           setDescription(parsed.description)
    if (parsed.schedule)              setSchedule(parsed.schedule)
    if (parsed.permission_mode)       setPermMode(parsed.permission_mode)
    if (parsed.agent_role)            setAgentRole(parsed.agent_role)
    if (parsed.model)                 setModel(parsed.model)
    if (parsed.instructions)          setInstructions(parsed.instructions)
    setShowGenPanel(false)
    setGeneratedYaml(null)
  }

  const isSystem  = recipe?.is_system ?? false
  const canDelete = !isSystem

  return (
    <div style={{
      position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.75)',
      display: 'flex', alignItems: 'flex-start', justifyContent: 'center',
      zIndex: 1000, overflowY: 'auto', padding: '40px 16px',
    }}>
      <div style={{
        background: 'var(--bg-1)', border: '1px solid var(--border)',
        borderRadius: 8, width: '100%', maxWidth: 640,
      }}>
        {/* Header */}
        <div style={{
          display: 'flex', justifyContent: 'space-between', alignItems: 'center',
          padding: '14px 20px', borderBottom: '1px solid var(--border)',
        }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
            <h3 style={{ fontFamily: 'monospace', fontSize: 13, color: 'var(--text-1)', margin: 0 }}>
              {isNew ? 'New Recipe' : `Edit: ${recipe.title}`}
            </h3>
            {isSystem && (
              <span style={{
                fontFamily: 'monospace', fontSize: 9, padding: '2px 6px',
                borderRadius: 3, background: 'rgba(130,170,255,0.08)',
                border: '1px solid rgba(130,170,255,0.2)', color: '#82AAFF',
              }}>system</span>
            )}
          </div>
          <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
            {!isNew && backups.length > 0 && (
              <button
                onClick={() => setShowHistory(h => !h)}
                style={{
                  fontFamily: 'monospace', fontSize: 10, padding: '3px 8px',
                  border: '1px solid var(--border)', borderRadius: 3,
                  background: 'transparent', color: 'var(--text-3)', cursor: 'pointer',
                  display: 'flex', alignItems: 'center', gap: 4,
                }}
              >
                <RotateCcw size={10} />
                History ({backups.length})
              </button>
            )}
            <button
              onClick={onClose}
              style={{ background: 'none', border: 'none', color: 'var(--text-3)', cursor: 'pointer', fontSize: 16 }}
            >✕</button>
          </div>
        </div>

        {/* Version history panel */}
        {showHistory && backups.length > 0 && (
          <div style={{
            padding: '12px 20px', borderBottom: '1px solid var(--border)',
            background: 'rgba(255,255,255,0.02)',
          }}>
            <div style={{ fontFamily: 'monospace', fontSize: 10, color: 'var(--text-3)', marginBottom: 8, textTransform: 'uppercase', letterSpacing: 1 }}>
              Version History
            </div>
            {backups.map(b => (
              <div key={b.filename} style={{
                display: 'flex', justifyContent: 'space-between', alignItems: 'center',
                padding: '5px 0', borderBottom: '1px solid rgba(255,255,255,0.04)',
              }}>
                <span style={{ fontFamily: 'monospace', fontSize: 11, color: 'var(--text-2)' }}>
                  {b.timestamp}
                </span>
                <button
                  onClick={() => handleRestore(b)}
                  disabled={restoring === b.filename}
                  style={{
                    fontFamily: 'monospace', fontSize: 10, padding: '2px 8px',
                    border: '1px solid var(--border)', borderRadius: 3,
                    background: 'transparent', color: 'var(--text-3)', cursor: 'pointer',
                  }}
                >
                  {restoring === b.filename ? 'Restoring…' : 'Restore'}
                </button>
              </div>
            ))}
          </div>
        )}

        {/* Form */}
        <div style={{ padding: '16px 20px' }}>

          {/* AI generation panel — available for new recipes and editable existing ones */}
          {(!isSystem || forceEdit) && (
            <div style={{
              marginBottom: 14, borderRadius: 6,
              background: 'rgba(0,255,136,0.03)',
              border: '1px solid rgba(0,255,136,0.12)',
            }}>
              <button
                onClick={() => setShowGenPanel(p => !p)}
                style={{
                  width: '100%', padding: '8px 12px', background: 'none', border: 'none',
                  cursor: 'pointer', display: 'flex', alignItems: 'center', justifyContent: 'space-between',
                  color: 'var(--text-2)', fontFamily: 'monospace', fontSize: 11,
                }}
              >
                <span style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                  <Wand2 size={11} color="var(--accent)" />
                  Generate with AI
                </span>
                {showGenPanel ? <ChevronUp size={11} /> : <ChevronDown size={11} />}
              </button>
              {showGenPanel && (
                <div style={{ padding: '0 12px 12px' }}>
                  <div style={{ display: 'grid', gridTemplateColumns: '140px 1fr', gap: 8, marginBottom: 8 }}>
                    <div>
                      <div style={{ fontFamily: 'monospace', fontSize: 9, color: 'var(--text-3)', textTransform: 'uppercase', letterSpacing: 1, marginBottom: 4 }}>
                        Worker Type
                      </div>
                      <select
                        value={genWorkerType}
                        onChange={e => setGenWorkerType(e.target.value)}
                        style={inp}
                        title="Worker type for generation"
                      >
                        {WORKER_TYPES.map(t => <option key={t} value={t}>{t}</option>)}
                      </select>
                    </div>
                    <div>
                      <div style={{ fontFamily: 'monospace', fontSize: 9, color: 'var(--text-3)', textTransform: 'uppercase', letterSpacing: 1, marginBottom: 4 }}>
                        Description
                      </div>
                      <input
                        value={genDesc}
                        onChange={e => setGenDesc(e.target.value)}
                        placeholder="What should this agent do?"
                        title="Description of what the agent should do"
                        style={inp}
                        onKeyDown={e => e.key === 'Enter' && !generating && handleGenerate()}
                      />
                    </div>
                  </div>
                  <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                    <button
                      onClick={handleGenerate}
                      disabled={generating || !genDesc.trim()}
                      style={{
                        fontFamily: 'monospace', fontSize: 10, padding: '4px 12px',
                        border: '1px solid var(--accent)', borderRadius: 3,
                        background: 'transparent', color: 'var(--accent)', cursor: 'pointer',
                        opacity: generating || !genDesc.trim() ? 0.5 : 1,
                        display: 'flex', alignItems: 'center', gap: 5,
                      }}
                    >
                      <Wand2 size={10} />
                      {generating ? 'Generating…' : 'Generate'}
                    </button>
                    {genError && (
                      <span style={{ fontFamily: 'monospace', fontSize: 10, color: '#FF6363' }}>
                        {genError}
                      </span>
                    )}
                  </div>
                  {generatedYaml && (
                    <div style={{ marginTop: 10 }}>
                      <div style={{ fontFamily: 'monospace', fontSize: 9, color: 'var(--text-3)', textTransform: 'uppercase', letterSpacing: 1, marginBottom: 4 }}>
                        Generated YAML Preview
                      </div>
                      <textarea
                        readOnly
                        value={generatedYaml}
                        rows={8}
                        title="Generated YAML preview"
                        style={{
                          ...inp, resize: 'vertical', fontFamily: 'monospace', fontSize: 10,
                          background: 'var(--bg-2)', color: 'var(--text-2)',
                        }}
                      />
                      <button
                        onClick={applyGenerated}
                        style={{
                          marginTop: 6, fontFamily: 'monospace', fontSize: 10, padding: '4px 12px',
                          border: '1px solid var(--accent)', borderRadius: 3,
                          background: 'var(--accent-muted)', color: 'var(--accent)', cursor: 'pointer',
                        }}
                      >
                        Apply to Form
                      </button>
                    </div>
                  )}
                </div>
              )}
            </div>
          )}

          {/* System recipe warning */}
          {isSystem && !forceEdit && (
            <div style={{
              fontFamily: 'monospace', fontSize: 11, color: '#FFCB8B',
              padding: '8px 12px', borderRadius: 4, marginBottom: 14,
              background: 'rgba(255,203,139,0.06)', border: '1px solid rgba(255,203,139,0.2)',
              display: 'flex', alignItems: 'center', justifyContent: 'space-between',
            }}>
              <span>System recipe — edits will override defaults.</span>
              <button
                onClick={() => setForceEdit(true)}
                style={{
                  fontFamily: 'monospace', fontSize: 10, padding: '2px 8px',
                  border: '1px solid #FFCB8B', borderRadius: 3,
                  background: 'transparent', color: '#FFCB8B', cursor: 'pointer',
                }}
              >
                I understand, edit anyway
              </button>
            </div>
          )}

          {/* ID + Title row */}
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12, marginBottom: 12 }}>
            <Field label="Title">
              <input
                value={title}
                onChange={e => setTitle(e.target.value)}
                placeholder="My Worker Recipe"
                disabled={isSystem && !forceEdit}
                style={inp}
              />
            </Field>
            <Field label="ID (slug)">
              <input
                value={id}
                onChange={e => setId(e.target.value)}
                placeholder="my-worker-recipe"
                disabled={!isNew || (isSystem && !forceEdit)}
                style={{ ...inp, fontFamily: 'monospace', opacity: !isNew ? 0.6 : 1 }}
              />
            </Field>
          </div>

          {/* Description */}
          <Field label="Description (optional)">
            <input
              value={description}
              onChange={e => setDescription(e.target.value)}
              placeholder="What this recipe does"
              disabled={isSystem && !forceEdit}
              style={inp}
            />
          </Field>

          {/* Schedule + Permission + Agent Role + Model row */}
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr 1fr', gap: 10, marginBottom: 12 }}>
            <Field label="Schedule (cron)">
              <input
                value={schedule}
                onChange={e => setSchedule(e.target.value)}
                placeholder="*/2 * * * *"
                disabled={isSystem && !forceEdit}
                style={inp}
              />
            </Field>
            <Field label="Permission Mode">
              <select
                value={permMode}
                onChange={e => setPermMode(e.target.value as 'autonomous' | 'supervised')}
                disabled={isSystem && !forceEdit}
                style={inp}
                title="Permission Mode"
              >
                <option value="autonomous">Autonomous</option>
                <option value="supervised">Supervised</option>
              </select>
            </Field>
            <Field label="Agent Role">
              <input
                value={agentRole}
                onChange={e => setAgentRole(e.target.value)}
                placeholder="worker-backend"
                disabled={isSystem && !forceEdit}
                style={inp}
              />
            </Field>
            <Field label="Model Override">
              <input
                value={model}
                onChange={e => setModel(e.target.value)}
                placeholder="(default)"
                disabled={isSystem && !forceEdit}
                style={inp}
              />
            </Field>
          </div>

          {/* Instructions */}
          <Field label="Instructions (System Prompt)">
            <textarea
              value={instructions}
              onChange={e => setInstructions(e.target.value)}
              rows={14}
              disabled={isSystem && !forceEdit}
              placeholder="Enter the agent's system prompt instructions…"
              style={{
                ...inp,
                resize: 'vertical', minHeight: 200,
                lineHeight: 1.6, fontFamily: 'monospace',
              }}
            />
          </Field>

          {error && (
            <div style={{ fontFamily: 'monospace', fontSize: 11, color: '#FF6363', marginBottom: 10 }}>
              {error}
            </div>
          )}

          {/* Footer buttons */}
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginTop: 16 }}>
            <div>
              {canDelete && !isNew && (
                <button
                  onClick={handleDelete}
                  disabled={deleting}
                  style={{
                    fontFamily: 'monospace', fontSize: 11, padding: '5px 12px',
                    border: '1px solid #AA3333', borderRadius: 4,
                    background: 'transparent', color: '#FF6666', cursor: 'pointer',
                  }}
                >
                  {deleting ? 'Deleting…' : 'Delete Recipe'}
                </button>
              )}
            </div>
            <div style={{ display: 'flex', gap: 8 }}>
              <button onClick={onClose} style={cancelBtn}>Cancel</button>
              <button
                onClick={handleSave}
                disabled={saving || (isSystem && !forceEdit)}
                style={{
                  fontFamily: 'monospace', fontSize: 11, padding: '5px 14px',
                  border: '1px solid var(--accent)', borderRadius: 4,
                  background: 'transparent', color: 'var(--accent)', cursor: 'pointer',
                  opacity: saving || (isSystem && !forceEdit) ? 0.4 : 1,
                }}
              >
                {saving ? 'Saving…' : isNew ? 'Create Recipe' : 'Save Changes'}
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div style={{ marginBottom: 10 }}>
      <label style={{
        display: 'block', fontFamily: 'monospace', fontSize: 9,
        color: 'var(--text-3)', textTransform: 'uppercase',
        letterSpacing: 1, marginBottom: 4,
      }}>
        {label}
      </label>
      {children}
    </div>
  )
}

const inp: React.CSSProperties = {
  width: '100%', padding: '5px 8px', fontFamily: 'inherit', fontSize: 11,
  background: 'var(--bg-2)', border: '1px solid var(--border)',
  borderRadius: 4, color: 'var(--text-1)', outline: 'none', boxSizing: 'border-box',
}

const cancelBtn: React.CSSProperties = {
  fontFamily: 'monospace', fontSize: 11, padding: '5px 12px',
  border: '1px solid var(--border)', borderRadius: 4,
  background: 'transparent', color: 'var(--text-2)', cursor: 'pointer',
}
