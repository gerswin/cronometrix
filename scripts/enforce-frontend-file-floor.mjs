#!/usr/bin/env node
// scripts/enforce-frontend-file-floor.mjs
//
// Vitest's threshold config supports a glob-keyed block, e.g.
//   '**/*.{ts,tsx}': { lines: 70, branches: 60, functions: 70, statements: 70 }
// but Vitest evaluates a glob-keyed threshold over the AGGREGATE of every
// file the glob matches, not file by file. When the glob covers the whole
// `coverage.include` set (as it used to in frontend/vitest.config.ts), that
// "per-file floor" measures roughly the same thing as the project-wide gate,
// only with lower numbers -- so it never binds, and a file far below 70%
// still exits 0. `perFile: true` does not fix this: it applies the PROJECT
// thresholds (90/85/90/90) to every file, which is stricter than the
// intended floor, not equal to it. "project 90, per-file 70" is simply not
// expressible in Vitest's threshold config.
//
// This script post-processes frontend/coverage/coverage-summary.json (the
// artifact the 'json-summary' reporter already emits) the same way
// scripts/enforce-coverage-floor.sh post-processes the backend's lcov.info:
// read the artifact after the test run, walk it file by file, fail loudly
// naming every offender.
//
// Governed files = frontend/vitest.config.ts's `coverage.include` minus
// `coverage.exclude`. Those two glob lists are NOT duplicated here -- they
// are loaded straight out of vitest.config.ts via Vite's own config loader,
// so this script and the Vitest run it measures cannot drift apart. If that
// loader ever breaks (e.g. the config file moves or changes shape), this
// script fails loudly rather than silently falling back to a hardcoded
// copy of the globs; see the matching comment in vitest.config.ts.
//
// Usage: node scripts/enforce-frontend-file-floor.mjs [--summary <path>]
// Exit code: 0 on pass, 1 on any file below floor, or on any structural
// error (missing artifact, unreadable config, zero governed files, etc.).
// A silent pass on a structural error would be worse than a loud failure.

import { createRequire } from 'node:module'
import { readFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const FLOOR = { lines: 70, branches: 60, functions: 70, statements: 70 }

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(scriptDir, '..')
const frontendDir = path.join(repoRoot, 'frontend')
const configPath = path.join(frontendDir, 'vitest.config.ts')

function usageError(message) {
  console.error(`FAIL: ${message}`)
  console.error('usage: enforce-frontend-file-floor.mjs [--summary <path>]')
  process.exit(1)
}

function parseArgs(argv) {
  let summary
  for (let index = 0; index < argv.length; index++) {
    if (argv[index] === '--summary') {
      summary = argv[index + 1]
      index++
      if (summary === undefined) usageError('--summary requires a value')
    } else {
      usageError(`unknown CLI argument ${argv[index]}`)
    }
  }
  return {
    summaryPath: summary === undefined
      ? path.join(frontendDir, 'coverage', 'coverage-summary.json')
      : path.resolve(process.cwd(), summary),
  }
}

// `vite` and `picomatch` are not declared in frontend/package.json directly,
// but both are guaranteed present after `npm ci`: `vite` is a direct
// dependency of the installed `vitest` package (Vitest cannot run without
// it), and `picomatch` is a direct dependency of `vite` itself. Resolving
// them relative to frontend/package.json (not this script's own location)
// is what makes that guarantee hold regardless of where this script lives.
function frontendRequire() {
  return createRequire(path.join(frontendDir, 'package.json'))
}

async function loadCoverageGlobs() {
  const require = frontendRequire()
  let vite
  try {
    vite = await import(pathToFileURL(require.resolve('vite')).href)
  } catch (error) {
    usageError(`unable to load vite to read vitest.config.ts coverage globs: ${error.message}`)
  }
  let loaded
  try {
    loaded = await vite.loadConfigFromFile({ command: 'serve', mode: 'test' }, configPath, frontendDir)
  } catch (error) {
    usageError(`unable to load ${configPath}: ${error.message}`)
  }
  const coverage = loaded?.config?.test?.coverage
  const include = coverage?.include
  const exclude = coverage?.exclude
  if (!Array.isArray(include) || include.length === 0 || !Array.isArray(exclude)) {
    usageError(`${configPath} test.coverage.include/exclude are missing or malformed`)
  }
  return { include, exclude }
}

function loadPicomatch() {
  try {
    return frontendRequire()('picomatch')
  } catch (error) {
    usageError(`unable to load picomatch (installed transitively via vite): ${error.message}`)
  }
}

function readSummary(summaryPath) {
  let raw
  try {
    raw = readFileSync(summaryPath, 'utf8')
  } catch {
    usageError(`coverage summary not found at ${summaryPath} -- run "vitest run --coverage" first`)
  }
  try {
    return JSON.parse(raw)
  } catch (error) {
    usageError(`coverage summary at ${summaryPath} is not valid JSON: ${error.message}`)
  }
}

function metricPct(metrics, metric, file) {
  const record = metrics?.[metric]
  const pct = record?.pct
  if (pct === 'Unknown') {
    console.error(`FAIL: ${file} ${metric} coverage is Unknown in coverage summary`)
    return undefined
  }
  if (typeof pct !== 'number' || !Number.isFinite(pct) || pct < 0 || pct > 100) {
    console.error(`FAIL: ${file} ${metric} coverage is missing or malformed in coverage summary`)
    return undefined
  }
  // Vitest's V8 json-summary reporter emits { total: 0, pct: 0 } for a
  // metric with nothing to cover -- e.g. a barrel file that is only
  // `export { x } from './y'` has zero executable statements/branches/
  // functions/lines. That is not "0% covered", it is "nothing to cover";
  // treat it as trivially satisfying the floor rather than an unfixable
  // permanent failure (there is no test you can write to raise a 0/0
  // metric above a percentage floor).
  if (record.total === 0) return null
  return pct
}

async function main() {
  const { summaryPath } = parseArgs(process.argv.slice(2))
  const { include, exclude } = await loadCoverageGlobs()
  const picomatch = loadPicomatch()
  const isIncluded = picomatch(include)
  const isExcluded = picomatch(exclude)

  const data = readSummary(summaryPath)
  const entries = Object.entries(data).filter(([key]) => key !== 'total')

  let failures = 0
  let governed = 0
  for (const [rawPath, metrics] of entries) {
    const relToFrontend = path.isAbsolute(rawPath) ? path.relative(frontendDir, rawPath) : rawPath
    const posixPath = relToFrontend.split(path.sep).join('/')
    if (!isIncluded(posixPath) || isExcluded(posixPath)) continue
    governed++
    for (const [metric, floor] of Object.entries(FLOOR)) {
      const pct = metricPct(metrics, metric, posixPath)
      if (pct === null) continue // zero-total metric: nothing to cover, vacuously passes
      if (pct === undefined) {
        failures++
        continue
      }
      if (pct < floor) {
        console.error(`FAIL: ${posixPath} ${metric} coverage ${pct.toFixed(2)}% < floor ${floor}%`)
        failures++
      }
    }
  }

  if (governed === 0) {
    usageError('no governed files found in coverage summary -- include/exclude globs may have drifted from vitest.config.ts, or the summary predates coverage.include changes')
  }

  if (failures > 0) {
    console.error(`FAIL: ${failures} per-file coverage floor violation(s) across ${governed} governed file(s) (floor: lines=${FLOOR.lines} branches=${FLOOR.branches} functions=${FLOOR.functions} statements=${FLOOR.statements})`)
    process.exitCode = 1
    return
  }

  console.log(`PASS frontend-file-floor governed=${governed} floor=lines${FLOOR.lines}/branches${FLOOR.branches}/functions${FLOOR.functions}/statements${FLOOR.statements}`)
}

main()
