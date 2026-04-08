/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    './index.html',
    './src/**/*.{ts,tsx}',
  ],
  theme: {
    extend: {
      colors: {
        'bg-app':     '#0A0A0A',
        'bg-surface': '#0D0D0D',
        'bg-card':    '#111111',
        'border-1':   '#1a1a1a',
        'border-2':   '#222222',
        accent:       '#00FF88',
        'accent-muted': '#00FF8812',
        'accent-border': '#00FF8825',
        danger:       '#FF4444',
        warning:      '#FFAA00',
        info:         '#4477FF',
        'text-1':     '#888888',
        'text-2':     '#555555',
        'text-3':     '#444444',
        'text-4':     '#333333',
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

