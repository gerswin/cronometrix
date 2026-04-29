/**
 * AuditEntry — mirrors backend/src/audit/models.rs::AuditEntry serialization.
 *
 * The audit_log table is append-only (schema-level constraint since migration 001).
 * old_data / new_data are stored as TEXT (JSON) in the DB and deserialized to
 * serde_json::Value by the backend, so the frontend receives plain JSON objects.
 *
 * Sort order: created_at DESC, id DESC (newest first, ID as deterministic tie-break).
 * RBAC: Admin + Supervisor read only; Viewer → 403.
 */
export interface AuditEntry {
  id: string
  table_name: string
  record_id: string
  operation: 'INSERT' | 'UPDATE' | 'DELETE'
  old_data: Record<string, unknown> | null
  new_data: Record<string, unknown> | null
  actor_id: string | null
  created_at: number   // epoch seconds
}
