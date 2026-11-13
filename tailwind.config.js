/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    './index.html',
    './src/**/*.{ts,tsx}',
  ],
  theme: {
    extend: {
      // Colour is defined once, in src/styles/globals.css (:root). These names
      // point at those variables so a utility class cannot carry its own hex.
      colors: {
        'bg-app':        'var(--bg-app)',
        'bg-surface':    'var(--bg-surface)',
        'bg-card':       'var(--bg-card)',
        'bg-2':          'var(--bg-2)',
        'bg-input':      'var(--bg-input)',
        'bg-selected':   'var(--bg-selected)',
        'bg-hover':      'var(--bg-hover)',
        'border-1':      'var(--border-1)',
        'border-2':      'var(--border-2)',
        accent:          'var(--accent)',
        'accent-muted':  'var(--accent-muted)',
        'accent-border': 'var(--accent-border)',
        aqua:            'var(--aqua)',
        success:         'var(--success)',
        danger:          'var(--danger)',
        warning:         'var(--warning)',
        info:            'var(--info)',
        gold:            'var(--gold)',
        'text-1':        'var(--text-1)',
        'text-2':        'var(--text-2)',
        'text-3':        'var(--text-3)',
        'text-4':        'var(--text-4)',
      },
      fontFamily: {
        mono: ['IBM Plex Mono', 'monospace'],
        sans: ['IBM Plex Sans', 'sans-serif'],
      },
      borderRadius: {
        DEFAULT: '2px',
        none: '0',
      },
      fontSize: {
        '2xs': '9px',
        xs:    '10px',
        sm:    '11px',
        base:  '12px',
        md:    '13px',
      },
    },
  },
  plugins: [],
}

