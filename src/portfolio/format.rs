//! Formato es-AR para los textos que arma el motor de métricas.
//! El frontend formatea tablas y montos con `Intl`; estos helpers son
//! solo para las explicaciones de los chequeos.

/// Porcentaje con un decimal y coma: `42,0 %`. El espacio antes del
/// símbolo es duro para que el número no se corte solo al final de línea.
pub fn pct(value: f64) -> String {
    format!("{:.1}", value).replace('.', ",").replace('-', "−") + "\u{00A0}%"
}

/// Desvío en puntos porcentuales con signo: `+4,2 pp` / `−3,0 pp`.
pub fn pp(value: f64) -> String {
    let sign = if value > 0.0 { "+" } else { "" };
    format!("{sign}{:.1}\u{00A0}pp", value)
        .replace('.', ",")
        .replace('-', "−")
}

#[cfg(test)]
mod tests {
    use super::{pct, pp};

    #[test]
    fn porcentajes_y_puntos() {
        assert_eq!(pct(42.04), "42,0\u{00A0}%");
        assert_eq!(pct(-1.25), "−1,2\u{00A0}%");
        assert_eq!(pp(4.2), "+4,2\u{00A0}pp");
        assert_eq!(pp(-3.0), "−3,0\u{00A0}pp");
    }
}
