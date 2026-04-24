import { describe, it, expect } from 'vitest'
import { addToRingBuffer } from '../lib/ring-buffer'

describe('addToRingBuffer', () => {
  it('keeps at most 20 items', () => {
    let buf: number[] = []
    for (let i = 0; i < 25; i++) {
      buf = addToRingBuffer(buf, i, 20)
    }
    expect(buf.length).toBe(20)
  })
  it('newest item is first', () => {
    let buf: number[] = []
    buf = addToRingBuffer(buf, 1, 20)
    buf = addToRingBuffer(buf, 2, 20)
    expect(buf[0]).toBe(2)
  })
})
