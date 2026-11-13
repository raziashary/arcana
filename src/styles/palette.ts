// Literal colours for the consumers that cannot read a CSS variable.
//
// The rest of the app takes every colour from the tokens in globals.css (:root).
// Three.js materials, the d3 graph's community colours and Monaco's theme need
// real colour values, so they live here and nowhere else. `TG-P6-04` fails if a
// hex literal appears in any other source file. The values are named once, and
// consumers import the name, so a colour is never retyped.

/** The code city: a three.js night scene with its own dark palette (D-016). */
export const CITY = {
  stage: '#050508',
  ground: '#0a0a0e',
  gridMajor: '#16161e',
  gridMinor: '#101018',
  ambient: '#99aabb',
  fillLight: '#4466aa',
  hemiSky: '#3a4a7a',
  hemiGround: '#2a2a3a',
  panel: '#111111',
  panelBorder: '#222222',
  menu: '#1a1a1f',
  inkBright: '#f0f0f0',
  ink: '#cccccc',
  inkMuted: '#888888',
  inkDim: '#666666',
  inkFaint: '#555555',
  black: '#000000',
  white: '#ffffff',
  link: '#4477ff',
  ok: '#00ff88',
  danger: '#ff4444',
  dangerSoft: '#ff8888',
  riskHigh: '#ff2222',
  riskMedium: '#ff8800',
  riskLow: '#aaff00',
  riskUntested: '#ff0000',
  riskUntestedOverlay: '#ff3333',
  worker: '#ffcc00',
  workerGlow: '#aa8800',
  god: '#ffcc44',
  kindFunction: '#5588dd',
  kindFunctionGlow: '#1a3a7a',
  kindClass: '#55bb55',
  kindClassGlow: '#1a551a',
  kindModule: '#8855cc',
  kindModuleGlow: '#331166',
  kindTest: '#ff9933',
  kindTestGlow: '#553300',
  kindType: '#44aaaa',
  kindTypeGlow: '#114444',
  kindInterface: '#cc9944',
  kindInterfaceGlow: '#553311',
  kindConstant: '#cc6677',
  kindConstantGlow: '#441122',
  kindVariable: '#77aacc',
  kindVariableGlow: '#1a3a55',
  kindDefault: '#7a7a8a',
  kindDefaultGlow: '#1a1a22',
  roadImportsGlow: '#5588ff',
  roadHighlightGlow: '#aaccff',
  roadDepends: '#ddaa33',
  roadDependsGlow: '#ffcc55',
  roadImportsEmissive: '#6699ff',
  bridgeInherits: '#9966cc',
  bridgeInheritsGlow: '#3d1166',
  bridgeImplements: '#cc6622',
  bridgeImplementsGlow: '#882200',
  roadAggregate: '#3a5fff',
  roadSelection: '#6e5a48',
  cat0: '#4e79a7',
  cat1: '#f28e2b',
  cat2: '#59a14f',
  cat3: '#e15759',
  cat4: '#76b7b2',
  cat5: '#edc948',
  cat6: '#b07aa1',
  cat7: '#ff9da7',
  cat8: '#9c755f',
  cat9: '#bab0ac',
} as const

/** Ten categorical colours for graph communities and city districts (Tableau 10). */
export const CATEGORICAL: readonly string[] = [
  CITY.cat0, CITY.cat1, CITY.cat2, CITY.cat3, CITY.cat4,
  CITY.cat5, CITY.cat6, CITY.cat7, CITY.cat8, CITY.cat9,
]

/** Edge-kind colours on the light Graph tab. */
export const GRAPH_EDGE: Record<string, string> = {
  CALLS:                '#3C7A12',
  IMPORTS_FROM:         '#2D6CDF',
  INHERITS:             CITY.cat6,
  IMPLEMENTS:           CITY.cat4,
  TESTED_BY:            CITY.cat1,
  DEPENDS_ON:           '#B8941F',
  REFERENCES:           '#8A90A8',
  CONTAINS:             '#BEBDF7',
  SEMANTICALLY_SIMILAR: CITY.cat8,
}

/** The Monaco editor theme, light, matching the tokens in globals.css. */
export const MONACO_THEME_NAME = 'arcana-light'
export const MONACO_THEME = {
  base: 'vs' as const,
  inherit: true,
  rules: [
    { token: '',        foreground: '0E1131', background: 'FFFFFF' },
    { token: 'comment', foreground: '6B7390', fontStyle: 'italic' },
    { token: 'keyword', foreground: '2D6CDF' },
    { token: 'string',  foreground: '8A5A00' },
    { token: 'number',  foreground: 'A15C00' },
    { token: 'type',    foreground: '1F7A6D' },
    { token: 'function',foreground: '2D6CDF' },
    { token: 'variable',foreground: '0E1131' },
  ],
  colors: {
    'editor.background':                 '#FFFFFF',
    'editor.foreground':                 '#0E1131',
    'editor.lineHighlightBackground':    '#EDECF9',
    'editorLineNumber.foreground':       '#7C83A0',
    'editorLineNumber.activeForeground': '#2D6CDF',
    'editorCursor.foreground':           '#2D6CDF',
    'editor.selectionBackground':        '#CFE3FF',
    'editor.inactiveSelectionBackground':'#CFE3FF80',
    'editorWidget.background':           '#FFFFFF',
    'editorWidget.border':               '#DBDBFC',
  },
}
