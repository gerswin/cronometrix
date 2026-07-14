#!/usr/bin/env node

import { execFileSync, spawnSync } from 'node:child_process'
import { readFileSync, realpathSync } from 'node:fs'
import path from 'node:path'

const EXPECTED_PLAN = '12-02'
const EXPECTED_THRESHOLDS = {
  backend: { lines: 70, branches: 60 },
  frontend: { statements: 70, branches: 60, functions: 70, lines: 70 },
}

function fail(messages) {
  for (const message of messages) console.error(`FAIL: ${message}`)
  process.exitCode = 1
}

function parseArgs(argv) {
  const flags = new Map()
  const allowed = new Set(['--manifest', '--frontend-summary', '--backend-lcov'])
  const errors = []
  for (let index = 0; index < argv.length; index += 2) {
    const flag = argv[index]
    const value = argv[index + 1]
    if (!allowed.has(flag)) {
      errors.push(`unknown CLI argument ${flag ?? '<missing>'}`)
      continue
    }
    if (value === undefined || value.startsWith('--')) {
      errors.push(`${flag} requires a path value`)
      index -= 1
      continue
    }
    if (flags.has(flag)) errors.push(`${flag} was provided more than once`)
    flags.set(flag, value)
  }
  if (!flags.has('--manifest')) errors.push('--manifest is required')
  return { flags, errors }
}

function readJson(filePath, label, errors) {
  try {
    return JSON.parse(readFileSync(filePath, 'utf8'))
  } catch {
    errors.push(`${label} JSON is missing or malformed: ${filePath}`)
    return undefined
  }
}

function sameKeysAndValues(actual, expected) {
  if (actual === null || typeof actual !== 'object' || Array.isArray(actual)) return false
  const actualKeys = Object.keys(actual).sort()
  const expectedKeys = Object.keys(expected).sort()
  return actualKeys.length === expectedKeys.length
    && actualKeys.every((key, index) => key === expectedKeys[index] && actual[key] === expected[key])
}

function validateManifest(raw, errors) {
  if (raw === null || typeof raw !== 'object' || Array.isArray(raw)) {
    errors.push('manifest must be a JSON object')
    return undefined
  }
  if (raw.plan !== EXPECTED_PLAN) errors.push(`manifest plan must be ${EXPECTED_PLAN}`)
  if (typeof raw.base_sha !== 'string' || raw.base_sha.length === 0) {
    errors.push('manifest base_sha is missing')
  }
  if (!sameKeysAndValues(raw.thresholds?.backend, EXPECTED_THRESHOLDS.backend)) {
    errors.push('manifest backend thresholds must exactly match lines=70 branches=60')
  }
  if (!sameKeysAndValues(raw.thresholds?.frontend, EXPECTED_THRESHOLDS.frontend)) {
    errors.push('manifest frontend thresholds must exactly match statements=70 branches=60 functions=70 lines=70')
  }

  const result = { ...raw }
  for (const side of ['backend', 'frontend']) {
    if (result[side] === undefined) {
      result[side] = []
    } else if (!Array.isArray(result[side])) {
      errors.push(`manifest ${side} must be an array`)
      result[side] = []
      continue
    }
    const seen = new Set()
    for (const entry of result[side]) {
      const prefix = `${side}/src/`
      if (typeof entry !== 'string'
        || entry.includes('\\')
        || path.posix.isAbsolute(entry)
        || path.posix.normalize(entry) !== entry
        || !entry.startsWith(prefix)) {
        errors.push(`manifest ${side} entry is not a normalized repo-relative source path: ${String(entry)}`)
      } else if (seen.has(entry)) {
        errors.push(`manifest ${side} contains duplicate entry ${entry}`)
      }
      seen.add(entry)
    }
  }
  return result
}

function repoRelative(repoRoot, side, rawPath, label, errors) {
  if (typeof rawPath !== 'string' || rawPath.length === 0) {
    errors.push(`${label} contains an empty source path`)
    return undefined
  }
  const resolved = path.isAbsolute(rawPath)
    ? path.normalize(rawPath)
    : rawPath.startsWith(`${side}/`)
      ? path.resolve(repoRoot, rawPath)
      : path.resolve(repoRoot, side, rawPath)
  let absolute = resolved
  try {
    absolute = realpathSync(resolved)
  } catch {
    // Coverage source paths should exist; retain the resolved path so later
    // artifact/manifest validation can report the actionable defect.
  }
  const relative = path.relative(repoRoot, absolute)
  if (relative === '' || relative === '..' || relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative)) {
    errors.push(`${label} source path is outside the repository: ${rawPath}`)
    return undefined
  }
  return relative.split(path.sep).join('/')
}

function parseCounter(record, name, file, errors) {
  const matches = record.filter((line) => line.startsWith(`${name}:`))
  if (matches.length !== 1 || !/^\d+$/.test(matches[0]?.slice(name.length + 1) ?? '')) {
    errors.push(`backend malformed or missing ${name} counter for ${file}`)
    return undefined
  }
  const value = Number(matches[0].slice(name.length + 1))
  if (!Number.isSafeInteger(value)) {
    errors.push(`backend malformed ${name} counter for ${file}`)
    return undefined
  }
  return value
}

function parseBackendLcov(filePath, repoRoot, errors) {
  let content
  try {
    content = readFileSync(filePath, 'utf8')
  } catch {
    errors.push(`backend LCOV artifact is missing or unreadable: ${filePath}`)
    return new Map()
  }
  const files = new Map()
  for (const block of content.split(/^end_of_record\r?$/m)) {
    const record = block.split(/\r?\n/).filter(Boolean)
    if (record.length === 0) continue
    const sourceLines = record.filter((line) => line.startsWith('SF:'))
    if (sourceLines.length !== 1) {
      errors.push('backend malformed LCOV record has missing or duplicate SF')
      continue
    }
    const file = repoRelative(repoRoot, 'backend', sourceLines[0].slice(3), 'backend LCOV', errors)
    if (file === undefined) continue
    const LF = parseCounter(record, 'LF', file, errors)
    const LH = parseCounter(record, 'LH', file, errors)
    const BRF = parseCounter(record, 'BRF', file, errors)
    const BRH = parseCounter(record, 'BRH', file, errors)
    if ([LF, LH, BRF, BRH].some((value) => value === undefined)) continue
    if (LF === 0 || LH > LF || BRH > BRF) {
      errors.push(`backend malformed coverage counters for ${file}`)
      continue
    }
    if (files.has(file)) {
      errors.push(`backend LCOV contains duplicate source record ${file}`)
      continue
    }
    files.set(file, {
      lines: (LH / LF) * 100,
      branches: BRF === 0 ? 100 : (BRH / BRF) * 100,
    })
  }
  return files
}

function frontendMetric(metrics, metric, file, errors) {
  if (!(metric in metrics)) {
    errors.push(`frontend ${metric} metric is missing for ${file}`)
    return undefined
  }
  const value = metrics[metric]?.pct
  if (value === 'Unknown') {
    errors.push(`frontend ${metric} metric is Unknown for ${file}`)
    return undefined
  }
  if (value === null || (typeof value === 'number' && !Number.isFinite(value))) {
    errors.push(`frontend ${metric} metric is non-finite for ${file}`)
    return undefined
  }
  if (typeof value !== 'number') {
    errors.push(`frontend ${metric} metric is malformed for ${file}`)
    return undefined
  }
  if (value < 0 || value > 100) {
    errors.push(`frontend ${metric} metric is out of range for ${file}: ${value}`)
    return undefined
  }
  return value
}

function parseFrontendSummary(filePath, repoRoot, errors) {
  const raw = readJson(filePath, 'frontend coverage summary', errors)
  const files = new Map()
  const seen = new Set()
  if (raw === undefined) return files
  if (raw === null || typeof raw !== 'object' || Array.isArray(raw)) {
    errors.push('frontend coverage summary JSON must be an object')
    return files
  }
  for (const [rawPath, metrics] of Object.entries(raw)) {
    if (rawPath === 'total') continue
    const file = repoRelative(repoRoot, 'frontend', rawPath, 'frontend coverage summary', errors)
    if (file === undefined) continue
    if (seen.has(file)) {
      errors.push(`frontend coverage summary contains duplicate normalized source path ${file}`)
      continue
    }
    seen.add(file)
    if (metrics === null || typeof metrics !== 'object' || Array.isArray(metrics)) {
      errors.push(`frontend metrics are malformed for ${file}`)
      continue
    }
    const parsed = {}
    for (const metric of Object.keys(EXPECTED_THRESHOLDS.frontend)) {
      parsed[metric] = frontendMetric(metrics, metric, file, errors)
    }
    if (Object.values(parsed).every((value) => value !== undefined)) files.set(file, parsed)
  }
  return files
}

function changedFiles(repoRoot, baseSha, errors) {
  if (typeof baseSha !== 'string' || baseSha.length === 0) return new Set()
  const valid = spawnSync('git', ['cat-file', '-e', `${baseSha}^{commit}`], {
    cwd: repoRoot,
    encoding: 'utf8',
  })
  if (valid.status !== 0) {
    errors.push(`manifest base_sha is invalid: ${baseSha}`)
    return new Set()
  }
  const diff = spawnSync('git', ['diff', '--name-only', `${baseSha}...HEAD`], {
    cwd: repoRoot,
    encoding: 'utf8',
  })
  if (diff.status !== 0) {
    errors.push(`git diff failed for base_sha ${baseSha}`)
    return new Set()
  }
  return new Set(diff.stdout.split(/\r?\n/).filter(Boolean).map((file) => file.split(path.sep).join('/')))
}

function enforceSide(side, manifestFiles, artifactFiles, changed, thresholds, errors) {
  const owned = new Set(manifestFiles)
  for (const file of artifactFiles.keys()) {
    if (changed.has(file) && !owned.has(file)) {
      errors.push(`${side} changed covered file omitted from manifest: ${file}`)
    }
  }
  for (const file of manifestFiles) {
    if (!changed.has(file)) {
      errors.push(`${side} manifest entry was not changed since base_sha: ${file}`)
    }
    const metrics = artifactFiles.get(file)
    if (metrics === undefined) {
      errors.push(`${side} manifest entry missing from coverage artifact: ${file}`)
      continue
    }
    for (const [metric, floor] of Object.entries(thresholds)) {
      const actual = metrics[metric]
      if (actual < floor) {
        errors.push(`${side} ${metric} coverage ${actual.toFixed(2)} below floor ${floor} for ${file}`)
      }
    }
  }
}

function main() {
  const { flags, errors } = parseArgs(process.argv.slice(2))
  if (errors.length > 0) return fail(errors)

  let repoRoot
  try {
    repoRoot = execFileSync('git', ['rev-parse', '--show-toplevel'], {
      cwd: process.cwd(),
      encoding: 'utf8',
    }).trim()
  } catch {
    return fail(['unable to resolve repository root with git'])
  }

  const manifestPath = path.resolve(process.cwd(), flags.get('--manifest'))
  const rawManifest = readJson(manifestPath, 'manifest', errors)
  const manifest = rawManifest === undefined ? undefined : validateManifest(rawManifest, errors)
  if (manifest === undefined) return fail(errors)
  if (errors.length > 0) return fail(errors)

  const frontendPath = flags.get('--frontend-summary')
  const backendPath = flags.get('--backend-lcov')
  if (manifest.frontend.length > 0 && frontendPath === undefined) {
    errors.push('frontend coverage artifact is required for a nonempty manifest')
  } else if (manifest.frontend.length === 0 && frontendPath !== undefined) {
    errors.push('frontend artifact was provided while the frontend manifest is empty')
  }
  if (manifest.backend.length > 0 && backendPath === undefined) {
    errors.push('backend coverage artifact is required for a nonempty manifest')
  } else if (manifest.backend.length === 0 && backendPath !== undefined) {
    errors.push('backend artifact was provided while the backend manifest is empty')
  }

  const changed = changedFiles(repoRoot, manifest.base_sha, errors)
  const frontendFiles = frontendPath === undefined
    ? new Map()
    : parseFrontendSummary(path.resolve(process.cwd(), frontendPath), repoRoot, errors)
  const backendFiles = backendPath === undefined
    ? new Map()
    : parseBackendLcov(path.resolve(process.cwd(), backendPath), repoRoot, errors)

  if (frontendPath !== undefined) {
    enforceSide('frontend', manifest.frontend, frontendFiles, changed, manifest.thresholds.frontend, errors)
  }
  if (backendPath !== undefined) {
    enforceSide('backend', manifest.backend, backendFiles, changed, manifest.thresholds.backend, errors)
  }

  if (errors.length > 0) return fail(errors)
  console.log(`PASS owned-coverage plan=${manifest.plan} backend=${manifest.backend.length} frontend=${manifest.frontend.length}`)
}

main()
