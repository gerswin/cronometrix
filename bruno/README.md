# Cronometrix — Bruno Collection

Manual API testing collection for the Cronometrix backend.

## Setup

1. Install Bruno: https://www.usebruno.com/downloads
2. Open Bruno → `Open Collection` → select `bruno/cronometrix/`
3. Activate environment: top-right dropdown → `local`
4. Edit `environments/local.bru` — set `username` and `password` to your admin user

## Recommended run order

1. **setup → status** — confirm backend is reachable
2. **setup → init** — create first admin (only on fresh DB)
3. **auth → login** — sets `access_token` env var; refresh cookie kept in jar
4. **departments → list** — populates `sample_department_id`
5. **employees → list** — populates `sample_employee_id`
6. **daily-records → list_week** — populates `sample_daily_record_id`
7. Run any other request — auth is already wired

Subsequent requests inherit the bearer token from the env var. Auto-refresh is not implemented in Bruno (re-run `auth → login` if you get 401).

## SSE limitation

Bruno's HTTP runner doesn't render `text/event-stream` live. Use:

```bash
TOKEN=$(curl -s -X POST http://localhost:3001/api/v1/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"USER","password":"PASS"}' | jq -r .access_token)

curl -N "http://localhost:3001/api/v1/events/stream?token=$TOKEN"
```

The connection stays open. Trigger a biometric event (or manually insert an attendance record) to see the JSON payload arrive.

## Notes

- All multipart endpoints (`leaves`, `daily-records/{id}/overrides`) accept evidence files up to 5MB (PDF/JPG/PNG only — magic-byte checked).
- Cookies (`refresh_token`) live in Bruno's cookie jar after `auth/login`. Don't clear them between requests.
- Endpoints under `/auth/*`, `/leaves` (POST), `/daily-records/{id}/overrides` (POST), and `/devices/{id}/commands` (POST) are admin-only.
