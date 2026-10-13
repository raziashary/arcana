// P4-60 only: preserve the production Vite config while forcing Vite's env
// loader to a launcher-owned empty directory. Normal builds never use this.
import { readdirSync, statSync } from 'node:fs'
import { isAbsolute, resolve } from 'node:path'
import { mergeConfig } from 'vite'
import baseConfig from '../vite.config'

const suppliedEnvDir = process.env.ARCANA_E2E_VITE_ENV_DIR
if (!suppliedEnvDir || !isAbsolute(suppliedEnvDir)) {
  throw new Error('The isolated P4-60 Vite env directory must be an absolute path.')
}

const envDir = resolve(suppliedEnvDir)
if (!statSync(envDir).isDirectory() || readdirSync(envDir).length !== 0) {
  throw new Error('The isolated P4-60 Vite env directory must exist and be empty.')
}

export default mergeConfig(baseConfig, { envDir })
