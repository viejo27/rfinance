use std::time::{SystemTime, UNIX_EPOCH};

use actix_session::Session;
use actix_web::{HttpResponse, Responder, get};
use askama::Template;

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate<'a> {
    email: &'a str,
    corte: String,
    hoy_iso: String,
    total_ars: &'a str,
    total_usd: &'a str,
    neto_mes: &'a str,
    cuentas: Vec<Cuenta>,
    movimientos: Vec<Movimiento>,
    categorias: Vec<&'static str>,
    presupuestos: Vec<Presupuesto>,
    carteles: Vec<Cartel>,
}

struct Cuenta {
    nombre: &'static str,
    moneda: &'static str,
    saldo: String,
    negativa: bool,
}

struct Movimiento {
    fecha: &'static str,
    iso_fecha: &'static str,
    categoria: &'static str,
    cuenta: &'static str,
    concepto: &'static str,
    monto: String,
    valor: f64,
    moneda: &'static str,
    egreso: bool,
}

struct Presupuesto {
    nombre: &'static str,
    limite: String,
    usado: String,
    pct: i32,
    excedido: bool,
}

struct MesBarra {
    mes: &'static str,
    x_ing: i32,
    y_ing: i32,
    h_ing: i32,
    x_gas: i32,
    y_gas: i32,
    h_gas: i32,
    ing_txt: String,
    gas_txt: String,
}

struct Cartel {
    moneda: &'static str,
    ing_total: String,
    gas_total: String,
    barras: Vec<MesBarra>,
}

/// Formato es-AR: punto como separador de miles, coma decimal.
fn formatear(valor: f64, decimales: usize) -> String {
    let negativo = valor < 0.0;
    let factor_f = 10f64.powi(decimales as i32);
    let factor = 10i64.pow(decimales as u32);
    let redondeado = (valor.abs() * factor_f).round() as i64;
    let enteros = redondeado / factor;
    let fraccion = redondeado % factor;

    let digitos = enteros.to_string();
    let mut grupos = String::new();
    let len = digitos.len();
    for (i, ch) in digitos.chars().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            grupos.push('.');
        }
        grupos.push(ch);
    }

    let cuerpo = if decimales > 0 {
        format!("{grupos},{fraccion:0>decimales$}")
    } else {
        grupos
    };
    if negativo {
        format!("-{cuerpo}")
    } else {
        cuerpo
    }
}

/// Altura de barra sobre una escala fija de 190 px.
fn escalar(valor: f64, maximo: f64) -> i32 {
    ((valor / maximo) * 190.0).round() as i32
}

/// Convierte días desde el epoch Unix a (año, mes, día).
/// Algoritmo civil de Howard Hinnant, sin dependencias.
fn fecha_civil(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
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
fn hoy() -> (String, String) {
    let dias = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() / 86_400)
        .unwrap_or(20_740) as i64;
    let (y, m, d) = fecha_civil(dias);
    (
        format!("{y:04}-{m:02}-{d:02}"),
        format!("{d:02}·{m:02}·{y}"),
    )
}

fn mv(
    fecha: &'static str,
    iso_fecha: &'static str,
    categoria: &'static str,
    cuenta: &'static str,
    concepto: &'static str,
    valor: f64,
    moneda: &'static str,
) -> Movimiento {
    let decimales = if moneda == "USD" { 2 } else { 0 };
    let signo = if valor < 0.0 { "−" } else { "" };
    Movimiento {
        fecha,
        iso_fecha,
        categoria,
        cuenta,
        concepto,
        monto: format!("{}{} {}", signo, formatear(valor.abs(), decimales), moneda),
        valor,
        moneda,
        egreso: valor < 0.0,
    }
}

fn pres(
    nombre: &'static str,
    limite: f64,
    usado: f64,
    decimales: usize,
    moneda: &'static str,
) -> Presupuesto {
    let pct = ((usado / limite) * 100.0).round() as i32;
    Presupuesto {
        nombre,
        limite: format!("{} {}", formatear(limite, decimales), moneda),
        usado: format!("{} {}", formatear(usado, decimales), moneda),
        pct,
        excedido: pct > 100,
    }
}

fn cartel(moneda: &'static str, serie: &[(&'static str, f64, f64); 6], maximo: f64) -> Cartel {
    let barras = serie
        .iter()
        .enumerate()
        .map(|(i, (mes, ing, gas))| {
            let x: i32 = 20 + (i as i32) * 103;
            let h_ing = escalar(*ing, maximo);
            let h_gas = escalar(*gas, maximo);
            MesBarra {
                mes,
                x_ing: x,
                y_ing: 208 - h_ing,
                h_ing,
                x_gas: x + 32,
                y_gas: 208 - h_gas,
                h_gas,
                ing_txt: formatear(*ing, 0),
                gas_txt: formatear(*gas, 0),
            }
        })
        .collect();
    let ing_total = serie.iter().map(|(_, i, _)| i).sum::<f64>();
    let gas_total = serie.iter().map(|(_, _, g)| g).sum::<f64>();
    Cartel {
        moneda,
        ing_total: formatear(ing_total, 0),
        gas_total: formatear(gas_total, 0),
        barras,
    }
}

#[get("/")]
async fn index(session: Session) -> impl Responder {
    let email = session
        .get::<String>("user_email")
        .unwrap_or(None)
        .unwrap_or_default();

    // ---- Datos de ejemplo: el backend financiero todavía no existe ----

    let cuentas = vec![
        Cuenta {
            nombre: "Efectivo",
            moneda: "ARS",
            saldo: formatear(84_520.0, 0),
            negativa: false,
        },
        Cuenta {
            nombre: "Banco Nación",
            moneda: "ARS",
            saldo: formatear(1_312_470.0, 0),
            negativa: false,
        },
        Cuenta {
            nombre: "Visa ARS",
            moneda: "ARS",
            saldo: formatear(-398_215.0, 0),
            negativa: true,
        },
        Cuenta {
            nombre: "Reserva USD",
            moneda: "USD",
            saldo: formatear(5_050.0, 0),
            negativa: false,
        },
    ];

    let categorias = vec![
        "Alquiler",
        "Alimentación",
        "Transporte",
        "Servicios",
        "Suscripciones",
        "Salud",
        "Ocio",
        "Ingresos",
    ];

    let movimientos = vec![
        mv(
            "12 OCT",
            "2026-10-12",
            "Alquiler",
            "Banco Nación",
            "Alquiler mes de octubre",
            -320_000.0,
            "ARS",
        ),
        mv(
            "12 OCT",
            "2026-10-12",
            "Alimentación",
            "Visa ARS",
            "Supermercado La Anónima",
            -46_800.0,
            "ARS",
        ),
        mv(
            "11 OCT",
            "2026-10-11",
            "Transporte",
            "Efectivo",
            "Nafta Shell, tanque lleno",
            -22_400.0,
            "ARS",
        ),
        mv(
            "11 OCT",
            "2026-10-11",
            "Transporte",
            "Efectivo",
            "Tarifa de subte (20 viajes)",
            -3_150.0,
            "ARS",
        ),
        mv(
            "10 OCT",
            "2026-10-10",
            "Suscripciones",
            "Reserva USD",
            "Netflix, plan familiar",
            -14.99,
            "USD",
        ),
        mv(
            "10 OCT",
            "2026-10-10",
            "Salud",
            "Banco Nación",
            "Prepaga Swiss Medical",
            -38_900.0,
            "ARS",
        ),
        mv(
            "09 OCT",
            "2026-10-09",
            "Alimentación",
            "Efectivo",
            "Verdulería Doña Rosa",
            -8_750.0,
            "ARS",
        ),
        mv(
            "08 OCT",
            "2026-10-08",
            "Ingresos",
            "Banco Nación",
            "Sueldo de octubre",
            950_000.0,
            "ARS",
        ),
        mv(
            "07 OCT",
            "2026-10-07",
            "Ingresos",
            "Reserva USD",
            "Honorario freelance (Denver)",
            400.0,
            "USD",
        ),
        mv(
            "06 OCT",
            "2026-10-06",
            "Servicios",
            "Banco Nación",
            "Luz + ABL + gas",
            -27_500.0,
            "ARS",
        ),
        mv(
            "05 OCT",
            "2026-10-05",
            "Ocio",
            "Visa ARS",
            "Cine y panchos, domingo",
            -19_300.0,
            "ARS",
        ),
        mv(
            "04 OCT",
            "2026-10-04",
            "Suscripciones",
            "Visa ARS",
            "Spotify familiar",
            -2_850.0,
            "ARS",
        ),
        mv(
            "03 OCT",
            "2026-10-03",
            "Ingresos",
            "Efectivo",
            "Reparación de PC, contado",
            45_000.0,
            "ARS",
        ),
        mv(
            "02 OCT",
            "2026-10-02",
            "Alimentación",
            "Efectivo",
            "Almuerzo en el trabajo",
            -6_900.0,
            "ARS",
        ),
    ];

    let presupuestos = vec![
        pres("Alquiler", 320_000.0, 320_000.0, 0, "ARS"),
        pres("Alimentación", 120_000.0, 138_600.0, 0, "ARS"),
        pres("Transporte", 60_000.0, 44_800.0, 0, "ARS"),
        pres("Servicios", 90_000.0, 55_000.0, 0, "ARS"),
        pres("Suscripciones", 30.0, 16.49, 2, "USD"),
    ];

    let serie_ars: [(&'static str, f64, f64); 6] = [
        ("MAY", 780_000.0, 640_000.0),
        ("JUN", 952_000.0, 701_500.0),
        ("JUL", 868_000.0, 559_200.0),
        ("AGO", 1_340_000.0, 812_300.0),
        ("SEP", 986_000.0, 664_800.0),
        ("OCT", 1_390_000.0, 978_600.0),
    ];
    let serie_usd: [(&'static str, f64, f64); 6] = [
        ("MAY", 600.0, 420.0),
        ("JUN", 850.0, 510.0),
        ("JUL", 400.0, 380.0),
        ("AGO", 1_240.0, 690.0),
        ("SEP", 780.0, 455.0),
        ("OCT", 1_150.0, 742.0),
    ];

    let carteles = vec![
        cartel("ARS", &serie_ars, 1_400_000.0),
        cartel("USD", &serie_usd, 1_300.0),
    ];

    let neto_mes = format!("+{} ARS", formatear(411_400.0, 0));

    let (hoy_iso, corte) = hoy();

    let plantilla = IndexTemplate {
        email: &email,
        corte,
        hoy_iso,
        total_ars: &formatear(998_775.0, 0),
        total_usd: &formatear(5_050.0, 0),
        neto_mes: &neto_mes,
        cuentas,
        movimientos,
        categorias,
        presupuestos,
        carteles,
    };

    match plantilla.render() {
        Ok(html) => HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(html),
        Err(_) => HttpResponse::InternalServerError().body("Error al renderizar la plantilla"),
    }
}

#[cfg(test)]
mod tests {
    use super::{fecha_civil, formatear};

    #[test]
    fn formatea_miles_es_ar() {
        assert_eq!(formatear(1_312_470.0, 0), "1.312.470");
        assert_eq!(formatear(-398_215.0, 0), "-398.215");
        assert_eq!(formatear(14.99, 2), "14,99");
        assert_eq!(formatear(5_050.0, 0), "5.050");
        assert_eq!(formatear(0.0, 0), "0");
    }

    #[test]
    fn redondea_sin_arrastre_visual() {
        // 999,999 con 2 decimales debe mostrarse 1.000,00
        assert_eq!(formatear(999.999, 2), "1.000,00");
    }

    #[test]
    fn epoch_a_fecha_civil() {
        assert_eq!(fecha_civil(0), (1970, 1, 1));
        assert_eq!(fecha_civil(-1), (1969, 12, 31));
        assert_eq!(fecha_civil(20_740), (2026, 10, 14));
        assert_eq!(fecha_civil(10_957), (2000, 1, 1));
    }
}
