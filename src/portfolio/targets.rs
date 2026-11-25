//! Reparto de pesos objetivo por bloque.
//!
//! Cambia solo porcentajes: no toca valores de mercado, liquidez ni
//! disparadores. La mixta y la liquidez quedan fijas; el reparto se hace
//! entre los activos puros del bloque elegido.

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::model::{Asset, AssetClass, Settings};

const EPS: f64 = 0.005;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetSleeve {
    Equity,
    FixedIncome,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetMode {
    Equal,
    Proportional,
}

/// Foto de los pesos objetivo actuales, separada por bloque.
#[derive(Clone, Debug, Serialize)]
pub struct TargetSummary {
    pub total_target_sum: f64,
    pub rv_target_sum: f64,
    pub rf_target_sum: f64,
    pub pure_equity_target_sum: f64,
    pub pure_fixed_income_target_sum: f64,
    pub mixed_rv_target_sum: f64,
    pub mixed_rf_target_sum: f64,
    pub cash_target_pct: f64,
    pub n_equity: usize,
    pub n_fixed_income: usize,
    pub n_mixed: usize,
    pub n_cash: usize,
    pub equal_equity_pct: Option<f64>,
    pub equal_equity_theoretical_pct: Option<f64>,
    pub equal_fixed_income_pct: Option<f64>,
    pub needs_equity: bool,
    pub needs_fixed_income: bool,
}

impl TargetSummary {
    pub fn compute(settings: &Settings, assets: &[Asset]) -> Self {
        let mut summary = TargetSummary {
            total_target_sum: 0.0,
            rv_target_sum: 0.0,
            rf_target_sum: 0.0,
            pure_equity_target_sum: 0.0,
            pure_fixed_income_target_sum: 0.0,
            mixed_rv_target_sum: 0.0,
            mixed_rf_target_sum: 0.0,
            cash_target_pct: 0.0,
            n_equity: 0,
            n_fixed_income: 0,
            n_mixed: 0,
            n_cash: 0,
            equal_equity_pct: None,
            equal_equity_theoretical_pct: None,
            equal_fixed_income_pct: None,
            needs_equity: false,
            needs_fixed_income: false,
        };

        if assets.is_empty() {
            return summary;
        }

        for asset in assets {
            let target = asset.target_weight_pct.max(0.0);
            let fixed_share = asset.fixed_income_pct.clamp(0.0, 100.0) / 100.0;
            let rf = target * fixed_share;
            let rv = target - rf;
            summary.total_target_sum += target;
            summary.rv_target_sum += rv;
            summary.rf_target_sum += rf;

            match asset.asset_class {
                AssetClass::Equity => {
                    summary.n_equity += 1;
                    summary.pure_equity_target_sum += target;
                }
                AssetClass::FixedIncome if asset.is_cash => {
                    summary.n_cash += 1;
                    summary.cash_target_pct += target;
                }
                AssetClass::FixedIncome => {
                    summary.n_fixed_income += 1;
                    summary.pure_fixed_income_target_sum += target;
                }
                AssetClass::Mixed => {
                    summary.n_mixed += 1;
                    summary.mixed_rv_target_sum += rv;
                    summary.mixed_rf_target_sum += rf;
                }
            }
        }

        summary.needs_equity = (summary.rv_target_sum - settings.target_equity_pct).abs() > EPS;
        summary.needs_fixed_income =
            (summary.rf_target_sum - settings.target_fixed_income_pct).abs() > EPS;

        if summary.n_equity > 0 {
            summary.equal_equity_theoretical_pct =
                Some(settings.target_equity_pct / summary.n_equity as f64);
            let remaining = settings.target_equity_pct - summary.mixed_rv_target_sum;
            if remaining >= -EPS {
                summary.equal_equity_pct = Some(remaining.max(0.0) / summary.n_equity as f64);
            }
        }

        if summary.n_fixed_income > 0 {
            let remaining = settings.target_fixed_income_pct
                - summary.cash_target_pct
                - summary.mixed_rf_target_sum;
            if remaining >= -EPS {
                summary.equal_fixed_income_pct =
                    Some(remaining.max(0.0) / summary.n_fixed_income as f64);
            }
        }

        summary
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct TargetChange {
    pub asset_id: Uuid,
    pub ticker: String,
    pub name: String,
    pub old_target_pct: f64,
    pub new_target_pct: f64,
    pub delta_pct: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct TargetPlan {
    pub sleeve: TargetSleeve,
    pub mode: TargetMode,
    pub changes: Vec<TargetChange>,
    pub rv_target_before: f64,
    pub rv_target_after: f64,
    pub rf_target_before: f64,
    pub rf_target_after: f64,
    pub total_before: f64,
    pub total_after: f64,
    pub warnings: Vec<String>,
}

/// Calcula el plan de reparto sin persistir nada.
pub fn plan_targets(
    settings: &Settings,
    assets: &[Asset],
    sleeve: TargetSleeve,
    mode: TargetMode,
) -> Result<TargetPlan, String> {
    let summary = TargetSummary::compute(settings, assets);
    let (selected, target_total, current_total, fixed_contribution) = match sleeve {
        TargetSleeve::Equity => {
            let selected: Vec<&Asset> = assets
                .iter()
                .filter(|asset| asset.asset_class == AssetClass::Equity)
                .collect();
            (
                selected,
                settings.target_equity_pct,
                summary.pure_equity_target_sum,
                summary.mixed_rv_target_sum,
            )
        }
        TargetSleeve::FixedIncome => {
            let selected: Vec<&Asset> = assets
                .iter()
                .filter(|asset| asset.asset_class == AssetClass::FixedIncome && !asset.is_cash)
                .collect();
            (
                selected,
                settings.target_fixed_income_pct,
                summary.pure_fixed_income_target_sum,
                summary.cash_target_pct + summary.mixed_rf_target_sum,
            )
        }
    };

    if selected.is_empty() {
        return Err(match sleeve {
            TargetSleeve::Equity => "No hay activos de renta variable pura para repartir".into(),
            TargetSleeve::FixedIncome => "No hay activos de renta fija pura para repartir".into(),
        });
    }

    let remaining = target_total - fixed_contribution;
    if remaining < -EPS {
        return Err(match sleeve {
            TargetSleeve::Equity => format!(
                "La mixta ya aporta {} de RV y supera el objetivo de {}. Bajá su peso antes de repartir",
                super::format::pct(summary.mixed_rv_target_sum),
                super::format::pct(settings.target_equity_pct)
            ),
            TargetSleeve::FixedIncome => format!(
                "La mixta y la liquidez ya aportan {} de RF y superan el objetivo de {}. Bajá esos pesos antes de repartir",
                super::format::pct(summary.cash_target_pct + summary.mixed_rf_target_sum),
                super::format::pct(settings.target_fixed_income_pct)
            ),
        });
    }
    let remaining = remaining.max(0.0);

    let targets = match mode {
        TargetMode::Equal => allocate_equal(selected.len(), remaining),
        TargetMode::Proportional => {
            if current_total > EPS {
                allocate_proportional(&selected, remaining, current_total)
            } else {
                allocate_equal(selected.len(), remaining)
            }
        }
    };

    let changes: Vec<TargetChange> = selected
        .iter()
        .zip(targets)
        .map(|(asset, new_target)| TargetChange {
            asset_id: asset.id,
            ticker: asset.ticker.clone().unwrap_or_default(),
            name: asset.name.clone(),
            old_target_pct: asset.target_weight_pct,
            new_target_pct: new_target,
            delta_pct: new_target - asset.target_weight_pct,
        })
        .collect();

    let new_pure_sum: f64 = changes.iter().map(|change| change.new_target_pct).sum();
    let (rv_target_after, rf_target_after) = match sleeve {
        TargetSleeve::Equity => (
            summary.rv_target_sum - current_total + new_pure_sum,
            summary.rf_target_sum,
        ),
        TargetSleeve::FixedIncome => (
            summary.rv_target_sum,
            summary.rf_target_sum - current_total + new_pure_sum,
        ),
    };
    let total_after = summary.total_target_sum - current_total + new_pure_sum;

    let mut warnings = Vec::new();
    if (total_after - 100.0).abs() > EPS {
        warnings.push(match sleeve {
            TargetSleeve::Equity => format!(
                "La renta fija queda en {}; su objetivo es {}. Repartí RF para cerrar el total.",
                super::format::pct(rf_target_after),
                super::format::pct(settings.target_fixed_income_pct)
            ),
            TargetSleeve::FixedIncome => format!(
                "La renta variable queda en {}; su objetivo es {}. Repartí RV para cerrar el total.",
                super::format::pct(rv_target_after),
                super::format::pct(settings.target_equity_pct)
            ),
        });
    }
    if mode == TargetMode::Proportional && current_total <= EPS {
        warnings.push("No había pesos previos en el bloque; se repartió en partes iguales.".into());
    }
    if remaining <= EPS {
        warnings.push(
            "La mixta y la liquidez ya cubren el objetivo del bloque; los activos puros quedan en 0 %."
                .into(),
        );
    }

    Ok(TargetPlan {
        sleeve,
        mode,
        changes,
        rv_target_before: summary.rv_target_sum,
        rv_target_after,
        rf_target_before: summary.rf_target_sum,
        rf_target_after,
        total_before: summary.total_target_sum,
        total_after,
        warnings,
    })
}

fn allocate_equal(count: usize, target: f64) -> Vec<f64> {
    if count == 0 {
        return Vec::new();
    }
    let cents = (target * 100.0).round().max(0.0) as i64;
    let base = cents / count as i64;
    let extra = (cents % count as i64) as usize;
    (0..count)
        .map(|index| {
            let value = base + i64::from(index < extra);
            value as f64 / 100.0
        })
        .collect()
}

fn allocate_proportional(assets: &[&Asset], target: f64, current_total: f64) -> Vec<f64> {
    let target_cents = (target * 100.0).round().max(0.0) as i64;
    let mut shares: Vec<(usize, f64, i64)> = assets
        .iter()
        .enumerate()
        .map(|(index, asset)| {
            let raw = asset.target_weight_pct / current_total * target * 100.0;
            (index, raw.fract(), raw.floor().max(0.0) as i64)
        })
        .collect();

    let mut cents: Vec<i64> = shares.iter().map(|(_, _, value)| *value).collect();
    let mut remaining = target_cents - cents.iter().sum::<i64>();
    shares.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    let mut cursor = 0;
    while remaining > 0 && !shares.is_empty() {
        cents[shares[cursor % shares.len()].0] += 1;
        remaining -= 1;
        cursor += 1;
    }

    cents
        .into_iter()
        .map(|value| value as f64 / 100.0)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::portfolio::model::{Economy, RiskProfile, Sector, SellTrigger, TriggerKind};

    fn settings() -> Settings {
        Settings::from_profile(RiskProfile::Aggressive)
    }

    fn asset(
        name: &str,
        class: AssetClass,
        fixed_income_pct: f64,
        target: f64,
        is_cash: bool,
    ) -> Asset {
        Asset {
            id: Uuid::new_v4(),
            ticker: Some(name.to_string()),
            name: name.to_string(),
            asset_class: class,
            fixed_income_pct,
            economy: Economy::Us,
            sector: Sector::Technology,
            market_value: 100.0,
            currency: "USD".into(),
            target_weight_pct: target,
            thesis: "tesis".into(),
            objective: "objetivo".into(),
            source: "manual".into(),
            is_cash,
            market_symbol: None,
            quantity: None,
            last_price: None,
            price_currency: None,
            priced_at: None,
            triggers: vec![SellTrigger {
                id: Uuid::new_v4(),
                kind: TriggerKind::ThesisBroken,
                description: "si cambia".into(),
                is_met: false,
                is_resolved: false,
            }],
        }
    }

    fn equity(name: &str, target: f64) -> Asset {
        asset(name, AssetClass::Equity, 0.0, target, false)
    }

    fn bond(name: &str, target: f64) -> Asset {
        asset(name, AssetClass::FixedIncome, 100.0, target, false)
    }

    fn cash(target: f64) -> Asset {
        asset("USD", AssetClass::FixedIncome, 100.0, target, true)
    }

    fn mixed(name: &str, fixed_income_pct: f64, target: f64) -> Asset {
        asset(name, AssetClass::Mixed, fixed_income_pct, target, false)
    }

    #[test]
    fn reparto_igual_rv_sin_mixta() {
        let mut assets = Vec::new();
        for name in ["A", "B", "C", "D", "E", "F", "G", "H"] {
            assets.push(equity(name, 10.0));
        }
        assets.push(bond("BONO", 20.0));
        let plan = plan_targets(
            &settings(),
            &assets,
            TargetSleeve::Equity,
            TargetMode::Equal,
        )
        .unwrap();
        assert_eq!(plan.changes.len(), 8);
        for change in &plan.changes {
            assert!((change.new_target_pct - 7.5).abs() < 0.001);
        }
        assert!((plan.rv_target_after - 60.0).abs() < 0.001);
        assert!((plan.rf_target_after - 20.0).abs() < 0.001);
        assert!((plan.total_after - 80.0).abs() < 0.001);
        assert!(plan.warnings.iter().any(|w| w.contains("renta fija")));
    }

    #[test]
    fn reparto_igual_rv_con_mixta_fija() {
        let mut assets = Vec::new();
        for name in ["A", "B", "C", "D", "E", "F", "G", "H"] {
            assets.push(equity(name, 10.0));
        }
        assets.push(mixed("FCI", 60.0, 8.0));
        assets.push(bond("BONO", 35.2));
        let plan = plan_targets(
            &settings(),
            &assets,
            TargetSleeve::Equity,
            TargetMode::Equal,
        )
        .unwrap();
        for change in &plan.changes {
            assert!((change.new_target_pct - 7.1).abs() < 0.001, "{change:?}");
        }
        assert!((plan.rv_target_after - 60.0).abs() < 0.001);
        assert!((plan.rf_target_after - 40.0).abs() < 0.001);
        assert!((plan.total_after - 100.0).abs() < 0.001);
    }

    #[test]
    fn reparto_igual_rf_con_liquidez_y_mixta_fijas() {
        let mut assets = vec![
            bond("AL30", 10.0),
            bond("GD30", 8.0),
            bond("YPFD", 3.0),
            cash(14.2),
            mixed("FCI", 60.0, 8.0),
        ];
        for name in ["A", "B", "C", "D", "E", "F", "G", "H"] {
            assets.push(equity(name, 7.1));
        }
        let plan = plan_targets(
            &settings(),
            &assets,
            TargetSleeve::FixedIncome,
            TargetMode::Equal,
        )
        .unwrap();
        for change in &plan.changes {
            assert!((change.new_target_pct - 7.0).abs() < 0.001, "{change:?}");
        }
        assert!((plan.rf_target_after - 40.0).abs() < 0.001);
        assert!((plan.rv_target_after - 60.0).abs() < 0.001);
        assert!((plan.total_after - 100.0).abs() < 0.001);
    }

    #[test]
    fn reparto_proporcional_rv() {
        let assets = vec![
            equity("AAPL", 40.0),
            equity("MSFT", 10.0),
            bond("BONO", 40.0),
        ];
        let plan = plan_targets(
            &settings(),
            &assets,
            TargetSleeve::Equity,
            TargetMode::Proportional,
        )
        .unwrap();
        let aapl = plan
            .changes
            .iter()
            .find(|change| change.ticker == "AAPL")
            .unwrap();
        let msft = plan
            .changes
            .iter()
            .find(|change| change.ticker == "MSFT")
            .unwrap();
        assert!((aapl.new_target_pct - 48.0).abs() < 0.001);
        assert!((msft.new_target_pct - 12.0).abs() < 0.001);
        assert!((plan.total_after - 100.0).abs() < 0.001);
    }

    #[test]
    fn reparto_proporcional_sin_pesos_previos_usa_iguales() {
        let assets = vec![equity("AAPL", 0.0), equity("MSFT", 0.0), bond("BONO", 40.0)];
        let plan = plan_targets(
            &settings(),
            &assets,
            TargetSleeve::Equity,
            TargetMode::Proportional,
        )
        .unwrap();
        for change in &plan.changes {
            assert!((change.new_target_pct - 30.0).abs() < 0.001);
        }
        assert!(
            plan.warnings
                .iter()
                .any(|warning| warning.contains("partes iguales"))
        );
    }

    #[test]
    fn redondeo_exacto_con_trece_activos() {
        let mut assets = Vec::new();
        for index in 0..13 {
            assets.push(equity(&format!("A{index}"), 4.0));
        }
        assets.push(bond("BONO", 40.0));
        let plan = plan_targets(
            &settings(),
            &assets,
            TargetSleeve::Equity,
            TargetMode::Equal,
        )
        .unwrap();
        let sum: f64 = plan
            .changes
            .iter()
            .map(|change| change.new_target_pct)
            .sum();
        assert!((sum - 60.0).abs() < 0.001, "suma {sum}");
        assert!((plan.total_after - 100.0).abs() < 0.001);
    }

    #[test]
    fn mixta_que_supera_el_objetivo_no_se_puede_repartir() {
        let assets = vec![equity("AAPL", 10.0), mixed("FCI", 20.0, 100.0)];
        let error = plan_targets(
            &settings(),
            &assets,
            TargetSleeve::Equity,
            TargetMode::Equal,
        )
        .unwrap_err();
        assert!(error.contains("supera"), "{error}");
    }

    #[test]
    fn sin_activos_puros_del_bloque_no_se_puede_repartir() {
        let assets = vec![bond("BONO", 40.0), mixed("FCI", 60.0, 60.0)];
        let error = plan_targets(
            &settings(),
            &assets,
            TargetSleeve::Equity,
            TargetMode::Equal,
        )
        .unwrap_err();
        assert!(error.contains("renta variable pura"), "{error}");
    }
}
