// Monaco Editor local worker setup.
//
// Rationale: @monaco-editor/react defaults to loading Monaco + its web workers
// from a CDN (cdn.jsdelivr.net). Tauri's CSP (`script-src 'self'`) blocks
// external scripts, so the editor hangs on "Loading..." forever.
//
// Fix: Import Monaco's workers via Vite's `?worker` import and hand them to
// Monaco through `self.MonacoEnvironment`. Vite bundles each worker into a
// local JS file served from the app origin, so CSP is satisfied.
//
// This file must be imported once, at app startup (see main.tsx).

import editorWorker  from 'monaco-editor/esm/vs/editor/editor.worker?worker'
import jsonWorker    from 'monaco-editor/esm/vs/language/json/json.worker?worker'
import cssWorker     from 'monaco-editor/esm/vs/language/css/css.worker?worker'
import htmlWorker    from 'monaco-editor/esm/vs/language/html/html.worker?worker'
import tsWorker      from 'monaco-editor/esm/vs/language/typescript/ts.worker?worker'

// eslint-disable-next-line @typescript-eslint/no-explicit-any
;(self as any).MonacoEnvironment = {
  getWorker(_workerId: string, label: string) {
    switch (label) {
      case 'json':
        return new jsonWorker()
      case 'css':
      case 'scss':
      case 'less':
        return new cssWorker()
      case 'html':
      case 'handlebars':
      case 'razor':
        return new htmlWorker()
      case 'typescript':
      case 'javascript':
        return new tsWorker()
      default:
        return new editorWorker()
    }
  },
}
