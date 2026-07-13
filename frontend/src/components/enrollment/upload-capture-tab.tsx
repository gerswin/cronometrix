'use client'
import { useEffect, useRef, useState } from 'react'
import { Upload, AlertTriangle, X } from 'lucide-react'
import type { CapturedPhotoCandidate } from '@/lib/enrollment-api'

interface UploadCaptureTabProps {
  onCaptured: (candidate: CapturedPhotoCandidate) => void
  onCleared: () => void
}

const MAX_SIZE = 2 * 1024 * 1024  // 2MB
const ERROR_MSG = 'El archivo debe ser JPG y pesar menos de 2 MB.'

export function UploadCaptureTab({ onCaptured, onCleared }: UploadCaptureTabProps) {
  const fileRef = useRef<HTMLInputElement | null>(null)
  const [file, setFile] = useState<File | null>(null)
  const [previewUrl, setPreviewUrl] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  // Revoke object URL on unmount or when file changes
  useEffect(() => {
    return () => {
      if (previewUrl) URL.revokeObjectURL(previewUrl)
    }
  }, [previewUrl])

  function handleFileChange(e: React.ChangeEvent<HTMLInputElement>) {
    const selected = e.target.files?.[0]
    if (!selected) return

    // Client-side validation: JPEG mime + ≤2MB
    if (!selected.type.match('image/jpeg') || selected.size > MAX_SIZE) {
      setError(ERROR_MSG)
      setFile(null)
      if (previewUrl) URL.revokeObjectURL(previewUrl)
      setPreviewUrl(null)
      // Reset input so same file can be re-selected after "Cambiar archivo"
      if (fileRef.current) fileRef.current.value = ''
      onCleared()
      return
    }

    setError(null)
    setFile(selected)
    const url = URL.createObjectURL(selected)
    setPreviewUrl(url)
    onCaptured({ blob: selected, capturedVia: 'upload', sourceDeviceId: null })
  }

  function reset() {
    setFile(null)
    setError(null)
    if (previewUrl) URL.revokeObjectURL(previewUrl)
    setPreviewUrl(null)
    if (fileRef.current) fileRef.current.value = ''
    onCleared()
  }

  return (
    <div className="space-y-3">
      {/* Hidden file input — accepts only JPEG */}
      <input
        ref={fileRef}
        type="file"
        accept="image/jpeg"
        className="hidden"
        onChange={handleFileChange}
        aria-label="Seleccionar archivo JPG"
      />

      {error && (
        <div
          role="alert"
          className="flex items-start gap-3 rounded-md bg-red-50 border border-red-200 px-4 py-3 text-sm text-red-700"
        >
          <AlertTriangle size={16} className="mt-0.5 shrink-0" />
          <span>{error}</span>
        </div>
      )}

      {!file ? (
        /* Drop zone */
        <button
          type="button"
          onClick={() => fileRef.current?.click()}
          className="flex flex-col items-center justify-center w-full h-40 border-2 border-dashed border-slate-300 rounded-lg hover:border-slate-400 hover:bg-slate-50 transition-colors cursor-pointer gap-2 text-slate-500"
        >
          <Upload size={24} className="text-slate-400" />
          <span className="text-sm">Haz clic para seleccionar un archivo JPG</span>
          <span className="text-xs text-slate-400">Máximo 2 MB</span>
        </button>
      ) : (
        /* Preview + change link */
        <div className="space-y-2">
          <div className="relative inline-block">
            {previewUrl && (
              // Blob-backed user preview cannot use the Next image optimizer.
              // eslint-disable-next-line @next/next/no-img-element
              <img
                src={previewUrl}
                alt="Vista previa"
                className="rounded-md object-cover border border-slate-200"
                style={{ width: 160, height: 160 }}
              />
            )}
            <button
              type="button"
              onClick={reset}
              className="absolute -top-2 -right-2 rounded-full bg-slate-700 text-white p-0.5 hover:bg-slate-900"
              aria-label="Quitar imagen"
            >
              <X size={12} />
            </button>
          </div>
          <p className="text-xs text-slate-500">{file.name}</p>
          <button
            type="button"
            onClick={reset}
            className="text-xs text-blue-600 hover:underline"
          >
            Cambiar archivo
          </button>
        </div>
      )}
    </div>
  )
}
