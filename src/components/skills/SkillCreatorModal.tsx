// SkillCreatorModal — Create new custom skills
// Phase 3.1

import { useState } from 'react'
import { X, AlertCircle } from 'lucide-react'
import { useSkillStore } from '../../stores/skillStore'

interface Props {
  onClose: () => void
}

export default function SkillCreatorModal({ onClose }: Props) {
  const store = useSkillStore()
  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [category, setCategory] = useState<'custom' | 'coding' | 'debugging' | 'devops' | 'fleet'>('custom')
  const [template, setTemplate] = useState('')
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(false)

  async function handleCreate() {
    if (!name.trim()) {
      setError('Skill name is required')
      return
    }
    if (!description.trim()) {
      setError('Description is required')
      return
    }
    if (!template.trim()) {
      setError('Template/content is required')
      return
    }

    setLoading(true)
    setError('')
    try {
      // Reload skills from disk to pick up new skill
      await store.reloadFromDisk()
      onClose()
    } catch (e) {
      setError(String(e))
    } finally {
      setLoading(false)
    }
  }

  return (
    <div
      className="fixed inset-0 bg-black/50 flex items-center justify-center z-50"
      onClick={e => e.target === e.currentTarget && onClose()}
    >
      <div
        className="rounded-lg w-full max-w-2xl max-h-[90vh] overflow-hidden flex flex-col"
        style={{ background: 'var(--bg-1)', border: '1px solid var(--border)' }}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4" style={{ borderBottom: '1px solid var(--border)' }}>
          <div>
            <div className="font-mono text-sm font-semibold" style={{ color: 'var(--text-1)' }}>
              Create Skill
            </div>
            <div className="text-xs mt-0.5" style={{ color: 'var(--text-3)' }}>
              Add a new custom skill to ~/.arcana/skills/
            </div>
          </div>
          <button title="Close" onClick={onClose} className="p-1 rounded hover:opacity-70">
            <X size={16} style={{ color: 'var(--text-3)' }} />
          </button>
        </div>

        {/* Form */}
        <div className="overflow-y-auto px-5 py-4 space-y-4 flex-1">
          {error && (
            <div className="flex items-start gap-2 text-xs p-3 rounded" style={{ background: 'rgba(255,99,99,0.1)', color: 'var(--danger)' }}>
              <AlertCircle size={14} className="mt-0.5 shrink-0" />
              <span>{error}</span>
            </div>
          )}

          {/* Name */}
          <div>
            <label className="text-xs font-semibold" style={{ color: 'var(--text-2)' }}>
              Skill Name <span style={{ color: 'var(--danger)' }}>*</span>
            </label>
            <input
              type="text"
              placeholder="e.g., my-custom-workflow"
              value={name}
              onChange={e => setName(e.target.value)}
              className="w-full mt-1 px-3 py-2 rounded text-xs font-mono"
              style={{ background: 'var(--bg-input)', border: '1px solid var(--border)', color: 'var(--text-1)' }}
            />
            <div className="text-xs mt-1" style={{ color: 'var(--text-3)' }}>
              Lowercase, hyphens only. Becomes: /my-custom-workflow
            </div>
          </div>

          {/* Description */}
          <div>
            <label className="text-xs font-semibold" style={{ color: 'var(--text-2)' }}>
              Description <span style={{ color: 'var(--danger)' }}>*</span>
            </label>
            <input
              type="text"
              placeholder="What does this skill do?"
              value={description}
              onChange={e => setDescription(e.target.value)}
              className="w-full mt-1 px-3 py-2 rounded text-xs font-mono"
              style={{ background: 'var(--bg-input)', border: '1px solid var(--border)', color: 'var(--text-1)' }}
            />
          </div>

          {/* Category */}
          <div>
            <label className="text-xs font-semibold" style={{ color: 'var(--text-2)' }}>
              Category
            </label>
            <select
              title="Select category"
              value={category}
              onChange={e => setCategory(e.target.value as any)}
              className="w-full mt-1 px-3 py-2 rounded text-xs font-mono"
              style={{ background: 'var(--bg-input)', border: '1px solid var(--border)', color: 'var(--text-1)' }}
            >
              <option value="custom">Custom</option>
              <option value="coding">Coding</option>
              <option value="debugging">Debugging</option>
              <option value="devops">DevOps</option>
              <option value="fleet">Fleet</option>
            </select>
          </div>

          {/* Template */}
          <div>
            <label className="text-xs font-semibold" style={{ color: 'var(--text-2)' }}>
              Template / Content <span style={{ color: 'var(--danger)' }}>*</span>
            </label>
            <textarea
              placeholder="Markdown template with parameter substitution. Example: ---&#10;name: my-skill&#10;description: My custom skill&#10;parameters:&#10;  - name: target&#10;    type: string&#10;    required: true&#10;---&#10;&#10;## Steps&#10;1. Analyze the target&#10;2. Execute plan&#10;3. Report results"
              value={template}
              onChange={e => setTemplate(e.target.value)}
              className="w-full mt-1 px-3 py-2 rounded text-xs font-mono"
              style={{ background: 'var(--bg-input)', border: '1px solid var(--border)', color: 'var(--text-1)', minHeight: '200px' }}
            />
            <div className="text-xs mt-1" style={{ color: 'var(--text-3)' }}>
              YAML frontmatter (---) + Markdown body. Use double braces for parameter substitution.
            </div>
          </div>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-2 px-5 py-3" style={{ borderTop: '1px solid var(--border)' }}>
          <button
            onClick={onClose}
            className="px-3 py-1.5 rounded text-xs font-mono"
            style={{ background: 'transparent', color: 'var(--text-2)', border: '1px solid var(--border)' }}
          >
            Cancel
          </button>
          <button
            onClick={handleCreate}
            disabled={loading}
            className="px-3 py-1.5 rounded text-xs font-mono disabled:opacity-50"
            style={{ background: 'var(--accent)', color: '#011627' }}
          >
            {loading ? 'Creating...' : 'Create Skill'}
          </button>
        </div>
      </div>
    </div>
  )
}
