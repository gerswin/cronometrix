//! H-08: los importes se calcularon a mano. `base_salary_cents` sin unidad
//! explícita era ambiguo y un salario mensual se pagaba como si fuera diario.

use cronometrix_api::employees::models::SalaryKind;
use cronometrix_api::reports::money::work_pay_cents;

/// Jornada completa (480 min) con salario DIARIO de 50,00 -> 50,00.
#[test]
fn a_daily_salary_pays_itself_for_one_full_day() {
    assert_eq!(work_pay_cents(480, 5_000, 480, SalaryKind::Daily), 5_000);
}

/// Jornada completa con salario MENSUAL de 1.500,00 -> 50,00 (mensual/30).
/// El defecto H-08 pagaba 1.500,00 por ese mismo día.
#[test]
fn a_monthly_salary_is_divided_by_thirty_not_paid_whole() {
    assert_eq!(work_pay_cents(480, 150_000, 480, SalaryKind::Monthly), 5_000);
    assert_ne!(work_pay_cents(480, 150_000, 480, SalaryKind::Monthly), 150_000);
}

/// Jornada completa con salario POR HORA de 6,25 sobre 8 h -> 50,00.
#[test]
fn an_hourly_salary_scales_by_the_ordinary_day() {
    assert_eq!(work_pay_cents(480, 625, 480, SalaryKind::Hourly), 5_000);
}

/// La normalización NO puede hacerse en dos divisiones. `money.rs:3-4`
/// documenta "multiplicar numeradores primero, dividir una sola vez"; calcular
/// primero un salario diario y luego prorratear pierde hasta 29 centavos por
/// día en aritmética de centavos enteros.
///
/// NOTA sobre el plan: el caso original del plan (mensual 100.007, jornada de
/// 7 min) NO diverge — se recalculó a mano y da 48 por los dos caminos
/// (una fracción: 7*100_007/(30*480) = 700_049/14_400 = 48,61 -> 48;
/// dos divisiones: (100_007/30)=3_333 -> 3_333*7/480 = 23_331/480 = 48,61 -> 48),
/// así que ese número no habría detectado una regresión a dos divisiones.
/// Se sustituyó por jornada de 18 min, que sí diverge:
/// una fracción: 18 * 100_007 * 1 / (30 * 480) = 1_800_126 / 14_400 = 125,00875 -> 125
/// dos divisiones: floor(100_007/30) = 3_333 -> 3_333*18/480 = 59_994/480 = 124,9875 -> 124
/// Los dos caminos difieren (125 vs. 124): este es el caso que realmente
/// ejerce la regla "una sola fracción" en vez de solo coincidir con ella.
#[test]
fn normalization_uses_one_fraction_not_two_divisions() {
    assert_eq!(work_pay_cents(18, 100_007, 480, SalaryKind::Monthly), 125);
}
