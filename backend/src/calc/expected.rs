//! Jornada esperada y déficit de horas.
//!
//! La esperada es `departments.ordinary_daily_minutes` — el mismo número contra
//! el que `engine::compute_daily_record` decide qué es hora extra
//! (`calc/engine.rs:134`) y contra el que `money.rs` convierte el sueldo a día
//! ordinario. Derivarla de `shift_start_time`/`shift_end_time` menos el almuerzo
//! crearía una segunda definición de jornada capaz de contradecir a la primera.
//!
//! El almuerzo no entra aquí: ya está descontado de `work_minutes` (en `fixed`
//! por `calc/lunch.rs`, en `punch` por el pareo de intervalos de
//! `calc/aggregation.rs`), y restarlo otra vez lo contaría dos veces.

use chrono::{Datelike, NaiveDate, Weekday};

/// Minutos que el empleado debía trabajar ese día. Devuelve 0 en fin de semana,
/// en días con permiso activo, y ante una jornada ordinaria no positiva.
pub fn expected_minutes(ordinary_daily_minutes: i64, date: NaiveDate, has_leave: bool) -> i64 {
    if has_leave || ordinary_daily_minutes <= 0 {
        return 0;
    }
    match date.weekday() {
        Weekday::Sat | Weekday::Sun => 0,
        _ => ordinary_daily_minutes,
    }
}

/// Minutos de jornada incumplidos. Nunca negativo: trabajar de más no compensa
/// un día corto.
pub fn deficit_minutes(expected: i64, worked: i64) -> i64 {
    (expected - worked).max(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn a_weekday_expects_the_departments_ordinary_day() {
        // 2026-08-05 es miércoles.
        assert_eq!(expected_minutes(480, d("2026-08-05"), false), 480);
    }

    #[test]
    fn weekends_expect_nothing() {
        // 2026-08-08 sábado, 2026-08-09 domingo.
        assert_eq!(expected_minutes(480, d("2026-08-08"), false), 0);
        assert_eq!(expected_minutes(480, d("2026-08-09"), false), 0);
    }

    #[test]
    fn a_leave_day_expects_nothing() {
        // El déficit mide incumplimiento real, no vacaciones.
        assert_eq!(expected_minutes(480, d("2026-08-05"), true), 0);
    }

    #[test]
    fn a_non_positive_ordinary_day_expects_nothing() {
        // Configuración inválida no debe producir déficit negativo (money.rs:47).
        assert_eq!(expected_minutes(0, d("2026-08-05"), false), 0);
        assert_eq!(expected_minutes(-60, d("2026-08-05"), false), 0);
    }

    #[test]
    fn deficit_is_the_shortfall_and_never_negative() {
        assert_eq!(deficit_minutes(480, 210), 270);
        assert_eq!(deficit_minutes(480, 480), 0);
        // Trabajar de más no compensa: las extras tienen su propia columna.
        assert_eq!(deficit_minutes(480, 600), 0);
        assert_eq!(deficit_minutes(0, 0), 0);
    }
}
