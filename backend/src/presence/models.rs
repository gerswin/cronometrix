use serde::Serialize;

/// Una fila por empleado con registro del día.
#[derive(Debug, Serialize)]
pub struct PresenceRow {
    pub employee_id: String,
    pub employee_name: String,
    pub department_name: String,
    /// "inside" = entró y no ha salido; "left" = ya marcó salida.
    pub status: String,
    pub entry_at: Option<String>, // ISO 8601 (D-13)
    pub exit_at: Option<String>,  // ISO 8601
    pub expected_min: i64,
    pub worked_min: i64,
    pub deficit_min: i64,
}

#[derive(Debug, Serialize)]
pub struct PresenceToday {
    pub date: String, // YYYY-MM-DD
    pub inside_now: i64,
    pub attended_today: i64,
    pub data: Vec<PresenceRow>,
}
