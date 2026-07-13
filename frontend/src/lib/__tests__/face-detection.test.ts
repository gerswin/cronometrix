/**
 * Coverage for src/lib/face-detection.ts.
 *
 * The module has two exports:
 *   - loadFaceApi: lazy-imports `@vladmandic/face-api` and loads the
 *     tinyFaceDetector model from /models. We mock the dynamic import
 *     via vi.mock so we don't try to fetch a WASM-backed model in jsdom.
 *   - analyzeFrame: pure-ish helper that combines a face detection result
 *     with manual luminance sampling on a canvas. We mock the canvas
 *     getContext / drawImage / getImageData paths.
 *
 * UPDATE: face-detection IS testable in jsdom when the dynamic
 * `@vladmandic/face-api` import is intercepted at the module-loader
 * boundary by vi.mock. No exclusion needed.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest'

const { loadFromUriMock, detectSingleFaceMock } = vi.hoisted(() => ({
  loadFromUriMock: vi.fn(),
  detectSingleFaceMock: vi.fn(),
}))

vi.mock('@vladmandic/face-api', () => ({
  nets: { tinyFaceDetector: { loadFromUri: loadFromUriMock } },
  TinyFaceDetectorOptions: class {
    constructor(public opts: unknown) {}
  },
  detectSingleFace: detectSingleFaceMock,
}))

beforeEach(() => {
  vi.clearAllMocks()
  loadFromUriMock.mockResolvedValue(undefined)
  // Re-import the module fresh per test so cached state (faceapiCache,
  // modelLoaded) does not leak across cases.
  vi.resetModules()
})

function makeVideo(): HTMLVideoElement {
  const v = document.createElement('video')
  return v
}

function makeImage(): HTMLImageElement {
  return document.createElement('img')
}

function makeCanvas(samplePixels: Uint8ClampedArray): HTMLCanvasElement {
  const canvas = document.createElement('canvas')
  // Stub getContext('2d') so it returns a context with the methods we need
  const ctx = {
    drawImage: vi.fn(),
    getImageData: vi.fn(() => ({ data: samplePixels })),
  }
  canvas.getContext = vi.fn(() => ctx) as unknown as typeof canvas.getContext
  return canvas
}

function pixels(rgbValue: number, count = 64 * 48): Uint8ClampedArray {
  const arr = new Uint8ClampedArray(count * 4)
  for (let i = 0; i < arr.length; i += 4) {
    arr[i] = rgbValue
    arr[i + 1] = rgbValue
    arr[i + 2] = rgbValue
    arr[i + 3] = 255
  }
  return arr
}

describe('lib/face-detection', () => {
  it('loadFaceApi resolves to the face-api module and triggers tinyFaceDetector.loadFromUri once', async () => {
    const { loadFaceApi } = await import('../face-detection')
    const fa = await loadFaceApi()
    expect(fa).toBeTruthy()
    expect(loadFromUriMock).toHaveBeenCalledTimes(1)
    expect(loadFromUriMock).toHaveBeenCalledWith('/models')
  })

  it('loadFaceApi caches the module and the model load (second call is a no-op)', async () => {
    const { loadFaceApi } = await import('../face-detection')
    await loadFaceApi()
    await loadFaceApi()
    // Cached: loadFromUri only invoked the first time
    expect(loadFromUriMock).toHaveBeenCalledTimes(1)
  })

  it('analyzeFrame returns faceDetected=true and sizeOk=true when face box >= 160x160', async () => {
    detectSingleFaceMock.mockResolvedValueOnce({
      box: { width: 200, height: 200 },
    })
    const { loadFaceApi, analyzeFrame } = await import('../face-detection')
    const fa = await loadFaceApi()
    const px = pixels(120) // mid luminance ~120 -> ok
    const result = await analyzeFrame(makeVideo(), makeCanvas(px), fa)
    expect(result.faceDetected).toBe(true)
    expect(result.sizeOk).toBe(true)
    expect(result.luminanceOk).toBe(true)
    expect(result.luminance).toBeCloseTo(120, 0)
    expect(result.width).toBe(200)
    expect(result.height).toBe(200)
  })

  it('analyzeFrame accepts a still image source and samples that exact image', async () => {
    detectSingleFaceMock.mockResolvedValueOnce({ box: { width: 200, height: 200 } })
    const { loadFaceApi, analyzeFrame } = await import('../face-detection')
    const fa = await loadFaceApi()
    const image = makeImage()
    const canvas = makeCanvas(pixels(120))

    await analyzeFrame(image, canvas, fa)

    const context = vi.mocked(canvas.getContext).mock.results[0].value as unknown as {
      drawImage: ReturnType<typeof vi.fn>
    }
    expect(context.drawImage).toHaveBeenCalledWith(image, 0, 0, 64, 48)
  })

  it('analyzeFrame returns sizeOk=false when face box smaller than 160x160', async () => {
    detectSingleFaceMock.mockResolvedValueOnce({
      box: { width: 100, height: 100 },
    })
    const { loadFaceApi, analyzeFrame } = await import('../face-detection')
    const fa = await loadFaceApi()
    const px = pixels(120)
    const result = await analyzeFrame(makeVideo(), makeCanvas(px), fa)
    expect(result.faceDetected).toBe(true)
    expect(result.sizeOk).toBe(false)
  })

  it('analyzeFrame returns faceDetected=false when no detection', async () => {
    detectSingleFaceMock.mockResolvedValueOnce(undefined)
    const { loadFaceApi, analyzeFrame } = await import('../face-detection')
    const fa = await loadFaceApi()
    const px = pixels(120)
    const result = await analyzeFrame(makeVideo(), makeCanvas(px), fa)
    expect(result.faceDetected).toBe(false)
    expect(result.sizeOk).toBe(false)
    expect(result.width).toBe(0)
    expect(result.height).toBe(0)
  })

  it('analyzeFrame reports luminanceOk=false for very dark frames (luminance < 80)', async () => {
    detectSingleFaceMock.mockResolvedValueOnce({ box: { width: 200, height: 200 } })
    const { loadFaceApi, analyzeFrame } = await import('../face-detection')
    const fa = await loadFaceApi()
    const result = await analyzeFrame(makeVideo(), makeCanvas(pixels(20)), fa)
    expect(result.luminanceOk).toBe(false)
    expect(result.luminance).toBeLessThan(80)
  })

  it('analyzeFrame reports luminanceOk=false for very bright (overexposed) frames (>200)', async () => {
    detectSingleFaceMock.mockResolvedValueOnce({ box: { width: 200, height: 200 } })
    const { loadFaceApi, analyzeFrame } = await import('../face-detection')
    const fa = await loadFaceApi()
    const result = await analyzeFrame(makeVideo(), makeCanvas(pixels(240)), fa)
    expect(result.luminanceOk).toBe(false)
    expect(result.luminance).toBeGreaterThan(200)
  })

  it('analyzePhotoBlob analyzes the decoded image and always revokes its object URL', async () => {
    detectSingleFaceMock.mockResolvedValueOnce({ box: { width: 200, height: 200 } })
    const sampleCanvas = makeCanvas(pixels(120))
    const createElement = vi.spyOn(document, 'createElement').mockImplementation(((tag: string) => {
      if (tag === 'canvas') return sampleCanvas
      return document.createElementNS('http://www.w3.org/1999/xhtml', tag)
    }) as typeof document.createElement)
    const createObjectURL = vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:face-photo')
    const revokeObjectURL = vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => {})

    class LoadedImage {
      onload: (() => void) | null = null
      onerror: (() => void) | null = null
      naturalWidth = 640
      naturalHeight = 480

      set src(_value: string) {
        queueMicrotask(() => this.onload?.())
      }
    }
    vi.stubGlobal('Image', LoadedImage)

    const { analyzePhotoBlob } = await import('../face-detection')
    const result = await analyzePhotoBlob(new Blob(['jpeg'], { type: 'image/jpeg' }))

    expect(result).toMatchObject({ faceDetected: true, luminanceOk: true, sizeOk: true })
    expect(createObjectURL).toHaveBeenCalledOnce()
    expect(revokeObjectURL).toHaveBeenCalledWith('blob:face-photo')

    createElement.mockRestore()
    createObjectURL.mockRestore()
    revokeObjectURL.mockRestore()
    vi.unstubAllGlobals()
  })

  it.each([
    ['missing face', { faceDetected: false, luminanceOk: true, sizeOk: true }],
    ['bad luminance', { faceDetected: true, luminanceOk: false, sizeOk: true }],
    ['small face', { faceDetected: true, luminanceOk: true, sizeOk: false }],
  ])('isAcceptableFace rejects %s', async (_label, flags) => {
    const { isAcceptableFace } = await import('../face-detection')
    expect(isAcceptableFace({ ...flags, luminance: 120, width: 200, height: 200 })).toBe(false)
  })

  it('isAcceptableFace accepts only the three green checks', async () => {
    const { isAcceptableFace } = await import('../face-detection')
    expect(isAcceptableFace({
      faceDetected: true,
      luminanceOk: true,
      sizeOk: true,
      luminance: 120,
      width: 200,
      height: 200,
    })).toBe(true)
  })
})
