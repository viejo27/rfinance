//! Tasas MEP y CCL desde DolarAPI, con caché en memoria.
//!
//! La app no inventa tasas: si la fuente no responde y no hay una
//! lectura previa, el estado queda sin valuar. Si hay una lectura vieja,
//! se devuelve marcada como `stale` para que el frontend la muestre.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

const DOLAR_API_URL: &str = "https://dolarapi.com/v1/dolares";
const CACHE_TTL_SECONDS: i64 = 600;
const REQUEST_TIMEOUT_SECONDS: u64 = 6;

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Rates {
    /// ARS por USD, dólar MEP ("bolsa").
    pub mep: f64,
    /// ARS por USD, contado con liquidación.
    pub ccl: f64,
    /// Epoch en segundos de la última lectura exitosa.
    pub fetched_at: i64,
    /// La última lectura no se pudo refrescar.
    pub stale: bool,
}

#[derive(Deserialize)]
struct DolarRate {
    casa: String,
    compra: f64,
    venta: f64,
}

/// Caché compartida por todos los handlers. Un único `reqwest::Client`
/// reutiliza conexiones y respeta el timeout de la fuente.
pub struct FxCache {
    client: reqwest::Client,
    inner: RwLock<Option<Rates>>,
}

impl FxCache {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECONDS))
            .user_agent("rfinance/0.1")
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            client,
            inner: RwLock::new(None),
        }
    }

    /// Devuelve las tasas. Con `force` saltea la caché; si la red falla,
    /// la última lectura se devuelve como `stale` en vez de romper la API.
    pub async fn get(&self, force: bool) -> Result<Rates, String> {
        if !force {
            let cached = self.inner.read().await;
            if let Some(rates) = *cached
                && !rates.stale
                && now() - rates.fetched_at < CACHE_TTL_SECONDS
            {
                return Ok(rates);
            }
        } else {
            // El botón de refresco no puede convertirse en un martillo
            // contra DolarAPI: dentro de 10 s se devuelve la última lectura.
            let cached = self.inner.read().await;
            if let Some(rates) = *cached
                && !rates.stale
                && now() - rates.fetched_at < 10
            {
                return Ok(rates);
            }
        }

        match fetch(&self.client).await {
            Ok(mut rates) => {
                rates.fetched_at = now();
                rates.stale = false;
                *self.inner.write().await = Some(rates);
                Ok(rates)
            }
            Err(error) => {
                let cached = *self.inner.read().await;
                match cached {
                    Some(mut rates) => {
                        rates.stale = true;
                        *self.inner.write().await = Some(rates);
                        Ok(rates)
                    }
                    None => Err(error),
                }
            }
        }
    }
}

impl Default for FxCache {
    fn default() -> Self {
        Self::new()
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

async fn fetch(client: &reqwest::Client) -> Result<Rates, String> {
    let response = client
        .get(DOLAR_API_URL)
        .send()
        .await
        .map_err(|e| format!("No se pudo consultar DolarAPI: {e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "DolarAPI respondió con estado {}",
            response.status().as_u16()
        ));
    }
    let body: Vec<DolarRate> = response
        .json()
        .await
        .map_err(|e| format!("Respuesta inválida de DolarAPI: {e}"))?;
    parse(&body)
}

fn parse(body: &[DolarRate]) -> Result<Rates, String> {
    let mep = body
        .iter()
        .find(|r| r.casa == "bolsa")
        .and_then(price)
        .ok_or_else(|| "DolarAPI no trajo el dólar MEP".to_string())?;
    let ccl = body
        .iter()
        .find(|r| r.casa == "contadoconliqui")
        .and_then(price)
        .ok_or_else(|| "DolarAPI no trajo el contado con liquidación".to_string())?;
    Ok(Rates {
        mep,
        ccl,
        fetched_at: 0,
        stale: false,
    })
}

fn price(rate: &DolarRate) -> Option<f64> {
    [rate.venta, rate.compra]
        .into_iter()
        .find(|value| value.is_finite() && *value > 0.0)
}

#[cfg(test)]
mod tests {
    use super::{DolarRate, parse, price};

    fn rate(casa: &str, compra: f64, venta: f64) -> DolarRate {
        DolarRate {
            casa: casa.to_string(),
            compra,
            venta,
        }
    }

    #[test]
    fn toma_bolsa_y_ccl() {
        let body = vec![
            rate("oficial", 1_550.0, 1_600.0),
            rate("bolsa", 1_682.6, 1_743.1),
            rate("contadoconliqui", 1_801.4, 1_806.2),
        ];
        let rates = parse(&body).expect("tasas");
        assert!((rates.mep - 1_743.1).abs() < 0.001);
        assert!((rates.ccl - 1_806.2).abs() < 0.001);
    }

    #[test]
    fn usa_compra_si_venta_es_cero() {
        assert_eq!(price(&rate("bolsa", 1_700.0, 0.0)), Some(1_700.0));
        assert_eq!(price(&rate("bolsa", 0.0, 0.0)), None);
    }

    #[test]
    fn falla_si_falta_una_tasa() {
        let body = vec![rate("bolsa", 1_682.6, 1_743.1)];
        assert!(parse(&body).is_err());
    }
}
