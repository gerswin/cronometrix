import assert from 'node:assert/strict'
import test from 'node:test'

const selectorsModule: typeof import('./selectors') = await import(`./selectors${'.ts'}`)

test('password selector metadata uses its label without an implicit textbox role', () => {
  assert.deepEqual(selectorsModule.SEL.loginPassword, { name: 'Contraseña' })
})
