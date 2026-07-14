import assert from 'node:assert/strict'
import { execFileSync, spawnSync } from 'node:child_process'
import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

const checker = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..',
  'enforce-owned-coverage.mjs',
)

function git(cwd, ...args) {
  return execFileSync('git', args, { cwd, encoding: 'utf8' }).trim()
}

function makeRepo(changedFiles = [], baseFiles = []) {
  const root = mkdtempSync(path.join(tmpdir(), 'owned-coverage-'))
  git(root, 'init', '-q')
  git(root, 'config', 'user.email', 'coverage@example.invalid')
  git(root, 'config', 'user.name', 'Coverage Test')
  writeFileSync(path.join(root, '.gitignore'), 'coverage-summary.json\nlcov.info\n')
  for (const relativePath of baseFiles) {
    const absolutePath = path.join(root, relativePath)
    mkdirSync(path.dirname(absolutePath), { recursive: true })
    writeFileSync(absolutePath, '// pre-existing covered production file\n')
  }
  git(root, 'add', '.gitignore', ...baseFiles)
  git(root, 'commit', '-qm', 'base')
  const baseSha = git(root, 'rev-parse', 'HEAD')

  for (const relativePath of changedFiles) {
    const absolutePath = path.join(root, relativePath)
    mkdirSync(path.dirname(absolutePath), { recursive: true })
    writeFileSync(absolutePath, '// covered production file\n')
  }
  if (changedFiles.length > 0) {
    git(root, 'add', ...changedFiles)
    git(root, 'commit', '-qm', 'change covered files')
  }

  return { root, baseSha }
}

function frontendMetrics(overrides = {}) {
  const metric = (pct) => ({ total: 10_000, covered: pct * 100, skipped: 0, pct })
  return {
    statements: metric(70),
    branches: metric(60),
    functions: metric(70),
    lines: metric(70),
    ...overrides,
  }
}

function artifactSourcePath(root, relativePath, side, style) {
  if (style === 'repo-relative') return relativePath
  if (style === 'side-relative') return relativePath.slice(`${side}/`.length)
  return path.join(root, relativePath)
}

function writeFrontendSummary(root, files, style = 'absolute') {
  const summary = {
    total: frontendMetrics({
      statements: { total: 0, covered: 0, skipped: 0, pct: 100 },
      branches: { total: 0, covered: 0, skipped: 0, pct: 100 },
      functions: { total: 0, covered: 0, skipped: 0, pct: 100 },
      lines: { total: 0, covered: 0, skipped: 0, pct: 100 },
    }),
  }
  for (const [relativePath, metrics] of Object.entries(files)) {
    summary[artifactSourcePath(root, relativePath, 'frontend', style)] = metrics
  }
  const output = path.join(root, 'coverage-summary.json')
  writeFileSync(output, `${JSON.stringify(summary)}\n`)
  return output
}

function writeBackendLcov(root, files, style = 'absolute') {
  const records = Object.entries(files).map(
    ([relativePath, counters]) => [
      `SF:${artifactSourcePath(root, relativePath, 'backend', style)}`,
      `LF:${counters.LF}`,
      `LH:${counters.LH}`,
      `BRF:${counters.BRF}`,
      `BRH:${counters.BRH}`,
      'end_of_record',
    ].join('\n'),
  )
  const output = path.join(root, 'lcov.info')
  writeFileSync(output, `${records.join('\n')}\n`)
  return output
}

function manifest(baseSha, overrides = {}) {
  return {
    plan: '12-02',
    base_sha: baseSha,
    thresholds: {
      backend: { lines: 70, branches: 60 },
      frontend: { statements: 70, branches: 60, functions: 70, lines: 70 },
    },
    backend: [],
    frontend: [],
    ...overrides,
  }
}

function runChecker({ root, manifest: data, frontendSummary, backendLcov }) {
  const manifestPath = path.join(root, `${data.plan}-COVERAGE-OWNERSHIP.json`)
  writeFileSync(manifestPath, `${JSON.stringify(data)}\n`)
  const args = [checker, '--manifest', manifestPath]
  if (frontendSummary !== undefined) args.push('--frontend-summary', frontendSummary)
  if (backendLcov !== undefined) args.push('--backend-lcov', backendLcov)
  return spawnSync(process.execPath, args, { cwd: root, encoding: 'utf8' })
}

function expectPass(result, backend, frontend, plan = '12-02') {
  assert.equal(result.status, 0, result.stderr || result.stdout)
  assert.equal(
    result.stdout.trim(),
    `PASS owned-coverage plan=${plan} backend=${backend} frontend=${frontend}`,
  )
  assert.equal(result.stderr, '')
}

function expectFail(result, pattern) {
  assert.notEqual(result.status, 0, 'checker unexpectedly passed')
  const output = `${result.stdout}${result.stderr}`
  assert.match(output, pattern)
  for (const line of output.trim().split('\n')) {
    if (line.length > 0) assert.match(line, /^FAIL:/)
  }
}

test('passes exact boundaries with backend and frontend artifacts', () => {
  const backendFile = 'backend/src/auth/service.rs'
  const frontendFile = 'frontend/src/lib/api.ts'
  const { root, baseSha } = makeRepo([backendFile, frontendFile])
  const backendLcov = writeBackendLcov(root, {
    [backendFile]: { LF: 10, LH: 7, BRF: 10, BRH: 6 },
  })
  const frontendSummary = writeFrontendSummary(root, {
    [frontendFile]: frontendMetrics(),
  })

  expectPass(runChecker({
    root,
    manifest: manifest(baseSha, {
      backend: [backendFile],
      frontend: [frontendFile],
    }),
    frontendSummary,
    backendLcov,
  }), 1, 1)
})

test('accepts the same manifest schema for later release plans', () => {
  const frontendFile = 'frontend/src/lib/api.ts'
  const { root, baseSha } = makeRepo([frontendFile])
  const frontendSummary = writeFrontendSummary(root, {
    [frontendFile]: frontendMetrics(),
  })

  expectPass(runChecker({
    root,
    manifest: manifest(baseSha, { plan: '12-03', frontend: [frontendFile] }),
    frontendSummary,
  }), 0, 1, '12-03')
})

test('passes a frontend-only manifest and artifact', () => {
  const frontendFile = 'frontend/src/lib/api.ts'
  const { root, baseSha } = makeRepo([frontendFile])
  const frontendSummary = writeFrontendSummary(root, {
    [frontendFile]: frontendMetrics(),
  })

  expectPass(runChecker({
    root,
    manifest: manifest(baseSha, { frontend: [frontendFile] }),
    frontendSummary,
  }), 0, 1)
})

test('passes a backend-only manifest and artifact', () => {
  const backendFile = 'backend/src/auth/service.rs'
  const { root, baseSha } = makeRepo([backendFile])
  const backendLcov = writeBackendLcov(root, {
    [backendFile]: { LF: 10, LH: 7, BRF: 10, BRH: 6 },
  })

  expectPass(runChecker({
    root,
    manifest: manifest(baseSha, { backend: [backendFile] }),
    backendLcov,
  }), 1, 0)
})

test('normalizes repo-relative paths from both coverage artifacts', () => {
  const backendFile = 'backend/src/auth/service.rs'
  const frontendFile = 'frontend/src/lib/api.ts'
  const { root, baseSha } = makeRepo([backendFile, frontendFile])
  const backendLcov = writeBackendLcov(root, {
    [backendFile]: { LF: 10, LH: 7, BRF: 10, BRH: 6 },
  }, 'repo-relative')
  const frontendSummary = writeFrontendSummary(root, {
    [frontendFile]: frontendMetrics(),
  }, 'repo-relative')

  expectPass(runChecker({
    root,
    manifest: manifest(baseSha, { backend: [backendFile], frontend: [frontendFile] }),
    frontendSummary,
    backendLcov,
  }), 1, 1)
})

test('normalizes side-relative src paths from both coverage artifacts', () => {
  const backendFile = 'backend/src/auth/service.rs'
  const frontendFile = 'frontend/src/lib/api.ts'
  const { root, baseSha } = makeRepo([backendFile, frontendFile])
  const backendLcov = writeBackendLcov(root, {
    [backendFile]: { LF: 10, LH: 7, BRF: 10, BRH: 6 },
  }, 'side-relative')
  const frontendSummary = writeFrontendSummary(root, {
    [frontendFile]: frontendMetrics(),
  }, 'side-relative')

  expectPass(runChecker({
    root,
    manifest: manifest(baseSha, { backend: [backendFile], frontend: [frontendFile] }),
    frontendSummary,
    backendLcov,
  }), 1, 1)
})

test('fails when a changed covered file is omitted from the manifest', () => {
  const frontendFile = 'frontend/src/lib/api.ts'
  const { root, baseSha } = makeRepo([frontendFile])
  const frontendSummary = writeFrontendSummary(root, {
    [frontendFile]: frontendMetrics(),
  })

  expectFail(runChecker({
    root,
    manifest: manifest(baseSha),
    frontendSummary,
  }), /FAIL:.*frontend.*omitted.*frontend\/src\/lib\/api\.ts/i)
})

test('fails when a manifest entry is missing from coverage', () => {
  const ownedFile = 'frontend/src/lib/api.ts'
  const coveredFile = 'frontend/src/lib/validations.ts'
  const { root, baseSha } = makeRepo([ownedFile])
  const frontendSummary = writeFrontendSummary(root, {
    [coveredFile]: frontendMetrics(),
  })

  expectFail(runChecker({
    root,
    manifest: manifest(baseSha, { frontend: [ownedFile] }),
    frontendSummary,
  }), /FAIL:.*frontend.*manifest.*missing from coverage.*frontend\/src\/lib\/api\.ts/i)
})

test('fails when a covered manifest entry was not changed since base_sha', () => {
  const extraFile = 'frontend/src/lib/api.ts'
  const { root, baseSha } = makeRepo([], [extraFile])
  const frontendSummary = writeFrontendSummary(root, {
    [extraFile]: frontendMetrics(),
  })

  expectFail(runChecker({
    root,
    manifest: manifest(baseSha, { frontend: [extraFile] }),
    frontendSummary,
  }), /FAIL:.*frontend.*manifest.*not changed.*frontend\/src\/lib\/api\.ts/i)
})

test('fails when a manifest side is null instead of absent or an array', () => {
  const { root, baseSha } = makeRepo()
  expectFail(runChecker({
    root,
    manifest: manifest(baseSha, { backend: null }),
  }), /FAIL:.*manifest backend must be an array/i)
})

for (const [label, thresholds] of [
  ['absent', undefined],
  ['null', null],
  ['malformed', { backend: 'invalid', frontend: 'invalid' }],
]) {
  test(`fails cleanly when thresholds are ${label} for a present side`, () => {
    const frontendFile = 'frontend/src/lib/api.ts'
    const { root, baseSha } = makeRepo([frontendFile])
    const frontendSummary = writeFrontendSummary(root, {
      [frontendFile]: frontendMetrics(),
    })
    expectFail(runChecker({
      root,
      manifest: manifest(baseSha, { thresholds, frontend: [frontendFile] }),
      frontendSummary,
    }), /FAIL:.*frontend thresholds/i)
  })
}

test('fails closed on malformed frontend JSON', () => {
  const { root, baseSha } = makeRepo()
  const frontendSummary = path.join(root, 'coverage-summary.json')
  writeFileSync(frontendSummary, '{not json')

  expectFail(runChecker({
    root,
    manifest: manifest(baseSha, { frontend: ['frontend/src/lib/api.ts'] }),
    frontendSummary,
  }), /FAIL:.*frontend.*JSON/i)
})

test('fails closed on malformed backend LCOV counters', () => {
  const backendFile = 'backend/src/auth/service.rs'
  const { root, baseSha } = makeRepo([backendFile])
  const backendLcov = path.join(root, 'lcov.info')
  writeFileSync(backendLcov, `SF:${path.join(root, backendFile)}\nLF:ten\nLH:7\nBRF:10\nBRH:6\nend_of_record\n`)

  expectFail(runChecker({
    root,
    manifest: manifest(baseSha, { backend: [backendFile] }),
    backendLcov,
  }), /FAIL:.*backend.*malformed.*LF/i)
})

test('fails closed when a required backend LCOV counter is absent', () => {
  const backendFile = 'backend/src/auth/service.rs'
  const { root, baseSha } = makeRepo([backendFile])
  const backendLcov = path.join(root, 'lcov.info')
  writeFileSync(backendLcov, `SF:${path.join(root, backendFile)}\nLF:10\nLH:7\nBRF:10\nend_of_record\n`)

  expectFail(runChecker({
    root,
    manifest: manifest(baseSha, { backend: [backendFile] }),
    backendLcov,
  }), /FAIL:.*backend.*missing BRH.*backend\/src\/auth\/service\.rs/i)
})

test('does not split an LCOV record when end_of_record occurs inside FN and FNDA', () => {
  const backendFile = 'backend/src/auth/service.rs'
  const { root, baseSha } = makeRepo([backendFile])
  const backendLcov = path.join(root, 'lcov.info')
  writeFileSync(backendLcov, [
    `SF:${path.join(root, backendFile)}`,
    'FN:1,helper_end_of_record_name',
    'FNDA:1,helper_end_of_record_name',
    'LF:10',
    'LH:7',
    'BRF:10',
    'BRH:6',
    'end_of_record',
    '',
  ].join('\n'))

  expectPass(runChecker({
    root,
    manifest: manifest(baseSha, { backend: [backendFile] }),
    backendLcov,
  }), 1, 0)
})

for (const [metric, threshold] of Object.entries({
  statements: 70,
  branches: 60,
  functions: 70,
  lines: 70,
})) {
  test(`fails when frontend ${metric} is below its floor`, () => {
    const frontendFile = 'frontend/src/lib/api.ts'
    const { root, baseSha } = makeRepo([frontendFile])
    const frontendSummary = writeFrontendSummary(root, {
      [frontendFile]: frontendMetrics({
        [metric]: {
          total: 10_000,
          covered: (threshold - 0.01) * 100,
          skipped: 0,
          pct: threshold - 0.01,
        },
      }),
    })

    expectFail(runChecker({
      root,
      manifest: manifest(baseSha, { frontend: [frontendFile] }),
      frontendSummary,
    }), new RegExp(`FAIL:.*${metric}.*${threshold - 0.01}.*floor ${threshold}`, 'i'))
  })
}

test('rejects missing, malformed, non-finite, and Unknown frontend metrics', () => {
  const cases = [
    ['missing', undefined],
    ['malformed', { pct: '70' }],
    ['non-finite', { pct: Number.POSITIVE_INFINITY }],
    ['Unknown', { pct: 'Unknown' }],
  ]
  for (const [label, badMetric] of cases) {
    const frontendFile = 'frontend/src/lib/api.ts'
    const { root, baseSha } = makeRepo([frontendFile])
    const metrics = frontendMetrics()
    if (badMetric === undefined) delete metrics.lines
    else metrics.lines = badMetric
    const frontendSummary = writeFrontendSummary(root, { [frontendFile]: metrics })

    expectFail(runChecker({
      root,
      manifest: manifest(baseSha, { frontend: [frontendFile] }),
      frontendSummary,
    }), new RegExp(`FAIL:.*frontend.*lines.*${label}`, 'i'))
  }
})

test('rejects frontend pct metrics outside the inclusive zero-to-100 range', () => {
  for (const value of [-0.01, 100.01]) {
    const frontendFile = 'frontend/src/lib/api.ts'
    const { root, baseSha } = makeRepo([frontendFile])
    const frontendSummary = writeFrontendSummary(root, {
      [frontendFile]: frontendMetrics({
        lines: { total: 10_000, covered: value * 100, skipped: 0, pct: value },
      }),
    })

    expectFail(runChecker({
      root,
      manifest: manifest(baseSha, { frontend: [frontendFile] }),
      frontendSummary,
    }), new RegExp(`FAIL:.*frontend lines.*out of range.*${String(value).replace('.', '\\.')}`, 'i'))
  }
})

test('rejects duplicate frontend keys after path normalization', () => {
  const frontendFile = 'frontend/src/lib/api.ts'
  const { root, baseSha } = makeRepo([frontendFile])
  const frontendSummary = path.join(root, 'coverage-summary.json')
  writeFileSync(frontendSummary, `${JSON.stringify({
    total: frontendMetrics(),
    [path.join(root, frontendFile)]: frontendMetrics(),
    [frontendFile]: frontendMetrics(),
  })}\n`)

  expectFail(runChecker({
    root,
    manifest: manifest(baseSha, { frontend: [frontendFile] }),
    frontendSummary,
  }), /FAIL:.*frontend.*duplicate.*frontend\/src\/lib\/api\.ts/i)
})

test('rejects normalized duplicate keys even when the first metrics are malformed', () => {
  const frontendFile = 'frontend/src/lib/api.ts'
  const { root, baseSha } = makeRepo([frontendFile])
  const frontendSummary = path.join(root, 'coverage-summary.json')
  writeFileSync(frontendSummary, `${JSON.stringify({
    total: frontendMetrics(),
    [path.join(root, frontendFile)]: { lines: { pct: 'invalid' } },
    [frontendFile]: frontendMetrics(),
  })}\n`)

  expectFail(runChecker({
    root,
    manifest: manifest(baseSha, { frontend: [frontendFile] }),
    frontendSummary,
  }), /FAIL:.*frontend.*duplicate.*frontend\/src\/lib\/api\.ts/i)
})

test('fails when backend line coverage is below its floor', () => {
  const backendFile = 'backend/src/auth/service.rs'
  const { root, baseSha } = makeRepo([backendFile])
  const backendLcov = writeBackendLcov(root, {
    [backendFile]: { LF: 10, LH: 6, BRF: 10, BRH: 6 },
  })

  expectFail(runChecker({
    root,
    manifest: manifest(baseSha, { backend: [backendFile] }),
    backendLcov,
  }), /FAIL:.*lines.*60\.00.*floor 70/i)
})

test('fails when backend branch coverage is below its floor', () => {
  const backendFile = 'backend/src/auth/service.rs'
  const { root, baseSha } = makeRepo([backendFile])
  const backendLcov = writeBackendLcov(root, {
    [backendFile]: { LF: 10, LH: 7, BRF: 10, BRH: 5 },
  })

  expectFail(runChecker({
    root,
    manifest: manifest(baseSha, { backend: [backendFile] }),
    backendLcov,
  }), /FAIL:.*branches.*50\.00.*floor 60/i)
})

test('requires an artifact when the corresponding manifest side is nonempty', () => {
  const { root, baseSha } = makeRepo()
  expectFail(runChecker({
    root,
    manifest: manifest(baseSha, { frontend: ['frontend/src/lib/api.ts'] }),
  }), /FAIL:.*frontend.*artifact.*required/i)
})

test('allows an artifact for an empty side when it contains no relevant changed file', () => {
  const { root, baseSha } = makeRepo(['README.md'])
  const frontendSummary = writeFrontendSummary(root, {})
  expectPass(runChecker({
    root,
    manifest: manifest(baseSha),
    frontendSummary,
  }), 0, 0)
})

test('treats zero measured backend branches as 100 percent', () => {
  const backendFile = 'backend/src/http_trace.rs'
  const { root, baseSha } = makeRepo([backendFile])
  const backendLcov = writeBackendLcov(root, {
    [backendFile]: { LF: 10, LH: 7, BRF: 0, BRH: 0 },
  })

  expectPass(runChecker({
    root,
    manifest: manifest(baseSha, { backend: [backendFile] }),
    backendLcov,
  }), 1, 0)
})

test('fails when base_sha is missing', () => {
  const { root } = makeRepo()
  expectFail(runChecker({
    root,
    manifest: manifest(undefined),
  }), /FAIL:.*base_sha.*missing/i)
})

test('fails when base_sha does not resolve to a commit', () => {
  const { root } = makeRepo()
  expectFail(runChecker({
    root,
    manifest: manifest('0000000000000000000000000000000000000000'),
  }), /FAIL:.*base_sha.*invalid/i)
})

test('fails when base_sha is HEAD even with an empty owned manifest', () => {
  const { root } = makeRepo()
  const head = git(root, 'rev-parse', 'HEAD')
  expectFail(runChecker({
    root,
    manifest: manifest(head),
  }), /FAIL:.*base_sha.*must precede HEAD/i)
})

test('fails explicitly when base_sha is not an ancestor of HEAD', () => {
  const { root, baseSha } = makeRepo()
  git(root, 'checkout', '--orphan', 'unrelated')
  git(root, 'rm', '-rf', '.')
  writeFileSync(path.join(root, '.gitignore'), 'coverage-summary.json\nlcov.info\n')
  git(root, 'add', '.gitignore')
  git(root, 'commit', '-qm', 'unrelated head')

  expectFail(runChecker({
    root,
    manifest: manifest(baseSha),
  }), /FAIL:.*base_sha.*not an ancestor of HEAD/i)
})

test('fails vacuous empty ownership when an artifact has a relevant changed covered file', () => {
  const frontendFile = 'frontend/src/lib/api.ts'
  const { root, baseSha } = makeRepo([frontendFile])
  const frontendSummary = writeFrontendSummary(root, {
    [frontendFile]: frontendMetrics(),
  })

  expectFail(runChecker({
    root,
    manifest: manifest(baseSha),
    frontendSummary,
  }), /FAIL:.*frontend.*covered files.*nonempty manifest/i)
})

test('rejects malformed frontend count fields and relationships', () => {
  const invalidMetrics = [
    { total: 10.5, covered: 7, skipped: 0, pct: 70 },
    { total: 10, covered: -1, skipped: 0, pct: 70 },
    { total: 10, covered: 11, skipped: 0, pct: 70 },
    { total: 10, covered: 7, skipped: 4, pct: 70 },
  ]
  for (const lines of invalidMetrics) {
    const frontendFile = 'frontend/src/lib/api.ts'
    const { root, baseSha } = makeRepo([frontendFile])
    const frontendSummary = writeFrontendSummary(root, {
      [frontendFile]: frontendMetrics({ lines }),
    })
    expectFail(runChecker({
      root,
      manifest: manifest(baseSha, { frontend: [frontendFile] }),
      frontendSummary,
    }), /FAIL:.*frontend lines.*(count|relationship)/i)
  }
})

test('rejects frontend pct inconsistent with counts at two-decimal coverage semantics', () => {
  const frontendFile = 'frontend/src/lib/api.ts'
  const { root, baseSha } = makeRepo([frontendFile])
  const frontendSummary = writeFrontendSummary(root, {
    [frontendFile]: frontendMetrics({
      lines: { total: 3, covered: 2, skipped: 0, pct: 66.67 },
    }),
  })

  expectFail(runChecker({
    root,
    manifest: manifest(baseSha, { frontend: [frontendFile] }),
    frontendSummary,
  }), /FAIL:.*frontend lines.*pct.*counts.*66\.66/i)
})
