//! Modelo puro de la cartera: enums, criterios, presets y activos.
//!
//! Nada de este módulo toca la base ni la red: el motor de métricas y los
//! tests trabajan con estas estructuras.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Opción para poblar un `<select>` en el frontend.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct OptionItem {
    pub id: &'static str,
    pub label: &'static str,
}

/// Perfil de riesgo: preset de objetivos y límites, no una jaula.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskProfile {
    Conservative,
    Moderate,
    Aggressive,
}

impl RiskProfile {
    pub const ALL: [RiskProfile; 3] = [
        RiskProfile::Conservative,
        RiskProfile::Moderate,
        RiskProfile::Aggressive,
    ];

    pub fn id(self) -> &'static str {
        match self {
            RiskProfile::Conservative => "conservative",
            RiskProfile::Moderate => "moderate",
            RiskProfile::Aggressive => "aggressive",
        }
    }

    pub fn from_id(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "conservative" | "conservador" => Some(RiskProfile::Conservative),
            "moderate" | "moderado" => Some(RiskProfile::Moderate),
            "aggressive" | "agresivo" => Some(RiskProfile::Aggressive),
            _ => None,
        }
    }

    pub fn preset(self) -> ProfilePreset {
        match self {
            RiskProfile::Conservative => ProfilePreset {
                id: "conservative",
                label: "Conservador",
                target_fixed_income_pct: 70.0,
                target_equity_pct: 30.0,
                rebalance_band_pp: 3.0,
                min_assets: 6,
                max_assets: 10,
                max_asset_pct: 12.0,
                max_economy_pct: 60.0,
                max_cluster_pct: 25.0,
            },
            RiskProfile::Moderate => ProfilePreset {
                id: "moderate",
                label: "Moderado",
                target_fixed_income_pct: 55.0,
                target_equity_pct: 45.0,
                rebalance_band_pp: 4.0,
                min_assets: 7,
                max_assets: 12,
                max_asset_pct: 15.0,
                max_economy_pct: 65.0,
                max_cluster_pct: 30.0,
            },
            RiskProfile::Aggressive => ProfilePreset {
                id: "aggressive",
                label: "Agresivo",
                target_fixed_income_pct: 40.0,
                target_equity_pct: 60.0,
                rebalance_band_pp: 5.0,
                min_assets: 8,
                max_assets: 13,
                max_asset_pct: 15.0,
                max_economy_pct: 70.0,
                max_cluster_pct: 35.0,
            },
        }
    }
}

/// Preset completo de un perfil. El usuario puede editar cada criterio;
/// el perfil queda como punto de partida y la UI avisa "personalizado".
#[derive(Clone, Copy, Debug, Serialize)]
pub struct ProfilePreset {
    pub id: &'static str,
    pub label: &'static str,
    pub target_fixed_income_pct: f64,
    pub target_equity_pct: f64,
    pub rebalance_band_pp: f64,
    pub min_assets: i16,
    pub max_assets: i16,
    pub max_asset_pct: f64,
    pub max_economy_pct: f64,
    pub max_cluster_pct: f64,
}

pub fn profile_presets() -> Vec<ProfilePreset> {
    RiskProfile::ALL.iter().map(|p| p.preset()).collect()
}

/// Moneda en la que se muestra la cartera. Las dos primeras son pesos;
/// las dos últimas, dólares valuados a MEP o CCL.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Valuation {
    ArsMep,
    ArsCcl,
    UsdMep,
    UsdCcl,
}

impl Valuation {
    pub const ALL: [Valuation; 4] = [
        Valuation::ArsMep,
        Valuation::ArsCcl,
        Valuation::UsdMep,
        Valuation::UsdCcl,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Valuation::ArsMep => "ars_mep",
            Valuation::ArsCcl => "ars_ccl",
            Valuation::UsdMep => "usd_mep",
            Valuation::UsdCcl => "usd_ccl",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Valuation::ArsMep => "ARS · MEP",
            Valuation::ArsCcl => "ARS · CCL",
            Valuation::UsdMep => "USD · MEP",
            Valuation::UsdCcl => "USD · CCL",
        }
    }

    pub fn from_id(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "ars_mep" => Some(Valuation::ArsMep),
            "ars_ccl" => Some(Valuation::ArsCcl),
            "usd_mep" => Some(Valuation::UsdMep),
            "usd_ccl" => Some(Valuation::UsdCcl),
            _ => None,
        }
    }

    pub fn display_currency(self) -> &'static str {
        match self {
            Valuation::ArsMep | Valuation::ArsCcl => "ARS",
            Valuation::UsdMep | Valuation::UsdCcl => "USD",
        }
    }

    pub fn rate_kind(self) -> RateKind {
        match self {
            Valuation::ArsMep | Valuation::UsdMep => RateKind::Mep,
            Valuation::ArsCcl | Valuation::UsdCcl => RateKind::Ccl,
        }
    }

    pub fn options() -> Vec<OptionItem> {
        Valuation::ALL
            .iter()
            .map(|v| OptionItem {
                id: v.id(),
                label: v.label(),
            })
            .collect()
    }
}

/// Tasa ARS por USD que usa cada vista.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RateKind {
    Mep,
    Ccl,
}

impl RateKind {
    pub fn label(self) -> &'static str {
        match self {
            RateKind::Mep => "MEP",
            RateKind::Ccl => "CCL",
        }
    }
}

/// Criterios de la estrategia + perfil elegido.
#[derive(Clone, Debug, Serialize)]
pub struct Settings {
    pub public_id: Uuid,
    pub risk_profile: RiskProfile,
    pub valuation: Valuation,
    pub target_fixed_income_pct: f64,
    pub target_equity_pct: f64,
    pub rebalance_band_pp: f64,
    pub min_assets: i16,
    pub max_assets: i16,
    pub max_asset_pct: f64,
    pub max_economy_pct: f64,
    pub max_cluster_pct: f64,
}

impl Settings {
    /// Criterios del perfil, listos para persistir. El public_id es un
    /// placeholder: lo real lo pone la base al crear la fila.
    pub fn from_profile(profile: RiskProfile) -> Settings {
        let p = profile.preset();
        Settings {
            public_id: Uuid::nil(),
            risk_profile: profile,
            valuation: Valuation::UsdCcl,
            target_fixed_income_pct: p.target_fixed_income_pct,
            target_equity_pct: p.target_equity_pct,
            rebalance_band_pp: p.rebalance_band_pp,
            min_assets: p.min_assets,
            max_assets: p.max_assets,
            max_asset_pct: p.max_asset_pct,
            max_economy_pct: p.max_economy_pct,
            max_cluster_pct: p.max_cluster_pct,
        }
    }

    /// ¿Los valores se despegaron del preset elegido?
    pub fn is_custom(&self) -> bool {
        let p = self.risk_profile.preset();
        let eps = 0.011;
        (self.target_fixed_income_pct - p.target_fixed_income_pct).abs() > eps
            || (self.target_equity_pct - p.target_equity_pct).abs() > eps
            || (self.rebalance_band_pp - p.rebalance_band_pp).abs() > eps
            || self.min_assets != p.min_assets
            || self.max_assets != p.max_assets
            || (self.max_asset_pct - p.max_asset_pct).abs() > eps
            || (self.max_economy_pct - p.max_economy_pct).abs() > eps
            || (self.max_cluster_pct - p.max_cluster_pct).abs() > eps
    }
}

/// Tipo de activo. `Mixed` se prorratea entre renta fija y variable
/// según `fixed_income_pct`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetClass {
    FixedIncome,
    Equity,
    Mixed,
}

impl AssetClass {
    pub const ALL: [AssetClass; 3] = [
        AssetClass::FixedIncome,
        AssetClass::Equity,
        AssetClass::Mixed,
    ];

    pub fn id(self) -> &'static str {
        match self {
            AssetClass::FixedIncome => "fixed_income",
            AssetClass::Equity => "equity",
            AssetClass::Mixed => "mixed",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            AssetClass::FixedIncome => "Renta fija",
            AssetClass::Equity => "Renta variable",
            AssetClass::Mixed => "Mixta",
        }
    }

    pub fn css(self) -> &'static str {
        match self {
            AssetClass::FixedIncome => "rf",
            AssetClass::Equity => "rv",
            AssetClass::Mixed => "mix",
        }
    }

    pub fn from_id(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "fixed_income" => Some(AssetClass::FixedIncome),
            "equity" => Some(AssetClass::Equity),
            "mixed" => Some(AssetClass::Mixed),
            _ => None,
        }
    }

    /// Porcentaje de renta fija que corresponde a la clase.
    pub fn default_fixed_income_pct(self) -> f64 {
        match self {
            AssetClass::FixedIncome => 100.0,
            AssetClass::Equity => 0.0,
            AssetClass::Mixed => 50.0,
        }
    }

    pub fn options() -> Vec<OptionItem> {
        AssetClass::ALL
            .iter()
            .map(|c| OptionItem {
                id: c.id(),
                label: c.label(),
            })
            .collect()
    }
}

/// Economía a la que se está expuesto. Es la del subyacente, no la del
/// mercado donde cotiza el instrumento.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Economy {
    Us,
    Europe,
    Emerging,
    Argentina,
    Global,
}

impl Economy {
    pub const ALL: [Economy; 5] = [
        Economy::Us,
        Economy::Europe,
        Economy::Emerging,
        Economy::Argentina,
        Economy::Global,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Economy::Us => "us",
            Economy::Europe => "europe",
            Economy::Emerging => "emerging",
            Economy::Argentina => "argentina",
            Economy::Global => "global",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Economy::Us => "Estados Unidos",
            Economy::Europe => "Europa",
            Economy::Emerging => "Mercados emergentes",
            Economy::Argentina => "Argentina",
            Economy::Global => "Global",
        }
    }

    pub fn from_id(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "us" | "eeuu" => Some(Economy::Us),
            "europe" | "europa" => Some(Economy::Europe),
            "emerging" | "emergentes" => Some(Economy::Emerging),
            "argentina" | "ar" => Some(Economy::Argentina),
            "global" => Some(Economy::Global),
            _ => None,
        }
    }

    pub fn options() -> Vec<OptionItem> {
        Economy::ALL
            .iter()
            .map(|e| OptionItem {
                id: e.id(),
                label: e.label(),
            })
            .collect()
    }
}

/// Naturaleza del sector dentro del ciclo económico.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Nature {
    Cyclical,
    Defensive,
    Mixed,
}

impl Nature {
    pub fn id(self) -> &'static str {
        match self {
            Nature::Cyclical => "cyclical",
            Nature::Defensive => "defensive",
            Nature::Mixed => "mixed",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Nature::Cyclical => "Cíclico",
            Nature::Defensive => "Defensivo",
            Nature::Mixed => "Mixto",
        }
    }
}

/// Sectores fijos. No hay sectores libres: la composición se mide con
/// este vocabulario cerrado.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sector {
    Technology,
    CommunicationServices,
    ConsumerDiscretionary,
    ConsumerStaples,
    Financials,
    Industrials,
    Materials,
    Energy,
    HealthCare,
    Utilities,
    RealEstate,
    Other,
}

impl Sector {
    pub const ALL: [Sector; 12] = [
        Sector::Technology,
        Sector::CommunicationServices,
        Sector::ConsumerDiscretionary,
        Sector::ConsumerStaples,
        Sector::Financials,
        Sector::Industrials,
        Sector::Materials,
        Sector::Energy,
        Sector::HealthCare,
        Sector::Utilities,
        Sector::RealEstate,
        Sector::Other,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Sector::Technology => "technology",
            Sector::CommunicationServices => "communication_services",
            Sector::ConsumerDiscretionary => "consumer_discretionary",
            Sector::ConsumerStaples => "consumer_staples",
            Sector::Financials => "financials",
            Sector::Industrials => "industrials",
            Sector::Materials => "materials",
            Sector::Energy => "energy",
            Sector::HealthCare => "health_care",
            Sector::Utilities => "utilities",
            Sector::RealEstate => "real_estate",
            Sector::Other => "other",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Sector::Technology => "Tecnología",
            Sector::CommunicationServices => "Comunicaciones",
            Sector::ConsumerDiscretionary => "Consumo discrecional",
            Sector::ConsumerStaples => "Consumo masivo",
            Sector::Financials => "Financiero",
            Sector::Industrials => "Industrial",
            Sector::Materials => "Materiales",
            Sector::Energy => "Energía",
            Sector::HealthCare => "Salud",
            Sector::Utilities => "Servicios públicos",
            Sector::RealEstate => "Inmobiliario",
            Sector::Other => "Otros",
        }
    }

    pub fn nature(self) -> Nature {
        match self {
            Sector::Technology
            | Sector::ConsumerDiscretionary
            | Sector::Financials
            | Sector::Industrials
            | Sector::Materials
            | Sector::RealEstate => Nature::Cyclical,
            Sector::HealthCare | Sector::ConsumerStaples | Sector::Utilities => Nature::Defensive,
            Sector::CommunicationServices | Sector::Energy | Sector::Other => Nature::Mixed,
        }
    }

    pub fn from_id(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "technology" | "tecnologia" => Some(Sector::Technology),
            "communication_services" | "comunicaciones" => Some(Sector::CommunicationServices),
            "consumer_discretionary" | "consumo_discrecional" => {
                Some(Sector::ConsumerDiscretionary)
            }
            "consumer_staples" | "consumo_masivo" => Some(Sector::ConsumerStaples),
            "financials" | "financiero" => Some(Sector::Financials),
            "industrials" | "industrial" => Some(Sector::Industrials),
            "materials" | "materiales" => Some(Sector::Materials),
            "energy" | "energia" => Some(Sector::Energy),
            "health_care" | "salud" => Some(Sector::HealthCare),
            "utilities" | "servicios_publicos" => Some(Sector::Utilities),
            "real_estate" | "inmobiliario" => Some(Sector::RealEstate),
            "other" | "otros" => Some(Sector::Other),
            _ => None,
        }
    }

    pub fn options() -> Vec<OptionItem> {
        Sector::ALL
            .iter()
            .map(|s| OptionItem {
                id: s.id(),
                label: s.label(),
            })
            .collect()
    }
}

/// Motivo de venta. Los tres primeros son de venta total; `Rebalanceo`
/// es el mecanismo parcial.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerKind {
    ThesisBroken,
    Fundamentals,
    TargetMet,
    Rebalance,
    Other,
}

impl TriggerKind {
    pub const ALL: [TriggerKind; 5] = [
        TriggerKind::ThesisBroken,
        TriggerKind::Fundamentals,
        TriggerKind::TargetMet,
        TriggerKind::Rebalance,
        TriggerKind::Other,
    ];

    pub fn id(self) -> &'static str {
        match self {
            TriggerKind::ThesisBroken => "thesis_broken",
            TriggerKind::Fundamentals => "fundamentals",
            TriggerKind::TargetMet => "target_met",
            TriggerKind::Rebalance => "rebalance",
            TriggerKind::Other => "other",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            TriggerKind::ThesisBroken => "Tesis rota",
            TriggerKind::Fundamentals => "Cambian los fundamentos",
            TriggerKind::TargetMet => "Objetivo cumplido",
            TriggerKind::Rebalance => "Rebalanceo",
            TriggerKind::Other => "Otro",
        }
    }

    pub fn from_id(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "thesis_broken" => Some(TriggerKind::ThesisBroken),
            "fundamentals" => Some(TriggerKind::Fundamentals),
            "target_met" => Some(TriggerKind::TargetMet),
            "rebalance" => Some(TriggerKind::Rebalance),
            "other" => Some(TriggerKind::Other),
            _ => None,
        }
    }

    pub fn options() -> Vec<OptionItem> {
        TriggerKind::ALL
            .iter()
            .map(|k| OptionItem {
                id: k.id(),
                label: k.label(),
            })
            .collect()
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct SellTrigger {
    pub id: Uuid,
    pub kind: TriggerKind,
    pub description: String,
    pub is_met: bool,
    pub is_resolved: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Asset {
    pub id: Uuid,
    pub ticker: Option<String>,
    pub name: String,
    pub asset_class: AssetClass,
    pub fixed_income_pct: f64,
    pub economy: Economy,
    pub sector: Sector,
    pub market_value: f64,
    pub currency: String,
    pub target_weight_pct: f64,
    pub thesis: String,
    pub objective: String,
    pub source: String,
    pub is_cash: bool,
    pub market_symbol: Option<String>,
    pub quantity: Option<f64>,
    pub last_price: Option<f64>,
    pub price_currency: Option<String>,
    pub priced_at: Option<String>,
    pub triggers: Vec<SellTrigger>,
}

impl Asset {
    pub fn display(&self) -> &str {
        self.ticker
            .as_deref()
            .filter(|t| !t.is_empty())
            .unwrap_or(&self.name)
    }

    pub fn has_thesis_and_objective(&self) -> bool {
        !self.thesis.trim().is_empty() && !self.objective.trim().is_empty()
    }
}
