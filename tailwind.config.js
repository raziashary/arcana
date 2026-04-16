/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    './index.html',
    './src/**/*.{ts,tsx}',
  ],
  theme: {
    extend: {
      colors: {
        /* Backgrounds — Winter is Coming */
        'bg-app':      '#011627',
        'bg-surface':  '#00233B',
        'bg-card':     '#01111D',
        'bg-2':        '#022C4A',
        'bg-input':    '#0D2137',
        'bg-selected': '#0B3A5C',
        'bg-hover':    '#022C4A',
        /* Borders */
        'border-1':    '#0D3652',
        'border-2':    '#1A527A',
        /* Accent system */
        accent:        '#82AAFF',
        'accent-muted':  '#82AAFF15',
        'accent-border': '#82AAFF30',
        aqua:          '#7FDBCA',
        success:       '#ADDB67',
        danger:        '#FF6363',
        warning:       '#FFCB8B',
        info:          '#C792EA',
        gold:          '#ECC48D',
        /* Typography */
        'text-1':      '#D6DEEB',
        'text-2':      '#637777',
        'text-3':      '#3D5A6E',
        'text-4':      '#1A3A50',
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

