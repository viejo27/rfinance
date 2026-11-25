use std::collections::HashMap;

use actix_session::Session;
use actix_web::{HttpResponse, Responder, delete, get, patch, post, put, web};
use serde::{Deserialize, Serialize};
use sqlx::{Connection, FromRow, PgConnection, PgPool};
use uuid::Uuid;

use crate::date;
use crate::market::fx::{FxCache, Rates};
use crate::portfolio::metrics::{Analysis, analyze};
use crate::portfolio::model::{
    Asset, AssetClass, Economy, OptionItem, ProfilePreset, RateKind, RiskProfile, Sector,
    SellTrigger, Settings, TriggerKind, Valuation, profile_presets,
};
use crate::portfolio::targets::{TargetMode, TargetPlan, TargetSleeve, plan_targets};
use crate::routes::api::MAX_FINANCIAL_VALUE;
use crate::routes::api::snapshot::session_user_id;

const MAX_NAME_LENGTH: usize = 120;
const MAX_TICKER_LENGTH: usize = 20;
const MAX_TEXT_LENGTH: usize = 2_000;
const MAX_TRIGGER_LENGTH: usize = 300;

// --------------------------------------------------------------- filas ----

#[derive(FromRow)]
struct SettingsRow {
    public_id: Uuid,
    risk_profile: String,
    valuation: String,
    target_fixed_income_pct: f64,
    target_equity_pct: f64,
    rebalance_band_pp: f64,
    min_assets: i16,
    max_assets: i16,
    max_asset_pct: f64,
    max_economy_pct: f64,
    max_cluster_pct: f64,
}

#[derive(FromRow)]
struct AssetRow {
    id: i32,
    public_id: Uuid,
    ticker: Option<String>,
    name: String,
    asset_class: String,
    fixed_income_pct: f64,
    economy: String,
    sector: String,
    market_value: f64,
    currency: String,
    target_weight_pct: f64,
    thesis: String,
    objective: String,
    source: String,
    is_cash: bool,
    market_symbol: Option<String>,
    quantity: Option<f64>,
    last_price: Option<f64>,
    price_currency: Option<String>,
    priced_at: Option<String>,
}

#[derive(FromRow)]
struct TriggerRow {
    asset_id: i32,
    public_id: Uuid,
    kind: String,
    description: String,
    is_met: bool,
    is_resolved: bool,
}

fn settings_from_row(row: SettingsRow) -> Result<Settings, String> {
    let risk_profile =
        RiskProfile::from_id(&row.risk_profile).ok_or("Perfil de riesgo inválido")?;
    let valuation = Valuation::from_id(&row.valuation).ok_or("Vista de valuación inválida")?;
    Ok(Settings {
        public_id: row.public_id,
        risk_profile,
        valuation,
        target_fixed_income_pct: row.target_fixed_income_pct,
        target_equity_pct: row.target_equity_pct,
        rebalance_band_pp: row.rebalance_band_pp,
        min_assets: row.min_assets,
        max_assets: row.max_assets,
        max_asset_pct: row.max_asset_pct,
        max_economy_pct: row.max_economy_pct,
        max_cluster_pct: row.max_cluster_pct,
    })
}

fn asset_from_row(row: AssetRow, triggers: Vec<SellTrigger>) -> Result<Asset, String> {
    let asset_class =
        AssetClass::from_id(&row.asset_class).ok_or("Clase de activo inválida en la base")?;
    let economy = Economy::from_id(&row.economy).ok_or("Economía inválida en la base")?;
    let sector = Sector::from_id(&row.sector).ok_or("Sector inválido en la base")?;
    Ok(Asset {
        id: row.public_id,
        ticker: row.ticker,
        name: row.name,
        asset_class,
        fixed_income_pct: row.fixed_income_pct,
        economy,
        sector,
        market_value: row.market_value,
        currency: row.currency,
        target_weight_pct: row.target_weight_pct,
        thesis: row.thesis,
        objective: row.objective,
        source: row.source,
        is_cash: row.is_cash,
        market_symbol: row.market_symbol,
        quantity: row.quantity,
        last_price: row.last_price,
        price_currency: row.price_currency,
        priced_at: row.priced_at,
        triggers,
    })
}

// ------------------------------------------------------------ settings ----

const SETTINGS_SELECT: &str = "SELECT public_id, risk_profile, valuation,
    target_fixed_income_pct::float8 AS target_fixed_income_pct,
    target_equity_pct::float8 AS target_equity_pct,
    rebalance_band_pp::float8 AS rebalance_band_pp,
    min_assets, max_assets,
    max_asset_pct::float8 AS max_asset_pct,
    max_economy_pct::float8 AS max_economy_pct,
    max_cluster_pct::float8 AS max_cluster_pct
    FROM portfolio_settings WHERE user_id = $1";

const SETTINGS_INSERT: &str = "INSERT INTO portfolio_settings (
        user_id, risk_profile, valuation, target_fixed_income_pct, target_equity_pct,
        rebalance_band_pp, min_assets, max_assets, max_asset_pct, max_economy_pct,
        max_cluster_pct
    ) VALUES (
        $1, $2, $3, $4::numeric, $5::numeric, $6::numeric, $7, $8, $9::numeric,
        $10::numeric, $11::numeric
    )
    ON CONFLICT (user_id) DO UPDATE SET user_id = portfolio_settings.user_id
    RETURNING public_id, risk_profile, valuation,
        target_fixed_income_pct::float8 AS target_fixed_income_pct,
        target_equity_pct::float8 AS target_equity_pct,
        rebalance_band_pp::float8 AS rebalance_band_pp,
        min_assets, max_assets,
        max_asset_pct::float8 AS max_asset_pct,
        max_economy_pct::float8 AS max_economy_pct,
        max_cluster_pct::float8 AS max_cluster_pct";

/// Crea la fila de criterios si no existe (perfil agresivo por defecto).
pub async fn ensure_settings(conn: &mut PgConnection, user_id: i32) -> Result<Settings, String> {
    let existing: Option<SettingsRow> = sqlx::query_as(SETTINGS_SELECT)
        .bind(user_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|_| "Error al leer los criterios de la cartera".to_string())?;
    if let Some(row) = existing {
        return settings_from_row(row);
    }

    let defaults = Settings::from_profile(RiskProfile::Aggressive);
    let row: SettingsRow = sqlx::query_as(SETTINGS_INSERT)
        .bind(user_id)
        .bind(defaults.risk_profile.id())
        .bind(defaults.valuation.id())
        .bind(defaults.target_fixed_income_pct)
        .bind(defaults.target_equity_pct)
        .bind(defaults.rebalance_band_pp)
        .bind(defaults.min_assets)
        .bind(defaults.max_assets)
        .bind(defaults.max_asset_pct)
        .bind(defaults.max_economy_pct)
        .bind(defaults.max_cluster_pct)
        .fetch_one(&mut *conn)
        .await
        .map_err(|_| "Error al crear los criterios de la cartera".to_string())?;
    settings_from_row(row)
}

// -------------------------------------------------------------- assets ----

const ASSET_SELECT: &str = "SELECT id, public_id, ticker, name, asset_class,
    fixed_income_pct::float8 AS fixed_income_pct, economy, sector,
    market_value::float8 AS market_value, currency,
    target_weight_pct::float8 AS target_weight_pct, thesis, objective,
    source, is_cash, market_symbol, quantity::float8 AS quantity,
    last_price::float8 AS last_price, price_currency, priced_at::text AS priced_at
    FROM assets WHERE user_id = $1 AND is_active
    ORDER BY sort_order, created_at";

pub async fn load_assets(conn: &mut PgConnection, user_id: i32) -> Result<Vec<Asset>, String> {
    let rows: Vec<AssetRow> = sqlx::query_as(ASSET_SELECT)
        .bind(user_id)
        .fetch_all(&mut *conn)
        .await
        .map_err(|_| "Error al leer los activos".to_string())?;
    let trigger_rows: Vec<TriggerRow> = sqlx::query_as(
        "SELECT asset_id, public_id, kind, description, is_met, is_resolved
         FROM sell_triggers WHERE user_id = $1 ORDER BY created_at",
    )
    .bind(user_id)
    .fetch_all(&mut *conn)
    .await
    .map_err(|_| "Error al leer los disparadores".to_string())?;

    let mut by_asset: HashMap<i32, Vec<SellTrigger>> = HashMap::new();
    for row in trigger_rows {
        let kind = TriggerKind::from_id(&row.kind).ok_or("Disparador inválido en la base")?;
        by_asset.entry(row.asset_id).or_default().push(SellTrigger {
            id: row.public_id,
            kind,
            description: row.description,
            is_met: row.is_met,
            is_resolved: row.is_resolved,
        });
    }

    rows.into_iter()
        .map(|row| {
            let triggers = by_asset.remove(&row.id).unwrap_or_default();
            asset_from_row(row, triggers)
        })
        .collect()
}

// ----------------------------------------------------------- entrada ----

#[derive(Deserialize)]
pub struct AssetInput {
    pub ticker: Option<String>,
    pub name: String,
    pub asset_class: String,
    pub fixed_income_pct: Option<f64>,
    pub economy: String,
    pub sector: String,
    pub market_value: f64,
    pub currency: String,
    pub target_weight_pct: f64,
    pub thesis: Option<String>,
    pub objective: Option<String>,
    pub is_cash: Option<bool>,
}

struct ValidAsset {
    ticker: Option<String>,
    name: String,
    asset_class: AssetClass,
    fixed_income_pct: f64,
    economy: Economy,
    sector: Sector,
    market_value: f64,
    currency: String,
    target_weight_pct: f64,
    thesis: String,
    objective: String,
    is_cash: bool,
}

fn validate_asset(input: &AssetInput) -> Result<ValidAsset, String> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err("El nombre del activo es obligatorio".into());
    }
    if name.chars().count() > MAX_NAME_LENGTH {
        return Err(format!(
            "El nombre del activo no puede superar los {MAX_NAME_LENGTH} caracteres"
        ));
    }
    let ticker = input
        .ticker
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_uppercase());
    if let Some(ticker) = &ticker
        && ticker.chars().count() > MAX_TICKER_LENGTH
    {
        return Err(format!(
            "El ticker no puede superar los {MAX_TICKER_LENGTH} caracteres"
        ));
    }
    let asset_class = AssetClass::from_id(&input.asset_class)
        .ok_or_else(|| "La clase debe ser renta fija, renta variable o mixta".to_string())?;
    let fixed_income_pct = match asset_class {
        AssetClass::FixedIncome => 100.0,
        AssetClass::Equity => 0.0,
        AssetClass::Mixed => {
            let value = input
                .fixed_income_pct
                .unwrap_or_else(|| asset_class.default_fixed_income_pct());
            if !value.is_finite() || !(1.0..=99.0).contains(&value) {
                return Err("La parte de renta fija de una mixta debe estar entre 1 y 99 %".into());
            }
            value
        }
    };
    let economy = Economy::from_id(&input.economy)
        .ok_or_else(|| "La economía elegida no es válida".to_string())?;
    let sector = Sector::from_id(&input.sector)
        .ok_or_else(|| "El sector elegido no es válido".to_string())?;
    let is_cash = input.is_cash.unwrap_or(false);
    if is_cash && asset_class != AssetClass::FixedIncome {
        return Err("La liquidez debe ser un activo de renta fija".into());
    }
    if !input.market_value.is_finite()
        || input.market_value < 0.0
        || input.market_value > MAX_FINANCIAL_VALUE
    {
        return Err("El valor del activo no es un número válido".into());
    }
    if input.currency != "ARS" && input.currency != "USD" {
        return Err("La moneda debe ser ARS o USD".into());
    }
    if !input.target_weight_pct.is_finite() || !(0.0..=100.0).contains(&input.target_weight_pct) {
        return Err("El peso objetivo debe estar entre 0 y 100 %".into());
    }
    let thesis = input.thesis.as_deref().unwrap_or("").trim().to_string();
    let objective = input.objective.as_deref().unwrap_or("").trim().to_string();
    if thesis.chars().count() > MAX_TEXT_LENGTH || objective.chars().count() > MAX_TEXT_LENGTH {
        return Err(format!(
            "La tesis y el objetivo no pueden superar los {MAX_TEXT_LENGTH} caracteres"
        ));
    }
    Ok(ValidAsset {
        ticker,
        name,
        asset_class,
        fixed_income_pct,
        economy,
        sector,
        market_value: input.market_value,
        currency: input.currency.clone(),
        target_weight_pct: input.target_weight_pct,
        thesis,
        objective,
        is_cash,
    })
}

pub async fn create_asset(
    conn: &mut PgConnection,
    user_id: i32,
    input: &AssetInput,
) -> Result<Uuid, String> {
    let valid = validate_asset(input)?;
    if valid.is_cash {
        let existing: Option<(i32,)> =
            sqlx::query_as("SELECT id FROM assets WHERE user_id = $1 AND is_cash")
                .bind(user_id)
                .fetch_optional(&mut *conn)
                .await
                .map_err(|_| "Error al verificar la liquidez".to_string())?;
        if existing.is_some() {
            return Err("Ya tenés un activo marcado como liquidez".into());
        }
    }
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO assets (
            user_id, ticker, name, asset_class, fixed_income_pct, economy, sector,
            market_value, currency, target_weight_pct, thesis, objective, is_cash
         ) VALUES ($1, $2, $3, $4, $5::numeric, $6, $7, $8::numeric, $9, $10::numeric, $11, $12, $13)
         RETURNING public_id",
    )
    .bind(user_id)
    .bind(valid.ticker.as_deref())
    .bind(&valid.name)
    .bind(valid.asset_class.id())
    .bind(valid.fixed_income_pct)
    .bind(valid.economy.id())
    .bind(valid.sector.id())
    .bind(valid.market_value)
    .bind(&valid.currency)
    .bind(valid.target_weight_pct)
    .bind(&valid.thesis)
    .bind(&valid.objective)
    .bind(valid.is_cash)
    .fetch_one(&mut *conn)
    .await
    .map_err(|_| "Error al crear el activo".to_string())?;
    Ok(row.0)
}

pub async fn update_asset(
    conn: &mut PgConnection,
    user_id: i32,
    public_id: Uuid,
    input: &AssetInput,
) -> Result<(), String> {
    let valid = validate_asset(input)?;
    if valid.is_cash {
        let existing: Option<(i32,)> = sqlx::query_as(
            "SELECT id FROM assets WHERE user_id = $1 AND is_cash AND public_id <> $2",
        )
        .bind(user_id)
        .bind(public_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|_| "Error al verificar la liquidez".to_string())?;
        if existing.is_some() {
            return Err("Ya tenés un activo marcado como liquidez".into());
        }
        let triggers: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sell_triggers WHERE asset_id = (
                SELECT id FROM assets WHERE public_id = $1 AND user_id = $2
             )",
        )
        .bind(public_id)
        .bind(user_id)
        .fetch_one(&mut *conn)
        .await
        .map_err(|_| "Error al verificar los disparadores".to_string())?;
        if triggers.0 > 0 {
            return Err(
                "La liquidez no puede tener disparadores; borralos antes de marcarla".into(),
            );
        }
    }
    let result = sqlx::query(
        "UPDATE assets SET
            ticker = $3,
            name = $4,
            asset_class = $5,
            fixed_income_pct = $6::numeric,
            economy = $7,
            sector = $8,
            market_value = $9::numeric,
            currency = $10,
            target_weight_pct = $11::numeric,
            thesis = $12,
            objective = $13,
            is_cash = $14,
            updated_at = CURRENT_TIMESTAMP
         WHERE public_id = $1 AND user_id = $2 AND is_active",
    )
    .bind(public_id)
    .bind(user_id)
    .bind(valid.ticker.as_deref())
    .bind(&valid.name)
    .bind(valid.asset_class.id())
    .bind(valid.fixed_income_pct)
    .bind(valid.economy.id())
    .bind(valid.sector.id())
    .bind(valid.market_value)
    .bind(&valid.currency)
    .bind(valid.target_weight_pct)
    .bind(&valid.thesis)
    .bind(&valid.objective)
    .bind(valid.is_cash)
    .execute(&mut *conn)
    .await
    .map_err(|_| "Error al guardar el activo".to_string())?;
    if result.rows_affected() == 0 {
        return Err("Activo no encontrado".into());
    }
    Ok(())
}

pub async fn delete_asset(
    conn: &mut PgConnection,
    user_id: i32,
    public_id: Uuid,
) -> sqlx::Result<u64> {
    let result = sqlx::query("DELETE FROM assets WHERE public_id = $1 AND user_id = $2")
        .bind(public_id)
        .bind(user_id)
        .execute(&mut *conn)
        .await?;
    Ok(result.rows_affected())
}

// ---------------------------------------------------------- triggers ----

#[derive(Deserialize)]
pub struct TriggerInput {
    pub kind: String,
    pub description: String,
}

#[derive(Deserialize)]
pub struct TriggerPatch {
    pub kind: Option<String>,
    pub description: Option<String>,
    pub is_met: Option<bool>,
    pub is_resolved: Option<bool>,
}

fn valid_trigger_description(value: &str) -> Result<String, String> {
    let description = value.trim().to_string();
    if description.is_empty() {
        return Err("La descripción del disparador es obligatoria".into());
    }
    if description.chars().count() > MAX_TRIGGER_LENGTH {
        return Err(format!(
            "La descripción no puede superar los {MAX_TRIGGER_LENGTH} caracteres"
        ));
    }
    Ok(description)
}

pub async fn create_trigger(
    conn: &mut PgConnection,
    user_id: i32,
    asset_id: Uuid,
    input: &TriggerInput,
) -> Result<Uuid, String> {
    let kind = TriggerKind::from_id(&input.kind)
        .ok_or_else(|| "El tipo de disparador no es válido".to_string())?;
    let description = valid_trigger_description(&input.description)?;
    let row: Option<(Uuid,)> = sqlx::query_as(
        "INSERT INTO sell_triggers (user_id, asset_id, kind, description)
         SELECT $1, id, $3, $4 FROM assets
         WHERE public_id = $2 AND user_id = $1 AND is_active AND NOT is_cash
         RETURNING public_id",
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(kind.id())
    .bind(&description)
    .fetch_optional(&mut *conn)
    .await
    .map_err(|_| "Error al crear el disparador".to_string())?;
    row.map(|(id,)| id)
        .ok_or_else(|| "Activo no encontrado".to_string())
}

pub async fn update_trigger(
    conn: &mut PgConnection,
    user_id: i32,
    public_id: Uuid,
    patch: &TriggerPatch,
) -> Result<(), String> {
    let kind = match patch.kind.as_deref() {
        Some(value) => Some(
            TriggerKind::from_id(value)
                .ok_or_else(|| "El tipo de disparador no es válido".to_string())?,
        ),
        None => None,
    };
    let description = match patch.description.as_deref() {
        Some(value) => Some(valid_trigger_description(value)?),
        None => None,
    };
    // Reabrir un disparador (is_met=false) limpia también la ejecución:
    // si la condición dejó de cumplirse, no queda como resuelto.
    let is_resolved = if patch.is_met == Some(false) {
        Some(false)
    } else {
        patch.is_resolved
    };
    let result = sqlx::query(
        "UPDATE sell_triggers SET
            kind = COALESCE($3::text, kind),
            description = COALESCE($4::text, description),
            is_met = COALESCE($5::boolean, is_met),
            is_resolved = COALESCE($6::boolean, is_resolved)
         WHERE public_id = $1 AND user_id = $2",
    )
    .bind(public_id)
    .bind(user_id)
    .bind(kind.map(|k| k.id()))
    .bind(description.as_deref())
    .bind(patch.is_met)
    .bind(is_resolved)
    .execute(&mut *conn)
    .await
    .map_err(|_| "Error al guardar el disparador".to_string())?;
    if result.rows_affected() == 0 {
        return Err("Disparador no encontrado".into());
    }
    Ok(())
}

pub async fn delete_trigger(
    conn: &mut PgConnection,
    user_id: i32,
    public_id: Uuid,
) -> sqlx::Result<u64> {
    let result = sqlx::query("DELETE FROM sell_triggers WHERE public_id = $1 AND user_id = $2")
        .bind(public_id)
        .bind(user_id)
        .execute(&mut *conn)
        .await?;
    Ok(result.rows_affected())
}

// ---------------------------------------------------------- settings ----

#[derive(Deserialize)]
pub struct SettingsInput {
    pub risk_profile: String,
    pub valuation: String,
    pub target_fixed_income_pct: f64,
    pub target_equity_pct: f64,
    pub rebalance_band_pp: f64,
    pub min_assets: i16,
    pub max_assets: i16,
    pub max_asset_pct: f64,
    pub max_economy_pct: f64,
    pub max_cluster_pct: f64,
}

#[derive(Deserialize)]
pub struct TargetPlanInput {
    pub sleeve: TargetSleeve,
    pub mode: TargetMode,
}

struct ValidSettings {
    risk_profile: RiskProfile,
    valuation: Valuation,
    target_fixed_income_pct: f64,
    target_equity_pct: f64,
    rebalance_band_pp: f64,
    min_assets: i16,
    max_assets: i16,
    max_asset_pct: f64,
    max_economy_pct: f64,
    max_cluster_pct: f64,
}

fn validate_settings(input: &SettingsInput) -> Result<ValidSettings, String> {
    let risk_profile = RiskProfile::from_id(&input.risk_profile)
        .ok_or_else(|| "El perfil de riesgo no es válido".to_string())?;
    let valuation = Valuation::from_id(&input.valuation)
        .ok_or_else(|| "La moneda de valuación no es válida".to_string())?;
    let values = [
        ("renta fija", input.target_fixed_income_pct),
        ("renta variable", input.target_equity_pct),
        ("banda de rebalanceo", input.rebalance_band_pp),
        ("máximo por activo", input.max_asset_pct),
        ("máximo por economía", input.max_economy_pct),
        ("máximo por bloque", input.max_cluster_pct),
    ];
    for (label, value) in values {
        if !value.is_finite() || !(0.0..=100.0).contains(&value) {
            return Err(format!("El {label} debe estar entre 0 y 100 %"));
        }
    }
    if (input.target_fixed_income_pct + input.target_equity_pct - 100.0).abs() > 0.01 {
        return Err("Los objetivos de renta fija y variable deben sumar 100 %".into());
    }
    if input.min_assets < 0 || input.max_assets < input.min_assets {
        return Err("El rango de activos no es válido".into());
    }
    Ok(ValidSettings {
        risk_profile,
        valuation,
        target_fixed_income_pct: input.target_fixed_income_pct,
        target_equity_pct: input.target_equity_pct,
        rebalance_band_pp: input.rebalance_band_pp,
        min_assets: input.min_assets,
        max_assets: input.max_assets,
        max_asset_pct: input.max_asset_pct,
        max_economy_pct: input.max_economy_pct,
        max_cluster_pct: input.max_cluster_pct,
    })
}

pub async fn update_settings(
    conn: &mut PgConnection,
    user_id: i32,
    input: &SettingsInput,
) -> Result<(), String> {
    let valid = validate_settings(input)?;
    ensure_settings(conn, user_id).await?;
    sqlx::query(
        "UPDATE portfolio_settings SET
            risk_profile = $2,
            valuation = $3,
            target_fixed_income_pct = $4::numeric,
            target_equity_pct = $5::numeric,
            rebalance_band_pp = $6::numeric,
            min_assets = $7,
            max_assets = $8,
            max_asset_pct = $9::numeric,
            max_economy_pct = $10::numeric,
            max_cluster_pct = $11::numeric,
            updated_at = CURRENT_TIMESTAMP
         WHERE user_id = $1",
    )
    .bind(user_id)
    .bind(valid.risk_profile.id())
    .bind(valid.valuation.id())
    .bind(valid.target_fixed_income_pct)
    .bind(valid.target_equity_pct)
    .bind(valid.rebalance_band_pp)
    .bind(valid.min_assets)
    .bind(valid.max_assets)
    .bind(valid.max_asset_pct)
    .bind(valid.max_economy_pct)
    .bind(valid.max_cluster_pct)
    .execute(&mut *conn)
    .await
    .map_err(|_| "Error al guardar los criterios".to_string())?;
    Ok(())
}

// -------------------------------------------------------------- estado ----

#[derive(Serialize)]
pub struct Options {
    pub asset_classes: Vec<OptionItem>,
    pub economies: Vec<OptionItem>,
    pub sectors: Vec<OptionItem>,
    pub trigger_kinds: Vec<OptionItem>,
    pub valuations: Vec<OptionItem>,
}

#[derive(Serialize)]
pub struct FxView {
    pub valuation: Valuation,
    pub display_currency: String,
    pub rate_kind: String,
    pub mep: Option<f64>,
    pub ccl: Option<f64>,
    pub fetched_at: Option<i64>,
    pub stale: bool,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct PortfolioState {
    pub settings: Settings,
    pub profiles: Vec<ProfilePreset>,
    pub options: Options,
    pub assets: Vec<Asset>,
    pub analysis: Option<Analysis>,
    pub analysis_error: Option<String>,
    pub fx: FxView,
    pub is_custom: bool,
    pub today: String,
}

pub async fn load_state(
    conn: &mut PgConnection,
    user_id: i32,
    fx_result: Result<Rates, String>,
) -> Result<PortfolioState, String> {
    let settings = ensure_settings(&mut *conn, user_id).await?;
    let assets = load_assets(&mut *conn, user_id).await?;

    let (mep, ccl, fetched_at, stale, error) = match fx_result {
        Ok(rates) => (
            Some(rates.mep),
            Some(rates.ccl),
            Some(rates.fetched_at),
            rates.stale,
            None,
        ),
        Err(error) => (None, None, None, false, Some(error)),
    };
    let fx = FxView {
        valuation: settings.valuation,
        display_currency: settings.valuation.display_currency().to_string(),
        rate_kind: settings.valuation.rate_kind().label().to_string(),
        mep,
        ccl,
        fetched_at,
        stale,
        error,
    };
    let rate = match settings.valuation.rate_kind() {
        RateKind::Mep => mep,
        RateKind::Ccl => ccl,
    };
    let (analysis, analysis_error) = match analyze(&settings, &assets, settings.valuation, rate) {
        Ok(analysis) => (Some(analysis), None),
        Err(error) => (None, Some(error)),
    };

    Ok(PortfolioState {
        is_custom: settings.is_custom(),
        settings,
        profiles: profile_presets(),
        options: Options {
            asset_classes: AssetClass::options(),
            economies: Economy::options(),
            sectors: Sector::options(),
            trigger_kinds: TriggerKind::options(),
            valuations: Valuation::options(),
        },
        assets,
        analysis,
        analysis_error,
        fx,
        today: date::today_iso(),
    })
}

// ------------------------------------------------------------- ejemplo ----

struct DemoAsset {
    ticker: &'static str,
    name: &'static str,
    class: AssetClass,
    fixed_income_pct: f64,
    economy: Economy,
    sector: Sector,
    value: f64,
    target: f64,
    is_cash: bool,
    thesis: &'static str,
    objective: &'static str,
    triggers: &'static [(&'static str, &'static str, bool)],
}

const DEMO: [DemoAsset; 13] = [
    DemoAsset {
        ticker: "AAPL",
        name: "Apple",
        class: AssetClass::Equity,
        fixed_income_pct: 0.0,
        economy: Economy::Us,
        sector: Sector::Technology,
        value: 2200.0,
        target: 8.8,
        is_cash: false,
        thesis: "Servicios y ecosistema sostienen el margen.",
        objective: "Crecimiento de servicios de dos dígitos.",
        triggers: &[
            (
                "thesis_broken",
                "El margen de servicios cae dos trimestres seguidos.",
                false,
            ),
            (
                "target_met",
                "La acción llega al precio objetivo antes de 2028.",
                false,
            ),
        ],
    },
    DemoAsset {
        ticker: "MSFT",
        name: "Microsoft",
        class: AssetClass::Equity,
        fixed_income_pct: 0.0,
        economy: Economy::Us,
        sector: Sector::Technology,
        value: 2500.0,
        target: 10.0,
        is_cash: false,
        thesis: "Nube y productividad con ingresos recurrentes.",
        objective: "Azure crece arriba del mercado.",
        triggers: &[(
            "fundamentals",
            "El crecimiento de Azure se desacelera dos trimestres.",
            false,
        )],
    },
    DemoAsset {
        ticker: "GOOGL",
        name: "Alphabet",
        class: AssetClass::Equity,
        fixed_income_pct: 0.0,
        economy: Economy::Us,
        sector: Sector::CommunicationServices,
        value: 2000.0,
        target: 8.0,
        is_cash: false,
        thesis: "Búsqueda y nube financian la inversión en IA.",
        objective: "El free cash flow se mantiene.",
        triggers: &[(
            "target_met",
            "El objetivo de precio se cumplió antes de tiempo.",
            true,
        )],
    },
    DemoAsset {
        ticker: "JNJ",
        name: "Johnson & Johnson",
        class: AssetClass::Equity,
        fixed_income_pct: 0.0,
        economy: Economy::Us,
        sector: Sector::HealthCare,
        value: 2000.0,
        target: 8.0,
        is_cash: false,
        thesis: "Defensivo con caja y dividendo.",
        objective: "Sostener el dividendo y el margen.",
        triggers: &[(
            "thesis_broken",
            "Un litigio cambia el perfil de riesgo de la compañía.",
            false,
        )],
    },
    DemoAsset {
        ticker: "KO",
        name: "Coca-Cola",
        class: AssetClass::Equity,
        fixed_income_pct: 0.0,
        economy: Economy::Us,
        sector: Sector::ConsumerStaples,
        value: 1500.0,
        target: 6.0,
        is_cash: false,
        thesis: "Consumo masivo con marca y distribución.",
        objective: "Volumen estable y precio.",
        triggers: &[],
    },
    DemoAsset {
        ticker: "XOM",
        name: "Exxon Mobil",
        class: AssetClass::Equity,
        fixed_income_pct: 0.0,
        economy: Economy::Us,
        sector: Sector::Energy,
        value: 1750.0,
        target: 7.0,
        is_cash: false,
        thesis: "Energía como cobertura del ciclo.",
        objective: "Caja libre para dividendo y recompras.",
        triggers: &[(
            "fundamentals",
            "El precio del crudo cae y el capex no se ajusta.",
            false,
        )],
    },
    DemoAsset {
        ticker: "SAP",
        name: "SAP",
        class: AssetClass::Equity,
        fixed_income_pct: 0.0,
        economy: Economy::Europe,
        sector: Sector::Technology,
        value: 1500.0,
        target: 6.0,
        is_cash: false,
        thesis: "Software empresarial europeo con migración a nube.",
        objective: "Crecimiento de la nube por encima del 20 %.",
        triggers: &[(
            "thesis_broken",
            "La migración a la nube se frena dos años.",
            false,
        )],
    },
    DemoAsset {
        ticker: "V",
        name: "Visa",
        class: AssetClass::Equity,
        fixed_income_pct: 0.0,
        economy: Economy::Us,
        sector: Sector::Financials,
        value: 750.0,
        target: 3.0,
        is_cash: false,
        thesis: "Pagos digitales con red y escala global.",
        objective: "Crecimiento del volumen de pagos.",
        triggers: &[(
            "thesis_broken",
            "El volumen de pagos se estanca dos años.",
            false,
        )],
    },
    DemoAsset {
        ticker: "AL30",
        name: "Bono soberano AL30",
        class: AssetClass::FixedIncome,
        fixed_income_pct: 100.0,
        economy: Economy::Argentina,
        sector: Sector::Other,
        value: 2500.0,
        target: 10.0,
        is_cash: false,
        thesis: "Renta fija soberana como ancla y liquidez.",
        objective: "Cobrar cupones y sostener la paridad.",
        triggers: &[(
            "thesis_broken",
            "Cambia el régimen de pago de la deuda.",
            false,
        )],
    },
    DemoAsset {
        ticker: "GD30",
        name: "Bono soberano GD30",
        class: AssetClass::FixedIncome,
        fixed_income_pct: 100.0,
        economy: Economy::Argentina,
        sector: Sector::Other,
        value: 2000.0,
        target: 8.0,
        is_cash: false,
        thesis: "Ley extranjera como complemento del AL30.",
        objective: "Reducir el riesgo de una sola serie.",
        triggers: &[(
            "thesis_broken",
            "El canje cambia las condiciones originales.",
            false,
        )],
    },
    DemoAsset {
        ticker: "YPFD",
        name: "ON YPF",
        class: AssetClass::FixedIncome,
        fixed_income_pct: 100.0,
        economy: Economy::Argentina,
        sector: Sector::Energy,
        value: 750.0,
        target: 3.0,
        is_cash: false,
        thesis: "Crédito corporativo con tasa atractiva.",
        objective: "Cobrar los cupones sin sobreexponerse.",
        triggers: &[("fundamentals", "La calificación baja dos escalones.", false)],
    },
    DemoAsset {
        ticker: "FCI",
        name: "Fondo balanceado",
        class: AssetClass::Mixed,
        fixed_income_pct: 60.0,
        economy: Economy::Global,
        sector: Sector::Other,
        value: 2000.0,
        target: 8.0,
        is_cash: false,
        thesis: "Fondo mixto como diversificación global.",
        objective: "Aportar retorno sin cambiar el perfil.",
        triggers: &[(
            "target_met",
            "El fondo alcanza el rendimiento objetivo anual.",
            false,
        )],
    },
    DemoAsset {
        ticker: "USD",
        name: "Liquidez en dólares",
        class: AssetClass::FixedIncome,
        fixed_income_pct: 100.0,
        economy: Economy::Us,
        sector: Sector::Other,
        value: 3550.0,
        target: 14.2,
        is_cash: true,
        thesis: "",
        objective: "",
        triggers: &[],
    },
];

pub async fn load_example(conn: &mut PgConnection, user_id: i32) -> Result<(), String> {
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM assets WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&mut *conn)
        .await
        .map_err(|_| "Error al revisar la cartera".to_string())?;
    if count.0 > 0 {
        return Err("Ya tenés activos cargados: vaciá la cartera antes de traer el ejemplo".into());
    }
    ensure_settings(&mut *conn, user_id).await?;

    for (index, demo) in DEMO.iter().enumerate() {
        let (internal_id,): (i32,) = sqlx::query_as(
            "INSERT INTO assets (
                user_id, ticker, name, asset_class, fixed_income_pct, economy, sector,
                market_value, currency, target_weight_pct, thesis, objective, is_cash, sort_order
             ) VALUES ($1, $2, $3, $4, $5::numeric, $6, $7, $8::numeric, 'USD', $9::numeric, $10, $11, $12, $13)
             RETURNING id",
        )
        .bind(user_id)
        .bind(demo.ticker)
        .bind(demo.name)
        .bind(demo.class.id())
        .bind(demo.fixed_income_pct)
        .bind(demo.economy.id())
        .bind(demo.sector.id())
        .bind(demo.value)
        .bind(demo.target)
        .bind(demo.thesis)
        .bind(demo.objective)
        .bind(demo.is_cash)
        .bind(index as i16)
        .fetch_one(&mut *conn)
        .await
        .map_err(|_| "Error al crear un activo del ejemplo".to_string())?;

        for &(kind, description, is_met) in demo.triggers {
            sqlx::query(
                "INSERT INTO sell_triggers (user_id, asset_id, kind, description, is_met)
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(user_id)
            .bind(internal_id)
            .bind(kind)
            .bind(description)
            .bind(is_met)
            .execute(&mut *conn)
            .await
            .map_err(|_| "Error al crear un disparador del ejemplo".to_string())?;
        }
    }

    Ok(())
}

// ------------------------------------------------------------ handlers ----

fn error_response(msg: String) -> HttpResponse {
    if msg.starts_with("Activo no encontrado") || msg.starts_with("Disparador no encontrado") {
        HttpResponse::NotFound().body(msg)
    } else if msg.starts_with("Error al") {
        HttpResponse::InternalServerError().body(msg)
    } else {
        HttpResponse::BadRequest().body(msg)
    }
}

async fn state_response(
    conn: &mut PgConnection,
    user_id: i32,
    fx_result: Result<Rates, String>,
) -> HttpResponse {
    match load_state(conn, user_id, fx_result).await {
        Ok(state) => HttpResponse::Ok().json(state),
        Err(msg) => error_response(msg),
    }
}

async fn acquire(
    pool: &web::Data<PgPool>,
) -> Result<sqlx::pool::PoolConnection<sqlx::Postgres>, &'static str> {
    pool.get_ref()
        .acquire()
        .await
        .map_err(|_| "Error de conexión")
}

#[get("/api/portfolio")]
pub async fn portfolio_get(
    session: Session,
    pool: web::Data<PgPool>,
    fx: web::Data<FxCache>,
) -> impl Responder {
    let Some(user_id) = session_user_id(&session) else {
        return HttpResponse::Unauthorized().finish();
    };
    let fx_result = fx.get(false).await;
    let mut conn = match acquire(&pool).await {
        Ok(conn) => conn,
        Err(msg) => return HttpResponse::InternalServerError().body(msg),
    };
    state_response(&mut conn, user_id, fx_result).await
}

#[post("/api/portfolio/refresh")]
pub async fn portfolio_refresh(
    session: Session,
    pool: web::Data<PgPool>,
    fx: web::Data<FxCache>,
) -> impl Responder {
    let Some(user_id) = session_user_id(&session) else {
        return HttpResponse::Unauthorized().finish();
    };
    let fx_result = fx.get(true).await;
    let mut conn = match acquire(&pool).await {
        Ok(conn) => conn,
        Err(msg) => return HttpResponse::InternalServerError().body(msg),
    };
    state_response(&mut conn, user_id, fx_result).await
}

#[post("/api/portfolio/assets")]
pub async fn portfolio_asset_create(
    body: web::Json<AssetInput>,
    session: Session,
    pool: web::Data<PgPool>,
    fx: web::Data<FxCache>,
) -> impl Responder {
    let Some(user_id) = session_user_id(&session) else {
        return HttpResponse::Unauthorized().finish();
    };
    let fx_result = fx.get(false).await;
    let mut conn = match acquire(&pool).await {
        Ok(conn) => conn,
        Err(msg) => return HttpResponse::InternalServerError().body(msg),
    };
    if let Err(msg) = create_asset(&mut conn, user_id, &body).await {
        return error_response(msg);
    }
    state_response(&mut conn, user_id, fx_result).await
}

#[patch("/api/portfolio/assets/{id}")]
pub async fn portfolio_asset_update(
    path: web::Path<Uuid>,
    body: web::Json<AssetInput>,
    session: Session,
    pool: web::Data<PgPool>,
    fx: web::Data<FxCache>,
) -> impl Responder {
    let Some(user_id) = session_user_id(&session) else {
        return HttpResponse::Unauthorized().finish();
    };
    let fx_result = fx.get(false).await;
    let mut conn = match acquire(&pool).await {
        Ok(conn) => conn,
        Err(msg) => return HttpResponse::InternalServerError().body(msg),
    };
    if let Err(msg) = update_asset(&mut conn, user_id, path.into_inner(), &body).await {
        return error_response(msg);
    }
    state_response(&mut conn, user_id, fx_result).await
}

#[delete("/api/portfolio/assets/{id}")]
pub async fn portfolio_asset_delete(
    path: web::Path<Uuid>,
    session: Session,
    pool: web::Data<PgPool>,
    fx: web::Data<FxCache>,
) -> impl Responder {
    let Some(user_id) = session_user_id(&session) else {
        return HttpResponse::Unauthorized().finish();
    };
    let fx_result = fx.get(false).await;
    let mut conn = match acquire(&pool).await {
        Ok(conn) => conn,
        Err(msg) => return HttpResponse::InternalServerError().body(msg),
    };
    match delete_asset(&mut conn, user_id, path.into_inner()).await {
        Ok(0) => HttpResponse::NotFound().finish(),
        Ok(_) => state_response(&mut conn, user_id, fx_result).await,
        Err(_) => HttpResponse::InternalServerError().body("Error al borrar el activo"),
    }
}

#[post("/api/portfolio/assets/{id}/triggers")]
pub async fn portfolio_trigger_create(
    path: web::Path<Uuid>,
    body: web::Json<TriggerInput>,
    session: Session,
    pool: web::Data<PgPool>,
    fx: web::Data<FxCache>,
) -> impl Responder {
    let Some(user_id) = session_user_id(&session) else {
        return HttpResponse::Unauthorized().finish();
    };
    let fx_result = fx.get(false).await;
    let mut conn = match acquire(&pool).await {
        Ok(conn) => conn,
        Err(msg) => return HttpResponse::InternalServerError().body(msg),
    };
    if let Err(msg) = create_trigger(&mut conn, user_id, path.into_inner(), &body).await {
        return error_response(msg);
    }
    state_response(&mut conn, user_id, fx_result).await
}

#[patch("/api/portfolio/triggers/{id}")]
pub async fn portfolio_trigger_update(
    path: web::Path<Uuid>,
    body: web::Json<TriggerPatch>,
    session: Session,
    pool: web::Data<PgPool>,
    fx: web::Data<FxCache>,
) -> impl Responder {
    let Some(user_id) = session_user_id(&session) else {
        return HttpResponse::Unauthorized().finish();
    };
    let fx_result = fx.get(false).await;
    let mut conn = match acquire(&pool).await {
        Ok(conn) => conn,
        Err(msg) => return HttpResponse::InternalServerError().body(msg),
    };
    if let Err(msg) = update_trigger(&mut conn, user_id, path.into_inner(), &body).await {
        return error_response(msg);
    }
    state_response(&mut conn, user_id, fx_result).await
}

#[delete("/api/portfolio/triggers/{id}")]
pub async fn portfolio_trigger_delete(
    path: web::Path<Uuid>,
    session: Session,
    pool: web::Data<PgPool>,
    fx: web::Data<FxCache>,
) -> impl Responder {
    let Some(user_id) = session_user_id(&session) else {
        return HttpResponse::Unauthorized().finish();
    };
    let fx_result = fx.get(false).await;
    let mut conn = match acquire(&pool).await {
        Ok(conn) => conn,
        Err(msg) => return HttpResponse::InternalServerError().body(msg),
    };
    match delete_trigger(&mut conn, user_id, path.into_inner()).await {
        Ok(0) => HttpResponse::NotFound().finish(),
        Ok(_) => state_response(&mut conn, user_id, fx_result).await,
        Err(_) => HttpResponse::InternalServerError().body("Error al borrar el disparador"),
    }
}

/// Convierte un monto de la vista a la moneda nativa del activo. La tasa
/// es ARS por USD y solo hace falta si las monedas difieren.
fn display_to_native(value: f64, native: &str, display: &str, rate: Option<f64>) -> Option<f64> {
    if native == display {
        return Some(value);
    }
    let rate = rate.filter(|r| r.is_finite() && *r > 0.0)?;
    match (native, display) {
        ("USD", "ARS") => Some(value / rate),
        ("ARS", "USD") => Some(value * rate),
        _ => None,
    }
}

fn native_to_display(value: f64, native: &str, display: &str, rate: Option<f64>) -> Option<f64> {
    if native == display {
        return Some(value);
    }
    let rate = rate.filter(|r| r.is_finite() && *r > 0.0)?;
    match (native, display) {
        ("USD", "ARS") => Some(value * rate),
        ("ARS", "USD") => Some(value / rate),
        _ => None,
    }
}

/// Persiste un plan de reparto de objetivos. No toca valores de mercado.
pub async fn apply_target_plan(
    conn: &mut PgConnection,
    user_id: i32,
    plan: &TargetPlan,
) -> Result<(), String> {
    for change in &plan.changes {
        if (change.new_target_pct - change.old_target_pct).abs() < 0.005 {
            continue;
        }
        let result = sqlx::query(
            "UPDATE assets SET target_weight_pct = $3::numeric, updated_at = CURRENT_TIMESTAMP
             WHERE public_id = $1 AND user_id = $2 AND is_active AND NOT is_cash",
        )
        .bind(change.asset_id)
        .bind(user_id)
        .bind(change.new_target_pct)
        .execute(&mut *conn)
        .await
        .map_err(|_| "Error al guardar los objetivos".to_string())?;
        if result.rows_affected() == 0 {
            return Err("Activo no encontrado".into());
        }
    }
    Ok(())
}

/// Ejecuta una orden de rebalanceo. Si hay un activo marcado como
/// liquidez, el activo va a su objetivo y la diferencia se mueve a la
/// liquidez: el total no cambia y el peso queda exacto. Sin liquidez,
/// el activo va al valor objetivo y el total se recalcula.
pub async fn execute_rebalance(
    conn: &mut PgConnection,
    user_id: i32,
    asset_id: Uuid,
    fx_result: Result<Rates, String>,
) -> Result<PortfolioState, String> {
    let mut moved = false;
    for _ in 0..8 {
        let state = load_state(&mut *conn, user_id, fx_result.clone()).await?;
        let Some(analysis) = state.analysis else {
            return Err("La cartera no se puede valuar en este momento".into());
        };
        if !analysis
            .orders
            .iter()
            .any(|order| order.asset_id == asset_id)
        {
            break;
        }
        let Some(row) = analysis.rows.iter().find(|row| row.id == asset_id) else {
            break;
        };
        if row.is_cash {
            return Err("La liquidez no se ejecuta con una orden propia".into());
        }
        let target_display = analysis.total_display * row.target_weight_pct / 100.0;
        let target_native = display_to_native(
            target_display,
            &row.currency,
            &analysis.display_currency,
            analysis.rate_used,
        )
        .ok_or_else(|| "No hay tasa para ajustar el activo".to_string())?;

        if let Some(cash) = state.assets.iter().find(|asset| asset.is_cash) {
            let difference = row.value_display - target_display;
            let cash_delta = display_to_native(
                difference,
                &cash.currency,
                &analysis.display_currency,
                analysis.rate_used,
            )
            .ok_or_else(|| "No hay tasa para ajustar la liquidez".to_string())?;
            let cash_value = cash.market_value + cash_delta;
            if cash_value < 0.0 {
                return Err("No hay liquidez suficiente para pagar la compra".into());
            }
            let asset_update = sqlx::query(
                "UPDATE assets SET market_value = $3::numeric, updated_at = CURRENT_TIMESTAMP
                 WHERE public_id = $1 AND user_id = $2 AND is_active",
            )
            .bind(asset_id)
            .bind(user_id)
            .bind(target_native)
            .execute(&mut *conn)
            .await
            .map_err(|_| "Error al ajustar el activo".to_string())?;
            if asset_update.rows_affected() == 0 {
                return Err("Activo no encontrado".into());
            }
            let cash_update = sqlx::query(
                "UPDATE assets SET market_value = $3::numeric, updated_at = CURRENT_TIMESTAMP
                 WHERE public_id = $1 AND user_id = $2 AND is_active",
            )
            .bind(cash.id)
            .bind(user_id)
            .bind(cash_value)
            .execute(&mut *conn)
            .await
            .map_err(|_| "Error al ajustar la liquidez".to_string())?;
            if cash_update.rows_affected() == 0 {
                return Err("Liquidez no encontrada".into());
            }
            moved = true;
            break;
        }

        let result = sqlx::query(
            "UPDATE assets SET market_value = $3::numeric, updated_at = CURRENT_TIMESTAMP
             WHERE public_id = $1 AND user_id = $2 AND is_active",
        )
        .bind(asset_id)
        .bind(user_id)
        .bind(target_native)
        .execute(&mut *conn)
        .await
        .map_err(|_| "Error al ajustar el activo".to_string())?;
        if result.rows_affected() == 0 {
            return Err("Activo no encontrado".into());
        }
        moved = true;
    }
    if !moved {
        return Err("La orden ya está dentro de la banda".into());
    }
    load_state(&mut *conn, user_id, fx_result).await
}

#[post("/api/portfolio/assets/{id}/rebalance")]
pub async fn portfolio_rebalance_execute(
    path: web::Path<Uuid>,
    session: Session,
    pool: web::Data<PgPool>,
    fx: web::Data<FxCache>,
) -> impl Responder {
    let Some(user_id) = session_user_id(&session) else {
        return HttpResponse::Unauthorized().finish();
    };
    let asset_id = path.into_inner();
    let fx_result = fx.get(false).await;
    let mut conn = match acquire(&pool).await {
        Ok(conn) => conn,
        Err(msg) => return HttpResponse::InternalServerError().body(msg),
    };
    let mut tx = match conn.begin().await {
        Ok(tx) => tx,
        Err(_) => return HttpResponse::InternalServerError().body("Error al abrir la transacción"),
    };
    match execute_rebalance(&mut tx, user_id, asset_id, fx_result).await {
        Ok(state) => {
            if tx.commit().await.is_err() {
                return HttpResponse::InternalServerError().body("Error al guardar el rebalanceo");
            }
            HttpResponse::Ok().json(state)
        }
        Err(msg) => {
            let _ = tx.rollback().await;
            error_response(msg)
        }
    }
}

#[post("/api/portfolio/targets/preview")]
pub async fn portfolio_targets_preview(
    body: web::Json<TargetPlanInput>,
    session: Session,
    pool: web::Data<PgPool>,
) -> impl Responder {
    let Some(user_id) = session_user_id(&session) else {
        return HttpResponse::Unauthorized().finish();
    };
    let mut conn = match acquire(&pool).await {
        Ok(conn) => conn,
        Err(msg) => return HttpResponse::InternalServerError().body(msg),
    };
    let settings = match ensure_settings(&mut conn, user_id).await {
        Ok(settings) => settings,
        Err(msg) => return error_response(msg),
    };
    let assets = match load_assets(&mut conn, user_id).await {
        Ok(assets) => assets,
        Err(msg) => return error_response(msg),
    };
    match plan_targets(&settings, &assets, body.sleeve, body.mode) {
        Ok(plan) => HttpResponse::Ok().json(plan),
        Err(msg) => error_response(msg),
    }
}

#[post("/api/portfolio/targets/apply")]
pub async fn portfolio_targets_apply(
    body: web::Json<TargetPlanInput>,
    session: Session,
    pool: web::Data<PgPool>,
    fx: web::Data<FxCache>,
) -> impl Responder {
    let Some(user_id) = session_user_id(&session) else {
        return HttpResponse::Unauthorized().finish();
    };
    let fx_result = fx.get(false).await;
    let mut conn = match acquire(&pool).await {
        Ok(conn) => conn,
        Err(msg) => return HttpResponse::InternalServerError().body(msg),
    };
    let mut tx = match conn.begin().await {
        Ok(tx) => tx,
        Err(_) => return HttpResponse::InternalServerError().body("Error al abrir la transacción"),
    };
    let settings = match ensure_settings(&mut tx, user_id).await {
        Ok(settings) => settings,
        Err(msg) => {
            let _ = tx.rollback().await;
            return error_response(msg);
        }
    };
    let assets = match load_assets(&mut tx, user_id).await {
        Ok(assets) => assets,
        Err(msg) => {
            let _ = tx.rollback().await;
            return error_response(msg);
        }
    };
    let plan = match plan_targets(&settings, &assets, body.sleeve, body.mode) {
        Ok(plan) => plan,
        Err(msg) => {
            let _ = tx.rollback().await;
            return error_response(msg);
        }
    };
    if let Err(msg) = apply_target_plan(&mut tx, user_id, &plan).await {
        let _ = tx.rollback().await;
        return error_response(msg);
    }
    if tx.commit().await.is_err() {
        return HttpResponse::InternalServerError().body("Error al guardar los objetivos");
    }
    state_response(&mut conn, user_id, fx_result).await
}

/// Ejecuta un disparador de venta: el activo se vende completo (valor 0,
/// inactivo), el disparador queda resuelto y la cartera se recalcula.
pub async fn execute_trigger(
    conn: &mut PgConnection,
    user_id: i32,
    trigger_id: Uuid,
    fx_result: Result<Rates, String>,
) -> Result<PortfolioState, String> {
    let state = load_state(&mut *conn, user_id, fx_result.clone()).await?;
    let Some((asset, _)) = state.assets.iter().find_map(|asset| {
        asset
            .triggers
            .iter()
            .find(|trigger| trigger.id == trigger_id && trigger.is_met && !trigger.is_resolved)
            .map(|trigger| (asset, trigger))
    }) else {
        return Err("Disparador no encontrado o no cumplido".into());
    };
    if asset.is_cash {
        return Err("La liquidez no lleva disparadores".into());
    }
    let display_currency = state.settings.valuation.display_currency();
    let rate = state
        .analysis
        .as_ref()
        .and_then(|analysis| analysis.rate_used)
        .or_else(|| match state.settings.valuation.rate_kind() {
            RateKind::Mep => state.fx.mep,
            RateKind::Ccl => state.fx.ccl,
        });

    if let Some(cash) = state.assets.iter().find(|asset| asset.is_cash) {
        let asset_display =
            native_to_display(asset.market_value, &asset.currency, display_currency, rate)
                .ok_or_else(|| "No hay tasa para valuar la venta".to_string())?;
        let cash_delta = display_to_native(asset_display, &cash.currency, display_currency, rate)
            .ok_or_else(|| "No hay tasa para acreditar la liquidez".to_string())?;
        let cash_value = cash.market_value + cash_delta;
        let cash_update = sqlx::query(
            "UPDATE assets SET market_value = $3::numeric, updated_at = CURRENT_TIMESTAMP
             WHERE public_id = $1 AND user_id = $2 AND is_active",
        )
        .bind(cash.id)
        .bind(user_id)
        .bind(cash_value)
        .execute(&mut *conn)
        .await
        .map_err(|_| "Error al acreditar la liquidez".to_string())?;
        if cash_update.rows_affected() == 0 {
            return Err("Liquidez no encontrada".into());
        }
    }

    let asset_update = sqlx::query(
        "UPDATE assets SET market_value = 0, is_active = false, updated_at = CURRENT_TIMESTAMP
         WHERE public_id = $1 AND user_id = $2 AND is_active",
    )
    .bind(asset.id)
    .bind(user_id)
    .execute(&mut *conn)
    .await
    .map_err(|_| "Error al vender el activo".to_string())?;
    if asset_update.rows_affected() == 0 {
        return Err("Activo no encontrado".into());
    }
    sqlx::query(
        "UPDATE sell_triggers SET is_resolved = true WHERE public_id = $1 AND user_id = $2",
    )
    .bind(trigger_id)
    .bind(user_id)
    .execute(&mut *conn)
    .await
    .map_err(|_| "Error al resolver el disparador".to_string())?;
    load_state(&mut *conn, user_id, fx_result).await
}

#[post("/api/portfolio/triggers/{id}/execute")]
pub async fn portfolio_trigger_execute(
    path: web::Path<Uuid>,
    session: Session,
    pool: web::Data<PgPool>,
    fx: web::Data<FxCache>,
) -> impl Responder {
    let Some(user_id) = session_user_id(&session) else {
        return HttpResponse::Unauthorized().finish();
    };
    let trigger_id = path.into_inner();
    let fx_result = fx.get(false).await;
    let mut conn = match acquire(&pool).await {
        Ok(conn) => conn,
        Err(msg) => return HttpResponse::InternalServerError().body(msg),
    };
    let mut tx = match conn.begin().await {
        Ok(tx) => tx,
        Err(_) => return HttpResponse::InternalServerError().body("Error al abrir la transacción"),
    };
    match execute_trigger(&mut tx, user_id, trigger_id, fx_result).await {
        Ok(state) => {
            if tx.commit().await.is_err() {
                return HttpResponse::InternalServerError().body("Error al guardar la ejecución");
            }
            HttpResponse::Ok().json(state)
        }
        Err(msg) => {
            let _ = tx.rollback().await;
            error_response(msg)
        }
    }
}

/// Vaciar la cartera: borra todos los activos (y sus disparadores) del
/// usuario. Los criterios y la vista se conservan.
pub async fn clear_assets(conn: &mut PgConnection, user_id: i32) -> sqlx::Result<u64> {
    let result = sqlx::query("DELETE FROM assets WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *conn)
        .await?;
    Ok(result.rows_affected())
}

#[delete("/api/portfolio/assets")]
pub async fn portfolio_assets_clear(
    session: Session,
    pool: web::Data<PgPool>,
    fx: web::Data<FxCache>,
) -> impl Responder {
    let Some(user_id) = session_user_id(&session) else {
        return HttpResponse::Unauthorized().finish();
    };
    let fx_result = fx.get(false).await;
    let mut conn = match acquire(&pool).await {
        Ok(conn) => conn,
        Err(msg) => return HttpResponse::InternalServerError().body(msg),
    };
    if clear_assets(&mut conn, user_id).await.is_err() {
        return HttpResponse::InternalServerError().body("Error al vaciar la cartera");
    }
    state_response(&mut conn, user_id, fx_result).await
}

#[put("/api/portfolio/settings")]
pub async fn portfolio_settings_update(
    body: web::Json<SettingsInput>,
    session: Session,
    pool: web::Data<PgPool>,
    fx: web::Data<FxCache>,
) -> impl Responder {
    let Some(user_id) = session_user_id(&session) else {
        return HttpResponse::Unauthorized().finish();
    };
    let fx_result = fx.get(false).await;
    let mut conn = match acquire(&pool).await {
        Ok(conn) => conn,
        Err(msg) => return HttpResponse::InternalServerError().body(msg),
    };
    if let Err(msg) = update_settings(&mut conn, user_id, &body).await {
        return error_response(msg);
    }
    state_response(&mut conn, user_id, fx_result).await
}

#[post("/api/portfolio/example")]
pub async fn portfolio_example(
    session: Session,
    pool: web::Data<PgPool>,
    fx: web::Data<FxCache>,
) -> impl Responder {
    let Some(user_id) = session_user_id(&session) else {
        return HttpResponse::Unauthorized().finish();
    };
    let fx_result = fx.get(false).await;
    let mut conn = match acquire(&pool).await {
        Ok(conn) => conn,
        Err(msg) => return HttpResponse::InternalServerError().body(msg),
    };
    let mut tx = match conn.begin().await {
        Ok(tx) => tx,
        Err(_) => return HttpResponse::InternalServerError().body("Error al abrir la transacción"),
    };
    if let Err(msg) = load_example(&mut tx, user_id).await {
        let _ = tx.rollback().await;
        return error_response(msg);
    }
    if tx.commit().await.is_err() {
        return HttpResponse::InternalServerError().body("Error al guardar el ejemplo");
    }
    state_response(&mut conn, user_id, fx_result).await
}

#[cfg(test)]
mod tests {
    use super::{
        AssetInput, SettingsInput, analyze, apply_target_plan, clear_assets, create_asset,
        create_trigger, delete_asset, ensure_settings, execute_rebalance, execute_trigger,
        load_assets, load_example, update_asset, update_settings, update_trigger,
    };
    use crate::market::fx::Rates;
    use crate::portfolio::targets::{TargetMode, TargetSleeve, plan_targets};
    use sqlx::Connection;
    use sqlx::PgConnection;
    use std::time::{SystemTime, UNIX_EPOCH};
    use uuid::Uuid;

    async fn connect() -> PgConnection {
        dotenvy::dotenv().ok();
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL para los tests");
        PgConnection::connect(&url).await.expect("conexión de test")
    }

    async fn make_user(tx: &mut PgConnection) -> i32 {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64 + d.as_secs())
            .unwrap_or(1);
        let email = format!("test-portfolio-{nanos}@rfinance.local");
        let (id,): (i32,) =
            sqlx::query_as("INSERT INTO users (email, password) VALUES ($1, 'x') RETURNING id")
                .bind(&email)
                .fetch_one(&mut *tx)
                .await
                .expect("user de test");
        id
    }

    fn input(name: &str, class: &str, value: f64) -> AssetInput {
        AssetInput {
            ticker: Some(name.to_string()),
            name: name.to_string(),
            asset_class: class.to_string(),
            fixed_income_pct: Some(50.0),
            economy: "us".into(),
            sector: "technology".into(),
            market_value: value,
            currency: "USD".into(),
            target_weight_pct: 10.0,
            thesis: Some("tesis".into()),
            objective: Some("objetivo".into()),
            is_cash: Some(false),
        }
    }

    fn settings_input() -> SettingsInput {
        SettingsInput {
            risk_profile: "conservative".into(),
            valuation: "ars_ccl".into(),
            target_fixed_income_pct: 70.0,
            target_equity_pct: 30.0,
            rebalance_band_pp: 3.0,
            min_assets: 6,
            max_assets: 10,
            max_asset_pct: 12.0,
            max_economy_pct: 60.0,
            max_cluster_pct: 25.0,
        }
    }

    #[actix_web::test]
    async fn alta_edicion_y_baja_de_activo() {
        let mut conn = connect().await;
        let mut tx = conn.begin().await.expect("tx");
        let user = make_user(&mut tx).await;

        let asset = create_asset(&mut tx, user, &input("AAPL", "equity", 100.0))
            .await
            .expect("alta");
        let assets = load_assets(&mut tx, user).await.expect("activos");
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].id, asset);
        assert!((assets[0].fixed_income_pct - 0.0).abs() < 0.01);

        let mut edit = input("AAPL", "mixed", 120.0);
        edit.fixed_income_pct = Some(60.0);
        edit.target_weight_pct = 20.0;
        update_asset(&mut tx, user, asset, &edit)
            .await
            .expect("edición");
        let assets = load_assets(&mut tx, user).await.expect("activos editados");
        assert_eq!(assets[0].asset_class.id(), "mixed");
        assert!((assets[0].fixed_income_pct - 60.0).abs() < 0.01);
        assert!((assets[0].target_weight_pct - 20.0).abs() < 0.01);

        assert_eq!(delete_asset(&mut tx, user, asset).await.expect("baja"), 1);
        assert!(load_assets(&mut tx, user).await.expect("vacíos").is_empty());

        tx.rollback().await.expect("rollback");
    }

    #[actix_web::test]
    async fn activo_ajeno_no_se_toca() {
        let mut conn = connect().await;
        let mut tx = conn.begin().await.expect("tx");
        let owner = make_user(&mut tx).await;
        let other = make_user(&mut tx).await;
        let asset = create_asset(&mut tx, owner, &input("AAPL", "equity", 100.0))
            .await
            .expect("alta");

        let err = update_asset(&mut tx, other, asset, &input("AAPL", "equity", 50.0))
            .await
            .expect_err("ajeno");
        assert_eq!(err, "Activo no encontrado");
        assert_eq!(
            delete_asset(&mut tx, other, asset)
                .await
                .expect("baja ajena"),
            0
        );

        tx.rollback().await.expect("rollback");
    }

    #[actix_web::test]
    async fn disparadores_cuelgan_del_activo() {
        let mut conn = connect().await;
        let mut tx = conn.begin().await.expect("tx");
        let user = make_user(&mut tx).await;
        let asset = create_asset(&mut tx, user, &input("AAPL", "equity", 100.0))
            .await
            .expect("alta");

        let trigger = create_trigger(
            &mut tx,
            user,
            asset,
            &super::TriggerInput {
                kind: "thesis_broken".into(),
                description: "Si cambia la tesis".into(),
            },
        )
        .await
        .expect("disparador");
        update_trigger(
            &mut tx,
            user,
            trigger,
            &super::TriggerPatch {
                kind: None,
                description: None,
                is_met: Some(true),
                is_resolved: None,
            },
        )
        .await
        .expect("marcar");
        let assets = load_assets(&mut tx, user).await.expect("activos");
        assert_eq!(assets[0].triggers.len(), 1);
        assert!(assets[0].triggers[0].is_met);
        assert!(!assets[0].triggers[0].is_resolved);

        update_trigger(
            &mut tx,
            user,
            trigger,
            &super::TriggerPatch {
                kind: None,
                description: None,
                is_met: None,
                is_resolved: Some(true),
            },
        )
        .await
        .expect("marcar ejecutado");
        let assets = load_assets(&mut tx, user).await.expect("activos");
        assert!(assets[0].triggers[0].is_resolved);

        update_trigger(
            &mut tx,
            user,
            trigger,
            &super::TriggerPatch {
                kind: None,
                description: None,
                is_met: Some(false),
                is_resolved: None,
            },
        )
        .await
        .expect("reabrir");
        let assets = load_assets(&mut tx, user).await.expect("activos");
        assert!(!assets[0].triggers[0].is_met);
        assert!(!assets[0].triggers[0].is_resolved);

        delete_asset(&mut tx, user, asset).await.expect("baja");
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sell_triggers WHERE user_id = $1")
            .bind(user)
            .fetch_one(&mut *tx)
            .await
            .expect("conteo");
        assert_eq!(count.0, 0);

        tx.rollback().await.expect("rollback");
    }

    #[actix_web::test]
    async fn criterios_por_defecto_y_edicion() {
        let mut conn = connect().await;
        let mut tx = conn.begin().await.expect("tx");
        let user = make_user(&mut tx).await;

        let settings = ensure_settings(&mut tx, user).await.expect("criterios");
        assert_eq!(settings.risk_profile.id(), "aggressive");
        assert!((settings.target_fixed_income_pct - 40.0).abs() < 0.01);

        update_settings(&mut tx, user, &settings_input())
            .await
            .expect("edición");
        let settings = ensure_settings(&mut tx, user).await.expect("criterios");
        assert_eq!(settings.risk_profile.id(), "conservative");
        assert_eq!(settings.valuation.id(), "ars_ccl");
        assert!((settings.target_equity_pct - 30.0).abs() < 0.01);

        let mut invalid = settings_input();
        invalid.target_equity_pct = 40.0;
        assert!(update_settings(&mut tx, user, &invalid).await.is_err());

        tx.rollback().await.expect("rollback");
    }

    #[actix_web::test]
    async fn ejemplo_solo_sobre_cartera_vacia() {
        let mut conn = connect().await;
        let mut tx = conn.begin().await.expect("tx");
        let user = make_user(&mut tx).await;

        load_example(&mut tx, user).await.expect("ejemplo");
        let assets = load_assets(&mut tx, user).await.expect("activos");
        assert_eq!(assets.len(), 13);
        assert!(assets.iter().any(|asset| asset.is_cash));
        assert_eq!(
            assets
                .iter()
                .filter(|asset| {
                    asset.asset_class == crate::portfolio::model::AssetClass::Equity
                })
                .count(),
            8
        );
        let targets: f64 = assets.iter().map(|a| a.target_weight_pct).sum();
        assert!((targets - 100.0).abs() < 0.01);
        let settings = ensure_settings(&mut tx, user).await.expect("settings");
        let analysis = analyze(&settings, &assets, settings.valuation, Some(1_000.0))
            .expect("análisis del ejemplo");
        assert!((analysis.rf_pct - 40.0).abs() < 0.01, "{}", analysis.rf_pct);
        assert!((analysis.rv_pct - 60.0).abs() < 0.01, "{}", analysis.rv_pct);
        assert!(analysis.orders.is_empty(), "quedaron órdenes: {analysis:?}");
        assert!(load_example(&mut tx, user).await.is_err());

        tx.rollback().await.expect("rollback");
    }

    #[actix_web::test]
    async fn reparto_de_objetivos_rv_solo_cambia_porcentajes() {
        let mut conn = connect().await;
        let mut tx = conn.begin().await.expect("tx");
        let user = make_user(&mut tx).await;

        let mut aapl_input = input("AAPL", "equity", 100.0);
        aapl_input.target_weight_pct = 30.0;
        create_asset(&mut tx, user, &aapl_input)
            .await
            .expect("AAPL");
        let mut msft_input = input("MSFT", "equity", 200.0);
        msft_input.target_weight_pct = 20.0;
        create_asset(&mut tx, user, &msft_input)
            .await
            .expect("MSFT");
        let mut bond_input = input("BONO", "fixed_income", 300.0);
        bond_input.target_weight_pct = 40.0;
        bond_input.economy = "argentina".into();
        bond_input.sector = "other".into();
        create_asset(&mut tx, user, &bond_input)
            .await
            .expect("BONO");

        let settings = ensure_settings(&mut tx, user).await.expect("settings");
        let before = load_assets(&mut tx, user).await.expect("activos");
        let plan = plan_targets(&settings, &before, TargetSleeve::Equity, TargetMode::Equal)
            .expect("plan RV");
        assert_eq!(plan.changes.len(), 2);
        for change in &plan.changes {
            assert!((change.new_target_pct - 30.0).abs() < 0.01);
        }
        assert!((plan.total_after - 100.0).abs() < 0.01);
        assert!((plan.rf_target_after - 40.0).abs() < 0.01);

        apply_target_plan(&mut tx, user, &plan)
            .await
            .expect("aplicar objetivos");
        let after = load_assets(&mut tx, user).await.expect("activos");
        for asset in &after {
            let expected = match asset.ticker.as_deref() {
                Some("AAPL") | Some("MSFT") => 30.0,
                Some("BONO") => 40.0,
                _ => unreachable!("activo inesperado"),
            };
            assert!(
                (asset.target_weight_pct - expected).abs() < 0.01,
                "{}: {}",
                asset.display(),
                asset.target_weight_pct
            );
        }
        let value = |ticker: &str| {
            after
                .iter()
                .find(|asset| asset.ticker.as_deref() == Some(ticker))
                .map(|asset| asset.market_value)
                .expect(ticker)
        };
        assert!((value("AAPL") - 100.0).abs() < 0.01);
        assert!((value("MSFT") - 200.0).abs() < 0.01);
        assert!((value("BONO") - 300.0).abs() < 0.01);
        assert!((after.iter().map(|a| a.target_weight_pct).sum::<f64>() - 100.0).abs() < 0.01);

        tx.rollback().await.expect("rollback");
    }

    #[actix_web::test]
    async fn ejecutar_rebalanceo_ajusta_al_objetivo() {
        let mut conn = connect().await;
        let mut tx = conn.begin().await.expect("tx");
        let user = make_user(&mut tx).await;
        let mut aapl_input = input("AAPL", "equity", 180.0);
        aapl_input.target_weight_pct = 10.0;
        let aapl = create_asset(&mut tx, user, &aapl_input)
            .await
            .expect("AAPL");
        let mut msft_input = input("MSFT", "equity", 820.0);
        msft_input.target_weight_pct = 90.0;
        create_asset(&mut tx, user, &msft_input)
            .await
            .expect("MSFT");

        let rates = Rates {
            mep: 1_000.0,
            ccl: 1_100.0,
            fetched_at: 0,
            stale: false,
        };
        let state = execute_rebalance(&mut tx, user, aapl, Ok(rates))
            .await
            .expect("ejecutar rebalanceo");
        let analysis = state.analysis.expect("analysis");
        assert!(analysis.orders.is_empty(), "quedaron órdenes: {analysis:?}");
        let row = analysis
            .rows
            .iter()
            .find(|row| row.id == aapl)
            .expect("fila AAPL");
        assert!((row.value_native - 100.0).abs() < 0.01);
        assert!((row.current_weight_pct - 10.87).abs() < 0.2);
        assert!((analysis.total_display - 920.0).abs() < 0.01);

        tx.rollback().await.expect("rollback");
    }

    #[actix_web::test]
    async fn ejecutar_disparador_vende_el_activo() {
        let mut conn = connect().await;
        let mut tx = conn.begin().await.expect("tx");
        let user = make_user(&mut tx).await;
        let aapl = create_asset(&mut tx, user, &input("AAPL", "equity", 180.0))
            .await
            .expect("AAPL");
        let trigger = create_trigger(
            &mut tx,
            user,
            aapl,
            &super::TriggerInput {
                kind: "target_met".into(),
                description: "Llegó al objetivo".into(),
            },
        )
        .await
        .expect("disparador");
        update_trigger(
            &mut tx,
            user,
            trigger,
            &super::TriggerPatch {
                kind: None,
                description: None,
                is_met: Some(true),
                is_resolved: None,
            },
        )
        .await
        .expect("cumplido");

        let rates = Rates {
            mep: 1_000.0,
            ccl: 1_100.0,
            fetched_at: 0,
            stale: false,
        };
        let state = execute_trigger(&mut tx, user, trigger, Ok(rates))
            .await
            .expect("ejecutar disparador");
        assert!(state.assets.is_empty());

        let (value, active): (f64, bool) = sqlx::query_as(
            "SELECT market_value::float8, is_active FROM assets WHERE public_id = $1",
        )
        .bind(aapl)
        .fetch_one(&mut *tx)
        .await
        .expect("activo vendido");
        assert!((value - 0.0).abs() < 0.01);
        assert!(!active);

        let (resolved,): (bool,) =
            sqlx::query_as("SELECT is_resolved FROM sell_triggers WHERE public_id = $1")
                .bind(trigger)
                .fetch_one(&mut *tx)
                .await
                .expect("disparador resuelto");
        assert!(resolved);

        tx.rollback().await.expect("rollback");
    }

    #[actix_web::test]
    async fn vaciar_cartera_no_toca_al_otro_usuario() {
        let mut conn = connect().await;
        let mut tx = conn.begin().await.expect("tx");
        let owner = make_user(&mut tx).await;
        let other = make_user(&mut tx).await;
        let first = create_asset(&mut tx, owner, &input("AAPL", "equity", 100.0))
            .await
            .expect("AAPL");
        create_asset(&mut tx, owner, &input("MSFT", "equity", 100.0))
            .await
            .expect("MSFT");
        create_asset(&mut tx, other, &input("KO", "equity", 100.0))
            .await
            .expect("KO");
        create_trigger(
            &mut tx,
            owner,
            first,
            &super::TriggerInput {
                kind: "thesis_broken".into(),
                description: "Si cambia la tesis".into(),
            },
        )
        .await
        .expect("disparador");

        let removed = clear_assets(&mut tx, owner).await.expect("vaciar");
        assert_eq!(removed, 2);
        assert!(load_assets(&mut tx, owner).await.expect("owner").is_empty());
        assert_eq!(load_assets(&mut tx, other).await.expect("other").len(), 1);
        let (triggers,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM sell_triggers WHERE user_id = $1")
                .bind(owner)
                .fetch_one(&mut *tx)
                .await
                .expect("triggers");
        assert_eq!(triggers, 0);

        tx.rollback().await.expect("rollback");
    }

    async fn make_cash_portfolio(tx: &mut PgConnection, user: i32) -> (Uuid, Uuid, Uuid, Uuid) {
        let mut aapl_input = input("AAPL", "equity", 180.0);
        aapl_input.target_weight_pct = 10.0;
        let aapl = create_asset(tx, user, &aapl_input).await.expect("AAPL");

        let mut msft_input = input("MSFT", "equity", 570.0);
        msft_input.target_weight_pct = 50.0;
        let msft = create_asset(tx, user, &msft_input).await.expect("MSFT");

        let mut bond_input = input("BOND", "fixed_income", 250.0);
        bond_input.target_weight_pct = 25.0;
        bond_input.economy = "argentina".into();
        bond_input.sector = "other".into();
        let bond = create_asset(tx, user, &bond_input).await.expect("BOND");

        let mut cash_input = input("USD", "fixed_income", 0.0);
        cash_input.target_weight_pct = 15.0;
        cash_input.is_cash = Some(true);
        cash_input.economy = "us".into();
        cash_input.sector = "other".into();
        let cash = create_asset(tx, user, &cash_input).await.expect("USD");

        (aapl, msft, bond, cash)
    }

    fn rates() -> Rates {
        Rates {
            mep: 1_000.0,
            ccl: 1_100.0,
            fetched_at: 0,
            stale: false,
        }
    }

    #[actix_web::test]
    async fn rebalanceo_individual_usa_la_liquidez() {
        let mut conn = connect().await;
        let mut tx = conn.begin().await.expect("tx");
        let user = make_user(&mut tx).await;
        let (aapl, _msft, _bond, cash_id) = make_cash_portfolio(&mut tx, user).await;

        let state = execute_rebalance(&mut tx, user, aapl, Ok(rates()))
            .await
            .expect("rebalanceo individual");
        let analysis = state.analysis.expect("analysis");
        let aapl_row = analysis
            .rows
            .iter()
            .find(|row| row.id == aapl)
            .expect("AAPL");
        assert!((aapl_row.value_native - 100.0).abs() < 0.01);
        assert!((aapl_row.current_weight_pct - 10.0).abs() < 0.01);
        assert!((analysis.total_display - 1_000.0).abs() < 0.01);
        assert!(!analysis.orders.iter().any(|order| order.asset_id == aapl));
        let cash = state
            .assets
            .iter()
            .find(|asset| asset.id == cash_id)
            .expect("liquidez");
        assert!((cash.market_value - 80.0).abs() < 0.01);

        tx.rollback().await.expect("rollback");
    }

    #[actix_web::test]
    async fn liquidez_unica_por_usuario() {
        let mut conn = connect().await;
        let mut tx = conn.begin().await.expect("tx");
        let user = make_user(&mut tx).await;
        let mut first = input("USD", "fixed_income", 0.0);
        first.is_cash = Some(true);
        create_asset(&mut tx, user, &first).await.expect("liquidez");

        let mut second = input("USD2", "fixed_income", 0.0);
        second.is_cash = Some(true);
        let err = create_asset(&mut tx, user, &second)
            .await
            .expect_err("segunda liquidez");
        assert!(err.contains("liquidez"), "{err}");

        let other = make_user(&mut tx).await;
        create_asset(&mut tx, other, &second)
            .await
            .expect("liquidez de otro usuario");

        tx.rollback().await.expect("rollback");
    }

    #[test]
    fn validaciones_puras() {
        let mut invalid = input("AAPL", "mixed", 100.0);
        invalid.fixed_income_pct = Some(100.0);
        assert!(super::validate_asset(&invalid).is_err());

        let mut invalid = input("AAPL", "equity", 100.0);
        invalid.currency = "EUR".into();
        assert!(super::validate_asset(&invalid).is_err());

        let mut invalid = input("AAPL", "equity", 100.0);
        invalid.target_weight_pct = 101.0;
        assert!(super::validate_asset(&invalid).is_err());

        assert!(super::validate_asset(&input("AAPL", "equity", 0.0)).is_ok());
        assert!(Uuid::nil().is_nil());
    }
}
