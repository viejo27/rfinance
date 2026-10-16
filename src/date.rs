use std::time::{SystemTime, UNIX_EPOCH};

/// Convierte días desde el epoch Unix a (año, mes, día).
/// Algoritmo civil de Howard Hinnant, sin dependencias.
pub fn civil_date(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Fecha local del servidor como (ISO, corte legible es-AR).
pub fn today() -> (String, String) {
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() / 86_400)
        .unwrap_or(20_740) as i64;
    let (y, m, d) = civil_date(days);
    (
        format!("{y:04}-{m:02}-{d:02}"),
        format!("{d:02}·{m:02}·{y}"),
    )
}

/// ISO del día de hoy, listo para usar en handlers.
pub fn today_iso() -> String {
    today().0
}

#[cfg(test)]
mod tests {
    use super::civil_date;

    #[test]
    fn epoch_to_civil_date() {
        assert_eq!(civil_date(0), (1970, 1, 1));
        assert_eq!(civil_date(-1), (1969, 12, 31));
        assert_eq!(civil_date(20_740), (2026, 10, 14));
        assert_eq!(civil_date(10_957), (2000, 1, 1));
    }
}
