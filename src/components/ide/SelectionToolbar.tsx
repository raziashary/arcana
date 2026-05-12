// SelectionToolbar — floating pill that appears when the user selects code lines.
// Provides three actions: Ask PA, Delegate to fleet, or plain Note.

import './Annotations.css'

export type AnnotationIntent = 'ask' | 'delegate' | 'note'

interface SelectionToolbarProps {
  top:      number   // px from top of CodeViewer container
  left:     number   // px from left of CodeViewer container
  onAction: (intent: AnnotationIntent) => void
}

export function SelectionToolbar({ top, left, onAction }: SelectionToolbarProps) {
  return (
    <div
      className="selection-toolbar"
      style={{ top, left }}
      // Prevent toolbar mousedown from clearing the Monaco selection
      onMouseDown={(e) => e.preventDefault()}
    >
      <button
        className="selection-toolbar-btn selection-toolbar-btn--ask"
        onClick={() => onAction('ask')}
        title="Ask the PA about this code"
      >
        Ask PA
      </button>
      <span className="selection-toolbar-sep" />
      <button
        className="selection-toolbar-btn selection-toolbar-btn--delegate"
        onClick={() => onAction('delegate')}
        title="Delegate this code to the fleet"
      >
        Delegate
      </button>
      <span className="selection-toolbar-sep" />
      <button
        className="selection-toolbar-btn selection-toolbar-btn--note"
        onClick={() => onAction('note')}
        title="Add a note to this code"
      >
        Note
      </button>
    </div>
  )
}
