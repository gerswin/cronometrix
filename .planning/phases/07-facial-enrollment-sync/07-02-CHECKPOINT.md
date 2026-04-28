---
type: human-verify
plan: 07-02
task: 5
created: "2026-04-28T05:00:00Z"
---

# Checkpoint: Task 5 — Manual Smoke Test

**Type:** human-verify
**Plan:** 07-02 (Enrollment Frontend)
**Progress:** 4/5 tasks complete — all automated work done

## Completed Tasks

| Task | Name | Commit | Key Files |
|------|------|--------|-----------|
| 1 | Wave 0 packages + models + primitives + types | 254a072 | package.json, public/models/*, 6 ui/*.tsx, types/api.ts, validations.ts, access-restricted.tsx, 6 test stubs |
| 2 | face-detection.ts + ValidationPanel + Webcam + Upload | cf540d5 | face-detection.ts, validation-panel.tsx, webcam-capture-tab.tsx, upload-capture-tab.tsx |
| 3 | KioskCaptureTab + SyncPanel + SyncRow | 0e3c144 | kiosk-capture-tab.tsx, sync-panel.tsx, sync-row.tsx |
| 4 | EnrollmentModal + page replacement + employee-table | 8a8f5b1 | enrollment-modal.tsx, enrollment/page.tsx, employees/page.tsx, employee-table.tsx |
| fix | Kiosk test stability | 8067503 | kiosk-capture-tab.test.tsx |

## Automated Verification Passed

- **25/25 vitest tests green** (6 test files, all enrollment components)
- **TypeScript clean** (`tsc --noEmit` exits 0)
- All 6 shadcn primitives installed, @vladmandic/face-api@^1.7.15 in dependencies
- tinyFaceDetector model vendored (193KB shard + manifest, same-origin /models/)

## Current Task

**Task 5:** Manual smoke test against running backend (admin + supervisor flows)
**Status:** Awaiting human verification
**Blocked by:** Needs running backend (07-01) + browser with camera for webcam test

## Verification Steps

Run both servers:
```bash
# Terminal 1 — backend (07-01 must be built)
cd backend && cargo run

# Terminal 2 — frontend (needs HTTPS for getUserMedia)
cd frontend && npm run dev -- --experimental-https
```

Then verify all 14 steps:

1. Navigate to `/enrollment` as admin — confirm header "Enrolamiento Facial", picker dropdown, empty state visible
2. Pick an employee from the picker — confirm modal opens with employee name in title
3. Click "Webcam" tab — confirm browser prompts for camera permission, video preview appears
4. AI Validation panel — 3 rows (Rostro Detectado, Buena Iluminación, Resolución Óptima) flip to "OK" when face visible; primary "Enrolar" button enables
5. Click "Capturar Rostro" — frame freezes; click Aceptar — modal shows sync panel
6. SyncPanel rows cycle: Esperando → Enviando → Sincronizado (green) or Falló (red)
7. Close modal mid-sync via Cerrar — sticky Sonner toast "Enrolamiento en curso — X/Y dispositivos" appears at bottom
8. Click "Subir JPG" tab — drop a 100KB JPEG → preview shown; drop a PNG → Spanish error; drop a 3MB JPG → Spanish error
9. Click "Lector Hikvision" tab — select a device → Iniciar Captura → 30s countdown → preview (or timeout banner)
10. On a failed sync row, click Reintentar — row status cycles back through Enviando
11. Log in as supervisor — navigate to /enrollment — "Acceso restringido" shown; picker and modal NOT visible
12. Navigate to /employees as admin — each row has UserPlus icon; clicking opens modal preselected to that employee

## Resume Signal

Type **"approved"** to mark Task 5 complete and finalize Phase 7.

If any step fails, describe the failing step number and observed behavior — the executor will fix before finalizing.
