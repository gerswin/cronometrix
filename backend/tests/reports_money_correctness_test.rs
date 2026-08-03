//! C-01/C-02: importes calculados a mano desde la LOTTT. La suite anterior
//! codificaba la especificación equivocada y no sirve de referencia.

use cronometrix_api::employees::models::SalaryKind;
use cronometrix_api::reports::money::{ot_pay_cents, total_a_pagar_cents, work_pay_cents};

/// LOTTT 118: recargo mínimo de 50% sobre la hora extra, o sea 1,5x.
///   480 min ordinarios          -> 5000
///   60 min extra a 1,5x         ->  937  (9,375 truncado)
///   total                       -> 5937
/// El defecto producía 6562: la hora extra cobrada al 250%.
#[test]
fn overtime_is_paid_once_at_150_percent_not_250() {
    let work = work_pay_cents(480, 5_000, 480, SalaryKind::Daily);
    // day_premium_pct=100: ordinary day, no night/rest-day premium (Important 2).
    let ot = ot_pay_cents(60, 5_000, 480, SalaryKind::Daily, 100);
    let total = total_a_pagar_cents(work, ot, 0, 0, 0);
    assert_eq!((work, ot, total), (5_000, 937, 5_937));
    assert_ne!(total, 6_562, "250% — el defecto C-01");
}

/// C-02: llegar 30 min tarde y salir a la hora nominal da 450 min trabajados.
/// Se pagan 450 y nada más: el salario de esos 30 min no se causó.
#[test]
fn lateness_costs_the_unworked_minutes_and_nothing_more() {
    let work = work_pay_cents(450, 5_000, 480, SalaryKind::Daily);
    let total = total_a_pagar_cents(work, 0, 0, 0, 0);
    assert_eq!(total, 4_687);
    assert_ne!(total, 4_375, "doble descuento — el defecto C-02");
}
