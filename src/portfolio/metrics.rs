//! Motor de medición de la cartera contra la estrategia.
//!
//! Puro: recibe criterios, activos y una tasa opcional; devuelve el
//! diagnóstico completo. Toda la plata se convierte a la moneda de la
//! vista antes de pesar: los porcentajes son comparables entre ARS y USD.

use std::collections::HashMap;

use serde::Serialize;
use uuid::Uuid;

use super::format::{pct, pp};
use super::model::{Asset, AssetClass, Economy, Nature, Sector, Settings, Valuation};
use super::targets::TargetSummary;

#[derive(Clone, Debug, Serialize)]
pub struct Slice {
    pub id: String,
    pub label: String,
    pub value: f64,
    pub pct: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Check {
    pub title: String,
    pub detail: String,
    pub note: Option<String>,
    pub weight: f64,
    pub points: f64,
    pub state: String,
    pub state_label: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct RebalanceOrder {
    pub asset_id: Uuid,
    pub action: String,
    pub ticker: String,
    pub name: String,
    pub deviation_pp: f64,
    pub amount: f64,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct AssetView {
    pub id: Uuid,
    pub ticker: String,
    pub name: String,
    pub class_label: String,
    pub class_css: String,
    pub is_cash: bool,
    pub economy_label: String,
    pub sector_label: String,
    pub nature_label: String,
    pub value_native: f64,
    pub currency: String,
    pub value_display: f64,
    pub current_weight_pct: f64,
    pub target_weight_pct: f64,
    pub deviation_pp: f64,
    pub deviation_css: String,
    pub n_triggers: usize,
    pub n_met: usize,
    pub n_pending: usize,
    pub thesis_ok: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct PendingTrigger {
    pub id: Uuid,
    pub asset_id: Uuid,
    pub asset: String,
    pub kind: String,
    pub description: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Drawdown {
    pub drop_pct: f64,
    pub recovery_pct: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Composition {
    pub classes: Vec<Slice>,
    pub economies: Vec<Slice>,
    pub sectors: Vec<Slice>,
    pub natures: Vec<Slice>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Analysis {
    pub display_currency: String,
    pub rate_used: Option<f64>,
    pub total_display: f64,
    pub total_ars: Option<f64>,
    pub total_usd: Option<f64>,
    pub n_assets: usize,
    pub n_equity: usize,
    pub rf_pct: f64,
    pub rv_pct: f64,
    pub mixed_pct: f64,
    pub rf_target_pct: f64,
    pub rv_target_pct: f64,
    pub rf_deviation_pp: f64,
    pub hhi: f64,
    pub effective_units: f64,
    pub n_economies: usize,
    pub max_economy_pct: f64,
    pub max_economy_label: String,
    pub max_asset_pct: f64,
    pub max_asset_label: String,
    pub max_cluster_pct: f64,
    pub max_cluster_label: String,
    pub target_weight_sum: f64,
    pub targets: TargetSummary,
    pub score: u8,
    pub status: String,
    pub status_detail: String,
    pub composition: Composition,
    pub checks: Vec<Check>,
    pub orders: Vec<RebalanceOrder>,
    pub rows: Vec<AssetView>,
    pub pending_triggers: Vec<PendingTrigger>,
    pub drawdown: Vec<Drawdown>,
}

/// Convierte un valor nativo a la moneda de la vista. Si la moneda
/// coincide no hace falta tasa.
fn convert(value: f64, from: &str, display: &str, rate: Option<f64>) -> f64 {
    if from == display {
        return value;
    }
    let Some(rate) = rate.filter(|r| r.is_finite() && *r > 0.0) else {
        return 0.0;
    };
    match (from, display) {
        ("USD", "ARS") => value * rate,
        ("ARS", "USD") => value / rate,
        _ => value,
    }
}

fn check(title: &str, detail: String, weight: f64, points: f64) -> Check {
    let (state, state_label) = if points >= 0.999 {
        ("ok", "BIEN")
    } else if points >= 0.499 {
        ("review", "A REVISAR")
    } else {
        ("alert", "ALERTA")
    };
    Check {
        title: title.to_string(),
        detail,
        note: None,
        weight,
        points,
        state: state.to_string(),
        state_label: state_label.to_string(),
    }
}

/// Banda en puntos porcentuales sin signo: `5,0 pp`.
fn band_text(value: f64) -> String {
    format!("{value:.1}\u{00A0}pp").replace('.', ",")
}

fn slices_from<K>(
    map: HashMap<K, f64>,
    total: f64,
    id: impl Fn(K) -> &'static str,
    label: impl Fn(K) -> &'static str,
) -> Vec<Slice>
where
    K: Copy + Eq + std::hash::Hash,
{
    let mut slices: Vec<Slice> = map
        .into_iter()
        .filter(|(_, value)| *value > 0.0)
        .map(|(key, value)| Slice {
            id: id(key).to_string(),
            label: label(key).to_string(),
            value,
            pct: if total > 0.0 {
                value / total * 100.0
            } else {
                0.0
            },
        })
        .collect();
    slices.sort_by(|a, b| {
        b.value
            .partial_cmp(&a.value)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    slices
}

/// Cartera sin valores: el diagnóstico existe pero no puntúa.
fn no_value_analysis(
    settings: &Settings,
    assets: &[Asset],
    display_currency: &str,
    rate: Option<f64>,
    status: &str,
    detail: &str,
) -> Analysis {
    let targets = TargetSummary::compute(settings, assets);
    let disabled =
        |title: &str, weight: f64| check(title, "Sin valores para medir.".to_string(), weight, 0.0);
    let checks = vec![
        disabled("Ancla y motor", 12.0),
        disabled("Cantidad de activos de renta variable", 8.0),
        disabled("Economías", 10.0),
        disabled("Dependencia de una economía", 8.0),
        disabled("Diversificación real", 10.0),
        disabled("Peso máximo por activo", 10.0),
        disabled("Activos que se mueven junto", 10.0),
        disabled("Cíclico vs defensivo", 5.0),
        disabled("Disparadores de venta", 15.0),
        disabled("Tesis y objetivo", 5.0),
        disabled("Los pesos objetivo suman 100 %", 5.0),
    ];
    Analysis {
        display_currency: display_currency.to_string(),
        rate_used: rate,
        total_display: 0.0,
        total_ars: Some(0.0),
        total_usd: Some(0.0),
        n_assets: assets.len(),
        n_equity: targets.n_equity,
        rf_pct: 0.0,
        rv_pct: 0.0,
        mixed_pct: 0.0,
        rf_target_pct: settings.target_fixed_income_pct,
        rv_target_pct: settings.target_equity_pct,
        rf_deviation_pp: 0.0,
        hhi: 0.0,
        effective_units: 0.0,
        n_economies: 0,
        max_economy_pct: 0.0,
        max_economy_label: String::new(),
        max_asset_pct: 0.0,
        max_asset_label: String::new(),
        max_cluster_pct: 0.0,
        max_cluster_label: String::new(),
        target_weight_sum: targets.total_target_sum,
        targets,
        score: 0,
        status: status.to_string(),
        status_detail: detail.to_string(),
        composition: Composition {
            classes: Vec::new(),
            economies: Vec::new(),
            sectors: Vec::new(),
            natures: Vec::new(),
        },
        checks,
        orders: Vec::new(),
        rows: Vec::new(),
        pending_triggers: Vec::new(),
        drawdown: drawdown_table(),
    }
}

/// Cuenta de reparto igual de la RV para la tarjeta de cantidad.
fn equity_repartition_note(settings: &Settings, targets: &TargetSummary) -> Option<String> {
    let theoretical = targets.equal_equity_theoretical_pct?;
    let count = targets.n_equity;
    let simple = format!(
        "Reparto igual: {} ({} ÷ {}).",
        pct(theoretical),
        pct(settings.target_equity_pct),
        count
    );
    match targets.equal_equity_pct {
        Some(real) if (real - theoretical).abs() > 0.05 => Some(format!(
            "{} Con mixta fija: {} ({} ÷ {}).",
            simple,
            pct(real),
            pct(settings.target_equity_pct - targets.mixed_rv_target_sum),
            count
        )),
        Some(_) => Some(simple),
        None => Some(
            "Reparto igual: la mixta ya ocupa el objetivo de RV; no queda porcentaje para las RV puras."
                .to_string(),
        ),
    }
}

/// Analiza una cartera completa. `rate` es ARS por USD en la tasa que
/// pide la vista; solo es obligatoria si hay activos en la otra moneda.
pub fn analyze(
    settings: &Settings,
    assets: &[Asset],
    valuation: Valuation,
    rate: Option<f64>,
) -> Result<Analysis, String> {
    let display_currency = valuation.display_currency();
    let needs_rate = assets.iter().any(|a| a.currency != display_currency);
    if needs_rate && rate.is_none() {
        return Err(format!(
            "Falta la tasa {} para valuar la cartera en {}",
            valuation.rate_kind().label(),
            display_currency
        ));
    }

    if assets.is_empty() {
        return Ok(no_value_analysis(
            settings,
            assets,
            display_currency,
            rate,
            "Sin datos",
            "Sin cartera cargada.",
        ));
    }

    let mut rows: Vec<AssetView> = Vec::with_capacity(assets.len());
    let mut total = 0.0_f64;
    let mut rf_value = 0.0_f64;
    let mut rv_value = 0.0_f64;
    let mut mixed_value = 0.0_f64;
    let mut economies: HashMap<Economy, f64> = HashMap::new();
    let mut sectors: HashMap<Sector, f64> = HashMap::new();
    let mut natures: HashMap<Nature, f64> = HashMap::new();
    let mut clusters: HashMap<(Economy, Sector), f64> = HashMap::new();
    let mut without_triggers: Vec<String> = Vec::new();
    let mut without_thesis: Vec<String> = Vec::new();
    let mut pending_triggers: Vec<PendingTrigger> = Vec::new();

    for asset in assets {
        let value = convert(asset.market_value, &asset.currency, display_currency, rate);
        let fixed_share = asset.fixed_income_pct.clamp(0.0, 100.0);
        let rf = value * fixed_share / 100.0;
        let rv = value - rf;
        total += value;
        rf_value += rf;
        rv_value += rv;
        if asset.asset_class == AssetClass::Mixed {
            mixed_value += value;
        }
        *economies.entry(asset.economy).or_insert(0.0) += value;
        *sectors.entry(asset.sector).or_insert(0.0) += value;
        *natures.entry(asset.sector.nature()).or_insert(0.0) += rv;
        *clusters.entry((asset.economy, asset.sector)).or_insert(0.0) += value;

        if !asset.is_cash && asset.triggers.is_empty() {
            without_triggers.push(asset.display().to_string());
        }
        if !asset.is_cash && !asset.has_thesis_and_objective() {
            without_thesis.push(asset.display().to_string());
        }
        if !asset.is_cash {
            for trigger in asset.triggers.iter().filter(|t| t.is_met && !t.is_resolved) {
                pending_triggers.push(PendingTrigger {
                    id: trigger.id,
                    asset_id: asset.id,
                    asset: asset.display().to_string(),
                    kind: trigger.kind.label().to_string(),
                    description: trigger.description.clone(),
                });
            }
        }

        rows.push(AssetView {
            id: asset.id,
            ticker: asset.ticker.clone().unwrap_or_default(),
            name: asset.name.clone(),
            class_label: asset.asset_class.label().to_string(),
            class_css: asset.asset_class.css().to_string(),
            is_cash: asset.is_cash,
            economy_label: asset.economy.label().to_string(),
            sector_label: asset.sector.label().to_string(),
            nature_label: asset.sector.nature().label().to_string(),
            value_native: asset.market_value,
            currency: asset.currency.clone(),
            value_display: value,
            current_weight_pct: 0.0,
            target_weight_pct: asset.target_weight_pct,
            deviation_pp: 0.0,
            deviation_css: "neutral".to_string(),
            n_triggers: if asset.is_cash {
                0
            } else {
                asset.triggers.len()
            },
            n_met: if asset.is_cash {
                0
            } else {
                asset.triggers.iter().filter(|t| t.is_met).count()
            },
            n_pending: if asset.is_cash {
                0
            } else {
                asset
                    .triggers
                    .iter()
                    .filter(|t| t.is_met && !t.is_resolved)
                    .count()
            },
            thesis_ok: asset.is_cash || asset.has_thesis_and_objective(),
        });
    }

    if total <= 0.0 {
        return Ok(no_value_analysis(
            settings,
            assets,
            display_currency,
            rate,
            "Sin valuar",
            "Los activos cargados no tienen valor. Revisá los montos antes de medir.",
        ));
    }

    let band = settings.rebalance_band_pp;
    for row in &mut rows {
        row.current_weight_pct = row.value_display / total * 100.0;
        row.deviation_pp = row.current_weight_pct - row.target_weight_pct;
        row.deviation_css = if row.deviation_pp > band {
            "over".to_string()
        } else if row.deviation_pp < -band {
            "under".to_string()
        } else {
            "neutral".to_string()
        };
    }
    rows.sort_by(|a, b| {
        b.value_display
            .partial_cmp(&a.value_display)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let rf_pct = rf_value / total * 100.0;
    let rv_pct = rv_value / total * 100.0;
    let mixed_pct = mixed_value / total * 100.0;
    let hhi: f64 = rows
        .iter()
        .map(|r| (r.current_weight_pct / 100.0).powi(2))
        .sum();
    let effective_units = if hhi > 0.0 { 1.0 / hhi } else { 0.0 };
    let n_economies = economies.len();

    let max_economy = economies
        .iter()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(economy, value)| (*economy, *value))
        .unwrap_or((Economy::Global, 0.0));
    let max_economy_pct = max_economy.1 / total * 100.0;
    let max_asset = rows
        .iter()
        .max_by(|a, b| {
            a.current_weight_pct
                .partial_cmp(&b.current_weight_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|row| {
            (
                row.current_weight_pct,
                if row.ticker.is_empty() {
                    row.name.clone()
                } else {
                    row.ticker.clone()
                },
            )
        })
        .unwrap_or((0.0, String::new()));
    let max_cluster = clusters
        .iter()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|((economy, sector), value)| (*value / total * 100.0, *economy, *sector))
        .unwrap_or((0.0, Economy::Global, Sector::Other));

    let composition = Composition {
        classes: vec![
            Slice {
                id: AssetClass::FixedIncome.id().to_string(),
                label: AssetClass::FixedIncome.label().to_string(),
                value: rf_value,
                pct: rf_pct,
            },
            Slice {
                id: AssetClass::Equity.id().to_string(),
                label: AssetClass::Equity.label().to_string(),
                value: rv_value,
                pct: rv_pct,
            },
        ],
        economies: slices_from(economies, total, |e| e.id(), |e| e.label()),
        sectors: slices_from(sectors, total, |s| s.id(), |s| s.label()),
        natures: slices_from(natures.clone(), rv_value, |n| n.id(), |n| n.label()),
    };

    let targets = TargetSummary::compute(settings, assets);
    let target_weight_sum = targets.total_target_sum;
    let mut checks: Vec<Check> = Vec::new();

    // 1. Ancla y motor.
    let deviation = rv_pct - settings.target_equity_pct;
    let points = if deviation.abs() <= band {
        1.0
    } else if deviation.abs() <= band * 2.0 {
        0.5
    } else {
        0.0
    };
    let outside = (deviation.abs() - band).max(0.0);
    let band_note = if outside > 0.0 {
        format!("{} fuera", band_text(outside))
    } else {
        "dentro de la banda".to_string()
    };
    let mut ancla = check(
        "Ancla y motor",
        format!(
            "Renta fija {} (ancla) · renta variable {} (motor). Objetivo {} / {} · banda ±{}: desvío {} ({}).",
            pct(rf_pct),
            pct(rv_pct),
            pct(settings.target_fixed_income_pct),
            pct(settings.target_equity_pct),
            band_text(band),
            pp(deviation),
            band_note
        ),
        12.0,
        points,
    );
    if ancla.points > 0.499 && ancla.points < 0.999 {
        ancla.state_label = "FUERA DE BANDA".to_string();
    }
    checks.push(ancla);

    // 2. Cantidad de activos de renta variable. La renta fija, la
    // liquidez y las mixtas no entran en el 8-13.
    let n = rows.len();
    let n_equity = targets.n_equity;
    let (points, detail) = if n_equity < settings.min_assets as usize {
        (
            0.0,
            format!(
                "{n_equity} activos de renta variable: pocos. El piso del perfil es {}.",
                settings.min_assets
            ),
        )
    } else if n_equity > settings.max_assets as usize {
        (
            0.5,
            format!(
                "{n_equity} activos de renta variable: más de {}. Arriba de ese número sumás cantidad, no diversificación.",
                settings.max_assets
            ),
        )
    } else {
        (
            1.0,
            format!(
                "{n_equity} activos de renta variable: dentro del rango de {} a {} del perfil.",
                settings.min_assets, settings.max_assets
            ),
        )
    };
    let mut cantidad = check("Cantidad de activos de renta variable", detail, 8.0, points);
    cantidad.note = equity_repartition_note(settings, &targets);
    checks.push(cantidad);

    // 3. Economías.
    let (points, detail) = if n_economies <= 1 {
        (
            0.0,
            format!(
                "Todo en {}: una sola economía manda sobre el 100 % del portafolio.",
                max_economy.0.label()
            ),
        )
    } else if n_economies <= 3 {
        (
            1.0,
            format!(
                "{n_economies} economías: Estados Unidos como base más el resto. Es lo que pide la estrategia."
            ),
        )
    } else {
        (
            0.5,
            format!(
                "{n_economies} economías: demasiadas para seguirlas. La estrategia pide 2 o 3 bien elegidas."
            ),
        )
    };
    checks.push(check("Economías", detail, 10.0, points));

    // 4. Dependencia de una economía.
    let points = if max_economy_pct <= settings.max_economy_pct {
        1.0
    } else if max_economy_pct <= settings.max_economy_pct + 15.0 {
        0.5
    } else {
        0.0
    };
    checks.push(check(
        "Dependencia de una economía",
        format!(
            "{} pesa {}. El límite del perfil es {}.",
            max_economy.0.label(),
            pct(max_economy_pct),
            pct(settings.max_economy_pct)
        ),
        8.0,
        points,
    ));

    // 5. Diversificación efectiva.
    let ratio = effective_units / n as f64;
    let points = if ratio >= 0.7 {
        1.0
    } else if ratio >= 0.5 {
        0.5
    } else {
        0.0
    };
    checks.push(check(
        "Diversificación real",
        format!(
            "{n} activos, pero por peso equivalen a {:.1}. Si hay mucha diferencia, estás sobrediversificado y concentrado a la vez.",
            effective_units
        )
        .replace('.', ","),
        10.0,
        points,
    ));

    // 6. Peso máximo por activo.
    let points = if max_asset.0 <= settings.max_asset_pct {
        1.0
    } else if max_asset.0 <= settings.max_asset_pct * 1.5 {
        0.5
    } else {
        0.0
    };
    checks.push(check(
        "Peso máximo por activo",
        format!(
            "El más grande es {} con {}. El límite es {}.",
            max_asset.1,
            pct(max_asset.0),
            pct(settings.max_asset_pct)
        ),
        10.0,
        points,
    ));

    // 7. Bloques economía + sector.
    let points = if max_cluster.0 <= settings.max_cluster_pct {
        1.0
    } else if max_cluster.0 <= settings.max_cluster_pct + 15.0 {
        0.5
    } else {
        0.0
    };
    checks.push(check(
        "Activos que se mueven juntos",
        format!(
            "El bloque más pesado es {} · {} con {}. Límite {}.",
            max_cluster.1.label(),
            max_cluster.2.label(),
            pct(max_cluster.0),
            pct(settings.max_cluster_pct)
        ),
        10.0,
        points,
    ));

    // 8. Cíclico vs defensivo (solo renta variable).
    let cyclical = natures.get(&Nature::Cyclical).copied().unwrap_or(0.0);
    let defensive = natures.get(&Nature::Defensive).copied().unwrap_or(0.0);
    let (points, detail) = if rv_value <= 0.0 {
        (
            1.0,
            "Sin renta variable: no hay riesgo cíclico.".to_string(),
        )
    } else {
        let cyclical_pct = cyclical / rv_value * 100.0;
        let points = if cyclical_pct <= 70.0 {
            1.0
        } else if cyclical_pct <= 85.0 {
            0.5
        } else {
            0.0
        };
        (
            points,
            format!(
                "En renta variable: {} cíclico y {} defensivo. Arriba de 70 % cíclico el portafolio depende del ciclo.",
                pct(cyclical_pct),
                pct(defensive / rv_value * 100.0)
            ),
        )
    };
    checks.push(check("Cíclico vs defensivo", detail, 5.0, points));

    // 9. Disparadores de venta. La liquidez es contraparte, no un activo
    // que necesite plan de salida.
    let triggerable = rows.iter().filter(|row| !row.is_cash).count();
    let (points, detail) = if triggerable == 0 {
        (
            1.0,
            "Solo liquidez: no hay activos que necesiten disparadores.".to_string(),
        )
    } else if without_triggers.is_empty() {
        (
            1.0,
            format!("Los {triggerable} activos entraron con sus disparadores escritos."),
        )
    } else if (triggerable - without_triggers.len()) * 2 >= triggerable {
        (
            0.5,
            format!(
                "Faltan disparadores en {}: {}.",
                without_triggers.len(),
                without_triggers.join(", ")
            ),
        )
    } else {
        (
            0.0,
            format!(
                "{} de {triggerable} activos no tienen disparadores de venta escritos.",
                without_triggers.len()
            ),
        )
    };
    checks.push(check("Disparadores de venta", detail, 15.0, points));

    // 10. Tesis y objetivo.
    let (points, detail) = if triggerable == 0 {
        (
            1.0,
            "Solo liquidez: no hay tesis de activos que revisar.".to_string(),
        )
    } else if without_thesis.is_empty() {
        (
            1.0,
            format!("Los {triggerable} activos tienen tesis y objetivo escritos."),
        )
    } else {
        (
            0.0,
            format!(
                "Sin tesis u objetivo escrito: {}.",
                without_thesis.join(", ")
            ),
        )
    };
    checks.push(check("Tesis y objetivo", detail, 5.0, points));

    // 11. Integridad de los pesos objetivo.
    let points = if (target_weight_sum - 100.0).abs() <= 1.0 {
        1.0
    } else {
        0.0
    };
    let missing = 100.0 - target_weight_sum;
    let detail = if missing.abs() <= 0.05 {
        format!("Suman {}.", pct(target_weight_sum))
    } else if missing > 0.0 {
        format!(
            "Suman {} · faltan {} para llegar a 100 %. Ajustá el peso objetivo de uno o más activos.",
            pct(target_weight_sum),
            pct(missing)
        )
    } else {
        format!(
            "Suman {} · sobran {} por encima de 100 %. Ajustá el peso objetivo de uno o más activos.",
            pct(target_weight_sum),
            pct(-missing)
        )
    };
    checks.push(check("Los pesos objetivo suman 100 %", detail, 5.0, points));

    let weight_total: f64 = checks.iter().map(|c| c.weight).sum();
    let earned: f64 = checks.iter().map(|c| c.weight * c.points).sum();
    let score = if weight_total > 0.0 {
        (earned / weight_total * 100.0).round() as u8
    } else {
        0
    };
    let (status, status_detail) = match score {
        85..=100 => (
            "Sólido",
            "El portafolio cumple la estrategia. Ahora es cuestión de medir y no tocar de más.",
        ),
        70..=84 => (
            "Aceptable",
            "Está bien armado, pero hay puntos flojos que conviene cerrar antes de sumar plata.",
        ),
        50..=69 => (
            "A mejorar",
            "Hay desvíos importantes: sobrediversificación, concentración o activos sin plan de salida.",
        ),
        _ => (
            "Frágil",
            "El portafolio no sigue la estrategia. Conviene rearmarlo de abajo hacia arriba.",
        ),
    };

    // Órdenes de rebalanceo: solo las que se salen de la banda.
    let mut orders: Vec<RebalanceOrder> = rows
        .iter()
        .filter(|row| !row.is_cash && row.deviation_pp.abs() > band)
        .map(|row| {
            let target_value = total * row.target_weight_pct / 100.0;
            let delta = row.value_display - target_value;
            RebalanceOrder {
                asset_id: row.id,
                action: if delta > 0.0 {
                    "Vender".to_string()
                } else {
                    "Comprar".to_string()
                },
                ticker: row.ticker.clone(),
                name: row.name.clone(),
                deviation_pp: row.deviation_pp,
                amount: delta.abs(),
                detail: format!(
                    "Actual {} · objetivo {} · banda ±{}",
                    pct(row.current_weight_pct),
                    pct(row.target_weight_pct),
                    band_text(band)
                ),
            }
        })
        .collect();
    orders.sort_by(|a, b| {
        b.deviation_pp
            .abs()
            .partial_cmp(&a.deviation_pp.abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(Analysis {
        display_currency: display_currency.to_string(),
        rate_used: rate,
        total_display: total,
        total_ars: match (display_currency, rate) {
            ("ARS", _) => Some(total),
            ("USD", Some(r)) => Some(total * r),
            _ => None,
        },
        total_usd: match (display_currency, rate) {
            ("USD", _) => Some(total),
            ("ARS", Some(r)) => Some(total / r),
            _ => None,
        },
        n_assets: n,
        n_equity,
        rf_pct,
        rv_pct,
        mixed_pct,
        rf_target_pct: settings.target_fixed_income_pct,
        rv_target_pct: settings.target_equity_pct,
        rf_deviation_pp: rf_pct - settings.target_fixed_income_pct,
        hhi,
        effective_units,
        n_economies,
        max_economy_pct,
        max_economy_label: max_economy.0.label().to_string(),
        max_asset_pct: max_asset.0,
        max_asset_label: max_asset.1,
        max_cluster_pct: max_cluster.0,
        max_cluster_label: format!("{} · {}", max_cluster.1.label(), max_cluster.2.label()),
        target_weight_sum,
        targets,
        score,
        status: status.to_string(),
        status_detail: status_detail.to_string(),
        composition,
        checks,
        orders,
        rows,
        pending_triggers,
        drawdown: drawdown_table(),
    })
}

fn drawdown_table() -> Vec<Drawdown> {
    [10.0, 20.0, 30.0, 40.0, 50.0]
        .iter()
        .map(|drop| Drawdown {
            drop_pct: *drop,
            recovery_pct: (1.0 / (1.0 - drop / 100.0) - 1.0) * 100.0,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::analyze;
    use crate::portfolio::model::{
        Asset, AssetClass, Economy, RiskProfile, Sector, SellTrigger, Settings, TriggerKind,
        Valuation,
    };
    use uuid::Uuid;

    fn settings() -> Settings {
        Settings::from_profile(RiskProfile::Aggressive)
    }

    fn asset(
        name: &str,
        class: AssetClass,
        fi_pct: f64,
        economy: Economy,
        sector: Sector,
        value: f64,
        currency: &str,
        target: f64,
    ) -> Asset {
        Asset {
            id: Uuid::new_v4(),
            ticker: Some(name.to_string()),
            name: name.to_string(),
            asset_class: class,
            fixed_income_pct: fi_pct,
            economy,
            sector,
            market_value: value,
            currency: currency.to_string(),
            target_weight_pct: target,
            thesis: "tesis".into(),
            objective: "objetivo".into(),
            source: "manual".into(),
            is_cash: false,
            market_symbol: None,
            quantity: None,
            last_price: None,
            price_currency: None,
            priced_at: None,
            triggers: vec![SellTrigger {
                id: Uuid::new_v4(),
                kind: TriggerKind::ThesisBroken,
                description: "si cambia la tesis".into(),
                is_met: false,
                is_resolved: false,
            }],
        }
    }

    #[test]
    fn cartera_vacia_no_puntua() {
        let analysis = analyze(&settings(), &[], Valuation::UsdCcl, Some(1_800.0)).unwrap();
        assert_eq!(analysis.score, 0);
        assert_eq!(analysis.status, "Sin datos");
        assert!(analysis.rows.is_empty());
        assert_eq!(analysis.checks.len(), 11);
    }

    #[test]
    fn rf_y_rv_en_objetivo() {
        let mut a = asset(
            "Bono",
            AssetClass::FixedIncome,
            100.0,
            Economy::Argentina,
            Sector::Other,
            40.0,
            "USD",
            40.0,
        );
        let mut b = asset(
            "AAPL",
            AssetClass::Equity,
            0.0,
            Economy::Us,
            Sector::Technology,
            60.0,
            "USD",
            60.0,
        );
        a.triggers.clear();
        b.triggers.clear();
        let analysis = analyze(&settings(), &[a, b], Valuation::UsdCcl, None).unwrap();
        assert!((analysis.rf_pct - 40.0).abs() < 0.01);
        assert!((analysis.rv_pct - 60.0).abs() < 0.01);
        assert_eq!(analysis.display_currency, "USD");
        assert!(analysis.score > 0);
        // Sin tasa no hace falta: todo está en USD.
        assert!(analysis.total_ars.is_none());
    }

    #[test]
    fn mixta_prorratea() {
        let mixed = asset(
            "FCI",
            AssetClass::Mixed,
            60.0,
            Economy::Global,
            Sector::Other,
            100.0,
            "USD",
            100.0,
        );
        let analysis = analyze(&settings(), &[mixed], Valuation::UsdCcl, None).unwrap();
        assert!((analysis.rf_pct - 60.0).abs() < 0.01);
        assert!((analysis.rv_pct - 40.0).abs() < 0.01);
        assert!((analysis.mixed_pct - 100.0).abs() < 0.01);
    }

    #[test]
    fn rebalanceo_solo_fuera_de_banda() {
        let mut a = asset(
            "AAPL",
            AssetClass::Equity,
            0.0,
            Economy::Us,
            Sector::Technology,
            100.0,
            "USD",
            50.0,
        );
        a.triggers.clear();
        let analysis = analyze(&settings(), &[a], Valuation::UsdCcl, None).unwrap();
        assert_eq!(analysis.orders.len(), 1);
        assert_eq!(analysis.orders[0].action, "Vender");
        assert!((analysis.orders[0].amount - 50.0).abs() < 0.01);
    }

    #[test]
    fn pesos_objetivo_que_no_suman() {
        let mut a = asset(
            "AAPL",
            AssetClass::Equity,
            0.0,
            Economy::Us,
            Sector::Technology,
            100.0,
            "USD",
            20.0,
        );
        a.triggers.clear();
        let analysis = analyze(&settings(), &[a], Valuation::UsdCcl, None).unwrap();
        let check = analysis
            .checks
            .iter()
            .find(|c| c.title.starts_with("Los pesos objetivo"))
            .unwrap();
        assert_eq!(check.points, 0.0);
        assert_eq!(check.state, "alert");
        assert!(check.detail.contains("faltan"), "{}", check.detail);
        assert!(check.detail.contains("80,0"), "{}", check.detail);

        let mut over = asset(
            "AAPL",
            AssetClass::Equity,
            0.0,
            Economy::Us,
            Sector::Technology,
            100.0,
            "USD",
            120.0,
        );
        over.triggers.clear();
        let analysis = analyze(&settings(), &[over], Valuation::UsdCcl, None).unwrap();
        let check = analysis
            .checks
            .iter()
            .find(|c| c.title.starts_with("Los pesos objetivo"))
            .unwrap();
        assert!(check.detail.contains("sobran"), "{}", check.detail);
        assert!(check.detail.contains("20,0"), "{}", check.detail);
    }

    #[test]
    fn cantidad_cuenta_solo_renta_variable() {
        let mut assets = Vec::new();
        for name in ["A", "B", "C", "D", "E", "F", "G", "H"] {
            assets.push(asset(
                name,
                AssetClass::Equity,
                0.0,
                Economy::Us,
                Sector::Technology,
                10.0,
                "USD",
                10.0,
            ));
        }
        assets.push(asset(
            "BONO",
            AssetClass::FixedIncome,
            100.0,
            Economy::Argentina,
            Sector::Other,
            10.0,
            "USD",
            10.0,
        ));
        assets.push(asset(
            "FCI",
            AssetClass::Mixed,
            60.0,
            Economy::Global,
            Sector::Other,
            10.0,
            "USD",
            10.0,
        ));
        for asset in &mut assets {
            asset.triggers.clear();
        }
        let analysis = analyze(&settings(), &assets, Valuation::UsdCcl, None).unwrap();
        assert_eq!(analysis.n_assets, 10);
        assert_eq!(analysis.n_equity, 8);
        assert!((analysis.targets.rv_target_sum - 84.0).abs() < 0.01);
        assert!((analysis.targets.rf_target_sum - 16.0).abs() < 0.01);
        assert!((analysis.targets.equal_equity_pct.unwrap() - 7.0).abs() < 0.01);
        let check = analysis
            .checks
            .iter()
            .find(|c| c.title == "Cantidad de activos de renta variable")
            .expect("cantidad RV");
        assert_eq!(check.points, 1.0);
        assert!(check.detail.contains("8 activos"), "{}", check.detail);
        let note = check.note.as_deref().expect("nota de reparto");
        assert!(note.contains("7,5"), "{note}");
        assert!(note.contains("7,0"), "{note}");
        assert!(note.contains("mixta fija"), "{note}");
    }

    #[test]
    fn reparto_igual_sin_mixta_usa_la_rv_objetivo() {
        let mut assets = Vec::new();
        for name in ["A", "B", "C", "D", "E", "F", "G", "H"] {
            assets.push(asset(
                name,
                AssetClass::Equity,
                0.0,
                Economy::Us,
                Sector::Technology,
                10.0,
                "USD",
                5.0,
            ));
        }
        for item in &mut assets {
            item.triggers.clear();
        }
        let analysis = analyze(&settings(), &assets, Valuation::UsdCcl, None).unwrap();
        let note = analysis
            .checks
            .iter()
            .find(|check| check.title == "Cantidad de activos de renta variable")
            .and_then(|check| check.note.as_deref())
            .expect("nota de reparto");
        assert!(note.contains("7,5"), "{note}");
        assert!(!note.contains("mixta fija"), "{note}");
    }

    #[test]
    fn reparto_igual_con_trece_rv_no_usa_el_total() {
        let mut assets = Vec::new();
        for index in 0..13 {
            assets.push(asset(
                &format!("A{index}"),
                AssetClass::Equity,
                0.0,
                Economy::Us,
                Sector::Technology,
                10.0,
                "USD",
                4.0,
            ));
        }
        for item in &mut assets {
            item.triggers.clear();
        }
        let analysis = analyze(&settings(), &assets, Valuation::UsdCcl, None).unwrap();
        let note = analysis
            .checks
            .iter()
            .find(|check| check.title == "Cantidad de activos de renta variable")
            .and_then(|check| check.note.as_deref())
            .expect("nota de reparto");
        assert!(note.contains("4,6"), "{note}");
        assert!(!note.contains("7,6"), "{note}");
    }

    #[test]
    fn ancla_fuera_de_banda_explica_el_desvio() {
        let mut rf = asset(
            "Bono",
            AssetClass::FixedIncome,
            100.0,
            Economy::Argentina,
            Sector::Other,
            45.4,
            "USD",
            40.0,
        );
        let mut rv = asset(
            "AAPL",
            AssetClass::Equity,
            0.0,
            Economy::Us,
            Sector::Technology,
            54.6,
            "USD",
            60.0,
        );
        rf.triggers.clear();
        rv.triggers.clear();
        let analysis = analyze(&settings(), &[rf, rv], Valuation::UsdCcl, None).unwrap();
        let check = analysis
            .checks
            .iter()
            .find(|check| check.title == "Ancla y motor")
            .expect("ancla");
        assert_eq!(check.state_label, "FUERA DE BANDA");
        assert!(check.detail.contains("banda"), "{}", check.detail);
        assert!(check.detail.contains("0,4"), "{}", check.detail);
    }

    #[test]
    fn pendientes_excluyen_ejecutados() {
        let mut a = asset(
            "AAPL",
            AssetClass::Equity,
            0.0,
            Economy::Us,
            Sector::Technology,
            100.0,
            "USD",
            100.0,
        );
        a.triggers[0].is_met = true;
        let analysis = analyze(&settings(), &[a.clone()], Valuation::UsdCcl, None).unwrap();
        assert_eq!(analysis.pending_triggers.len(), 1);
        assert_eq!(analysis.rows[0].n_pending, 1);

        a.triggers[0].is_resolved = true;
        let analysis = analyze(&settings(), &[a], Valuation::UsdCcl, None).unwrap();
        assert!(analysis.pending_triggers.is_empty());
        assert_eq!(analysis.rows[0].n_pending, 0);
        assert_eq!(analysis.rows[0].n_met, 1);
    }

    #[test]
    fn falta_tasa_solo_si_hay_otra_moneda() {
        let usd = asset(
            "AAPL",
            AssetClass::Equity,
            0.0,
            Economy::Us,
            Sector::Technology,
            100.0,
            "USD",
            100.0,
        );
        assert!(analyze(&settings(), &[usd.clone()], Valuation::UsdCcl, None).is_ok());

        let ars = asset(
            "AL30",
            AssetClass::FixedIncome,
            100.0,
            Economy::Argentina,
            Sector::Other,
            100.0,
            "ARS",
            100.0,
        );
        assert!(analyze(&settings(), &[ars], Valuation::UsdCcl, None).is_err());
        assert!(
            analyze(
                &settings(),
                &[asset(
                    "AL30",
                    AssetClass::FixedIncome,
                    100.0,
                    Economy::Argentina,
                    Sector::Other,
                    100.0,
                    "ARS",
                    100.0
                )],
                Valuation::UsdCcl,
                Some(1_800.0)
            )
            .is_ok()
        );
    }
}
