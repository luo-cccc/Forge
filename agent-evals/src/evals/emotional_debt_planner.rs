#![allow(unused_imports)]
use crate::fixtures::*;
use agent_writer_lib::writer_agent::promise_planner::emotional_debt_pressure;

pub fn run_emotional_debt_planner_eval() -> EvalResult {
    let mut errors = Vec::new();

    // Scenario A: overdue with 3 debts, 12 chapters overdue
    let pressure_overdue = emotional_debt_pressure(3, 12);
    if pressure_overdue <= 1.0 {
        errors.push(format!(
            "expected pressure > 1.0 for overdue case (3 debts, 12 chapters), got {}",
            pressure_overdue
        ));
    }

    // Scenario B: no debts, no overdue
    let pressure_zero = emotional_debt_pressure(0, 0);
    if (pressure_zero - 1.0).abs() > f64::EPSILON {
        errors.push(format!(
            "expected pressure = 1.0 for zero case, got {}",
            pressure_zero
        ));
    }

    // Scenario C: 1 debt, 3 chapters overdue — should be > 1.0 but less than overdue case
    let pressure_mild = emotional_debt_pressure(1, 3);
    if pressure_mild <= 1.0 {
        errors.push(format!(
            "expected pressure > 1.0 for mild case (1 debt, 3 chapters), got {}",
            pressure_mild
        ));
    }
    if pressure_mild >= pressure_overdue {
        errors.push(format!(
            "expected mild pressure ({}) < overdue pressure ({})",
            pressure_mild, pressure_overdue
        ));
    }

    // Scenario D: 5 debts with >5 chapters overdue (should be > 1.0)
    let pressure_5_debts = emotional_debt_pressure(5, 8);
    if pressure_5_debts <= 1.0 {
        errors.push(format!(
            "expected pressure > 1.0 for 5 debts (8 chapters overdue), got {}",
            pressure_5_debts
        ));
    }

    eval_result(
        "writer_agent:emotional_debt_planner",
        format!(
            "overdue={} zero={} mild={} five_debts={}",
            pressure_overdue, pressure_zero, pressure_mild, pressure_5_debts
        ),
        errors,
    )
}
