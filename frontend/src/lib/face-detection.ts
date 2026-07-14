// face-detection.ts — lazy load @vladmandic/face-api + per-frame analyzer
// Vendored tinyFaceDetector model served from /models/ (same-origin, no CDN)

let faceapiCache: typeof import('@vladmandic/face-api') | null = null
let modelLoaded = false

type TensorFlowRuntime = typeof import('@vladmandic/face-api')['tf'] & {
  setBackend(backendName: string): Promise<boolean>
  ready(): Promise<void>
}

function readWebGlRenderer(): string | null {
  if (typeof document === 'undefined') return null

  const canvas = document.createElement('canvas')
  const gl = canvas.getContext('webgl2') ?? canvas.getContext('webgl')
  if (!gl || typeof (gl as WebGLRenderingContext).getExtension !== 'function') return null

  const webGl = gl as WebGLRenderingContext
  const debugInfo = webGl.getExtension('WEBGL_debug_renderer_info')
  const renderer = webGl.getParameter(debugInfo?.UNMASKED_RENDERER_WEBGL ?? webGl.RENDERER)
  return typeof renderer === 'string' ? renderer : null
}

export function isSoftwareWebGlRenderer(renderer: string | null): boolean {
  if (!renderer) return false
  const normalized = renderer.toLowerCase()
  return normalized.includes('swiftshader') || normalized.includes('llvmpipe')
}

export async function configureFaceApiBackend(
  faceapi: typeof import('@vladmandic/face-api'),
  renderer: string | null = readWebGlRenderer(),
): Promise<void> {
  // face-api's bundled runtime exposes these TensorFlow.js methods even though
  // its generated declaration file omits them from the exported `tf` namespace.
  const tensorflow = faceapi.tf as TensorFlowRuntime
  if (isSoftwareWebGlRenderer(renderer)) {
    const selected = await tensorflow.setBackend('cpu')
    if (!selected) throw new Error('Unable to initialize CPU face-detection backend')
  }
  await tensorflow.ready()
}

export async function loadFaceApi(): Promise<typeof import('@vladmandic/face-api')> {
  if (!faceapiCache) {
    faceapiCache = await import('@vladmandic/face-api')
  }
  if (!modelLoaded) {
    await configureFaceApiBackend(faceapiCache)
    await faceapiCache.nets.tinyFaceDetector.loadFromUri('/models')
    modelLoaded = true
  }
  return faceapiCache
}

export interface FrameAnalysis {
  faceDetected: boolean
  luminanceOk: boolean
  sizeOk: boolean
  luminance: number
  width: number
  height: number
}

export type FaceImageSource = HTMLVideoElement | HTMLCanvasElement | HTMLImageElement

export async function analyzeFrame(
  source: FaceImageSource,
  sampleCanvas: HTMLCanvasElement,
  faceapi: typeof import('@vladmandic/face-api'),
): Promise<FrameAnalysis> {
  const det = await faceapi.detectSingleFace(
    source,
    new faceapi.TinyFaceDetectorOptions({ inputSize: 224, scoreThreshold: 0.5 })
  )
  const faceDetected = !!det
  const sizeOk = !!det && det.box.width >= 160 && det.box.height >= 160

  // Luminance — sample 64×48 pixels for speed
  sampleCanvas.width = 64
  sampleCanvas.height = 48
  const ctx = sampleCanvas.getContext('2d')!
  ctx.drawImage(source, 0, 0, 64, 48)
  const px = ctx.getImageData(0, 0, 64, 48).data
  let total = 0
  for (let i = 0; i < px.length; i += 4) {
    total += 0.299 * px[i] + 0.587 * px[i + 1] + 0.114 * px[i + 2]
  }
  const luminance = total / (px.length / 4)
  const luminanceOk = luminance >= 80 && luminance <= 200

  return {
    faceDetected,
    luminanceOk,
    sizeOk,
    luminance,
    width: det?.box.width ?? 0,
    height: det?.box.height ?? 0,
  }
}

export function isAcceptableFace(analysis: FrameAnalysis): boolean {
  return analysis.faceDetected && analysis.luminanceOk && analysis.sizeOk
}

export async function analyzePhotoBlob(blob: Blob): Promise<FrameAnalysis> {
  const objectUrl = URL.createObjectURL(blob)
  try {
    const image = await new Promise<HTMLImageElement>((resolve, reject) => {
      const nextImage = new Image()
      nextImage.onload = () => resolve(nextImage)
      nextImage.onerror = () => reject(new Error('Unable to decode captured photo'))
      nextImage.src = objectUrl
    })
    const faceapi = await loadFaceApi()
    return analyzeFrame(image, document.createElement('canvas'), faceapi)
  } finally {
    URL.revokeObjectURL(objectUrl)
  }
}
