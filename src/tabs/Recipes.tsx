// Recipes tab — list, create, edit, delete YAML recipes with versioning.
// Phase 2.1: read-only viewer.
// Phase 2.2: full CRUD via RecipeEditor modal + backup/restore versioning.
// See: DEVELOPMENT_PLAN.md Phase 2.2 — RECIPES TAB UI

import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { RefreshCw, ChevronDown, ChevronRight, Play, Plus, Pencil } from 'lucide-react'
import RecipeEditor, { type Recipe } from '../components/recipes/RecipeEditor'

function RecipeCard({ recipe, onEdit }: { recipe: Recipe; onEdit: (r: Recipe) => void }) {
  const [expanded, setExpanded] = useState(false)

  return (
    <div
      className="rounded font-mono text-xs"
      style={{
        background: 'var(--bg-surface)',
        border:     '1px solid var(--border-2)',
        overflow:   'hidden',
      }}
    >
      <div
        className="flex items-start gap-2 p-3 cursor-pointer"
        onClick={() => setExpanded(e => !e)}
      >
        <span style={{ color: 'var(--text-4)', marginTop: '1px' }}>
          {expanded ? <ChevronDown size={11} /> : <ChevronRight size={11} />}
        </span>

        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 mb-1 flex-wrap">
            <span style={{ color: 'var(--text-1)', fontWeight: 600 }}>{recipe.title}</span>
            {recipe.is_system && (
              <span
                className="px-1 rounded"
                style={{ background: 'var(--bg-card)', color: 'var(--accent)', fontSize: '9px' }}
              >
                system
              </span>
            )}
            {recipe.agent_role && (
              <span
                className="px-1 rounded"
                style={{ background: 'var(--bg-card)', color: 'var(--text-3)', fontSize: '9px' }}
              >
                {recipe.agent_role}
              </span>
            )}
            <span
              className="px-1 rounded"
              style={{
                background: 'var(--bg-card)',
                color:       recipe.permission_mode === 'autonomous' ? 'var(--success)' : 'var(--warning)',
                fontSize:    '9px',
              }}
            >
              {recipe.permission_mode}
            </span>
          </div>

          <div className="flex items-center gap-3" style={{ color: 'var(--text-4)', fontSize: '10px' }}>
            <span>{recipe.id}</span>
            {recipe.schedule && <span>⏱ {recipe.schedule}</span>}
            {recipe.model    && <span>🤖 {recipe.model}</span>}
          </div>

          {recipe.description && (
            <div className="mt-1 leading-relaxed" style={{ color: 'var(--text-3)', fontSize: '11px' }}>
              {recipe.description}
            </div>
          )}
        </div>
      </div>

      {/* Action bar */}
      <div
        className="px-3 pb-2 flex items-center justify-between"
        style={{ borderTop: '1px solid var(--border-1)', paddingTop: '6px' }}
      >
        <span style={{ color: 'var(--text-4)', fontSize: '9px' }}>
          {recipe.schedule ?? 'manual'}
        </span>
        <div className="flex items-center gap-1.5">
          <button
            onClick={e => { e.stopPropagation(); onEdit(recipe) }}
            className="flex items-center gap-1 rounded"
            style={{
              fontSize: '9px', padding: '2px 6px',
              background: 'var(--bg-card)', border: '1px solid var(--border-2)',
              color: 'var(--text-3)', cursor: 'pointer',
            }}
          >
            <Pencil size={8} />
            Edit
          </button>
          <button
            disabled
            title="Recipe execution engine coming in Phase 2.3"
            className="flex items-center gap-1 rounded opacity-40"
            style={{
              fontSize: '9px', padding: '2px 6px',
              background: 'var(--bg-card)', border: '1px solid var(--border-2)',
              color: 'var(--text-3)', cursor: 'not-allowed',
            }}
          >
            <Play size={8} />
            Run
          </button>
        </div>
      </div>

      {expanded && (
        <div
          className="px-3 pb-3"
          style={{ borderTop: '1px solid var(--border-1)' }}
        >
          <div
            className="mb-1 pt-2"
            style={{ color: 'var(--text-4)', fontSize: '9px', letterSpacing: '0.05em' }}
          >
            INSTRUCTIONS
          </div>
          <pre
            className="whitespace-pre-wrap leading-relaxed overflow-auto"
            style={{
              color:      'var(--text-2)',
              fontSize:   '10px',
              maxHeight:  '300px',
              fontFamily: 'inherit',
              background: 'var(--bg-card)',
              padding:    '8px',
              borderRadius: '4px',
              border:     '1px solid var(--border-2)',
            }}
          >
            {recipe.instructions}
          </pre>
        </div>
      )}
    </div>
  )
}

export default function RecipesTab() {
  const [recipes,       setRecipes]       = useState<Recipe[]>([])
  const [isLoading,     setIsLoading]     = useState(false)
  const [error,         setError]         = useState<string | null>(null)
  const [editingRecipe, setEditingRecipe] = useState<Recipe | null | undefined>(undefined)
  // undefined = closed, null = new recipe, Recipe = edit mode

  const loadRecipes = async () => {
    setIsLoading(true)
    setError(null)
    try {
      const result = await invoke<Recipe[]>('list_recipes')
      setRecipes(result)
    } catch (e) {
      setError(String(e))
      console.error('Failed to load recipes:', e)
    } finally {
      setIsLoading(false)
    }
  }

  useEffect(() => { loadRecipes() }, [])

  const systemRecipes = recipes.filter(r => r.is_system)
  const userRecipes   = recipes.filter(r => !r.is_system)

  return (
    <div
      className="flex flex-col flex-1 overflow-hidden"
      style={{ background: 'var(--bg-base)' }}
    >
      {/* Toolbar */}
      <div
        className="flex items-center gap-2 px-4 py-2 shrink-0 font-mono text-xs"
        style={{ borderBottom: '1px solid var(--border-1)', background: 'var(--bg-surface)' }}
      >
        <span style={{ color: 'var(--text-2)' }}>Recipes</span>
        <span
          className="px-1 rounded"
          style={{ background: 'var(--bg-card)', color: 'var(--text-4)', fontSize: '10px' }}
        >
          {recipes.length} total
        </span>
        <div className="flex-1" />
        {isLoading && <span style={{ color: 'var(--text-4)' }}>loading…</span>}
        {error    && <span style={{ color: 'var(--error)', fontSize: '10px' }}>{error}</span>}
        <button
          onClick={() => setEditingRecipe(null)}
          className="flex items-center gap-1 px-2 py-1 rounded"
          style={{
            background: 'var(--accent-muted)', border: '1px solid var(--accent-border)',
            color: 'var(--accent)', cursor: 'pointer', fontSize: '11px',
          }}
        >
          <Plus size={11} />
          <span>New Recipe</span>
        </button>
        <button
          onClick={loadRecipes}
          className="flex items-center gap-1 px-2 py-1 rounded transition-opacity hover:opacity-80"
          style={{
            background: 'var(--bg-card)',
            border:     '1px solid var(--border-2)',
            color:      'var(--text-3)',
            cursor:     'pointer',
            fontSize:   '11px',
          }}
        >
          <RefreshCw size={11} />
          <span>Refresh</span>
        </button>
      </div>

      {/* Recipe list */}
      <div className="flex-1 overflow-y-auto p-4">
        {systemRecipes.length > 0 && (
          <div className="mb-4">
            <div
              className="font-mono text-xs mb-2 px-1"
              style={{ color: 'var(--text-4)', letterSpacing: '0.05em' }}
            >
              SYSTEM RECIPES
            </div>
            <div className="flex flex-col gap-2">
              {systemRecipes.map(r => <RecipeCard key={r.id} recipe={r} onEdit={setEditingRecipe} />)}
            </div>
          </div>
        )}

        {userRecipes.length > 0 && (
          <div className="mb-4">
            <div
              className="font-mono text-xs mb-2 px-1"
              style={{ color: 'var(--text-4)', letterSpacing: '0.05em' }}
            >
              USER RECIPES
            </div>
            <div className="flex flex-col gap-2">
              {userRecipes.map(r => <RecipeCard key={r.id} recipe={r} onEdit={setEditingRecipe} />)}
            </div>
          </div>
        )}

        {!isLoading && recipes.length === 0 && (
          <div
            className="flex flex-col items-center justify-center h-full font-mono text-xs"
            style={{ color: 'var(--text-4)' }}
          >
            No recipes found in ~/.autodev/recipes/
          </div>
        )}
      </div>

      {/* Recipe editor modal */}
      {editingRecipe !== undefined && (
        <RecipeEditor
          recipe={editingRecipe}
          onClose={() => setEditingRecipe(undefined)}
          onSaved={() => { setEditingRecipe(undefined); loadRecipes() }}
        />
      )}
    </div>
  )
}
