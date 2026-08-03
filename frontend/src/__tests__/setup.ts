import '@testing-library/jest-dom'

// jsdom's Blob does not implement `.stream()`. When MSW intercepts a binary
// response (the XLSX/PDF export handlers return a Blob body) it builds a native
// `Response` from that Blob, and undici's body extractor calls `blob.stream()`,
// throwing "object.stream is not a function". Polyfill it so the export-button
// tests can read mocked binary downloads.
if (typeof Blob !== 'undefined' && typeof Blob.prototype.stream !== 'function') {
  Blob.prototype.stream = function stream(this: Blob): ReadableStream<Uint8Array> {
    const blob = this
    return new ReadableStream<Uint8Array>({
      async start(controller) {
        const buffer = await blob.arrayBuffer()
        controller.enqueue(new Uint8Array(buffer))
        controller.close()
      },
    })
  }
}
