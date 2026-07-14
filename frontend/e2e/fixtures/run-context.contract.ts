import assert from 'node:assert/strict'
import path from 'node:path'
import test from 'node:test'
import { tmpdir } from 'node:os'

// Keep the runtime `.ts` suffix for Node's built-in type stripping without
// requiring that suffix in the TypeScript module-resolution contract.
const runContext: typeof import('./run-context') = await import(`./run-context${'.ts'}`)
const { assertE2ETeardownPaths, resolveE2EPaths, validateE2ERunId } = runContext

test('accepts portable run ids and resolves direct children of the OS temp directory', () => {
  for (const runId of ['local', '123456789', 'run_12-02']) {
    assert.equal(validateE2ERunId(runId), runId)
    const paths = resolveE2EPaths(runId)
    assert.equal(path.dirname(paths.root), path.resolve(tmpdir()))
    assert.equal(path.basename(paths.root), `cronometrix-e2e-${runId}`)
    assert.equal(paths.dbPath, `${paths.root}.db`)
  }
})

test('rejects empty, traversal, absolute, separator, and non-portable run ids', () => {
  for (const runId of [
    '',
    '.',
    '..',
    '../escape',
    '/tmp/escape',
    'nested/escape',
    String.raw`nested\escape`,
    'drive:C',
    'white space',
  ]) {
    assert.throws(() => validateE2ERunId(runId), /run id/i, runId)
  }
})

test('teardown containment rejects paths that are not the validated run direct children', () => {
  const safe = resolveE2EPaths('safe-run')
  assert.doesNotThrow(() => assertE2ETeardownPaths('safe-run', safe.root, safe.dbPath))
  assert.throws(
    () => assertE2ETeardownPaths('safe-run', tmpdir(), safe.dbPath),
    /containment/i,
  )
  assert.throws(
    () => assertE2ETeardownPaths('safe-run', safe.root, path.join(tmpdir(), 'other.db')),
    /containment/i,
  )
})
