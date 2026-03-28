use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize, Serialize)]
pub struct ApiStatus {
    pub code: String,
    pub text: String,
}

/// Mixed-type summary for the whole portfolio (top "portfel" object).
/// Fields vary between f64 and String in the real API, so Value is used.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortfolioSummary {
    pub bench_name: Option<String>,
    pub tickers_count: Option<Value>,
    pub ticker_clear: Option<String>,
    pub nazwa: Option<String>,
    pub waluta: Option<String>,
    pub wartosc: Option<Value>,
    pub zysk: Option<Value>,
    pub zysk_dzienny: Option<Value>,
    #[serde(rename = "zmianaDzienna")]
    pub zmiana_dzienna: Option<Value>,
    pub close: Option<Value>,
    pub liczba_jednostek: Option<Value>,
    pub udzial: Option<Value>,
    pub zmiana: Option<Value>,
    #[serde(rename = "zmianaW")]
    pub zmiana_w: Option<String>,
    #[serde(rename = "zmiana2W")]
    pub zmiana_2w: Option<String>,
    #[serde(rename = "zmianaM")]
    pub zmiana_m: Option<String>,
    #[serde(rename = "zmiana3M")]
    pub zmiana_3m: Option<String>,
    #[serde(rename = "zmiana6M")]
    pub zmiana_6m: Option<String>,
    #[serde(rename = "zmianaR")]
    pub zmiana_r: Option<String>,
    #[serde(rename = "zmiana3R")]
    pub zmiana_3r: Option<String>,
    #[serde(rename = "zmiana5R")]
    pub zmiana_5r: Option<String>,
    #[serde(rename = "zmianaMdD")]
    pub zmiana_m_d_d: Option<String>,
    #[serde(rename = "zmianaRdD")]
    pub zmiana_r_d_d: Option<String>,
}

/// A single position (ticker) in the portfolio.
/// Numeric fields use Value because the real API returns them as either strings or floats.
#[allow(clippy::struct_field_names)]
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ticker {
    pub ticker_clear: Option<String>,
    pub nazwa: Option<String>,
    pub data: Option<String>,
    pub close: Option<Value>,
    #[serde(rename = "zmianaDzienna")]
    pub zmiana_dzienna: Option<Value>,
    pub liczba_jednostek: Option<Value>,
    pub typ: Option<String>,
    pub typ_org: Option<String>,
    pub wartosc: Option<Value>,
    pub udzial: Option<Value>,
    pub zmiana: Option<Value>,
    pub cena_zakupu: Option<Value>,
    pub zysk: Option<Value>,
    pub konto_inv_name: Option<String>,
    pub sektor: Option<String>,
    pub ryzyko: Option<String>,
    pub portfel_org: Option<String>,
    pub data_inv_start: Option<String>,
    pub okres_inwestycji: Option<Value>,
}

pub type TimeSeries = HashMap<String, String>;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortfolioResponse {
    pub status: ApiStatus,
    pub portfel: Option<PortfolioSummary>,
    /// Keys are numeric string IDs assigned by myfund.pl
    pub tickers: Option<HashMap<String, Ticker>>,
    pub struktura: Option<HashMap<String, Value>>,
    #[allow(dead_code)]
    pub struktura_walory: Option<HashMap<String, Value>>,
    #[serde(rename = "zyskWCzasie")]
    pub zysk_w_czasie: Option<TimeSeries>,
    #[serde(rename = "wartoscWCzasie")]
    pub wartosc_w_czasie: Option<TimeSeries>,
    #[serde(rename = "wkladWCzasie")]
    pub wklad_w_czasie: Option<TimeSeries>,
    #[serde(rename = "benchWCzasie")]
    pub bench_w_czasie: Option<TimeSeries>,
    #[serde(rename = "stopaZwrotuWCzasie")]
    pub stopa_zwrotu_w_czasie: Option<TimeSeries>,
    #[serde(rename = "zmianaDzienna")]
    pub zmiana_dzienna: Option<TimeSeries>,
}

impl PortfolioResponse {
    pub fn is_error(&self) -> bool {
        self.status.code == "1"
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::unnecessary_map_or)]
mod tests {
    use super::*;

    const FIXTURE_OK: &str = include_str!("../tests/fixtures/portfolio_ok.json");
    const FIXTURE_ERR: &str = include_str!("../tests/fixtures/portfolio_error.json");

    fn parse_ok() -> PortfolioResponse {
        serde_json::from_str(FIXTURE_OK).expect("portfolio_ok.json should deserialize")
    }

    fn parse_err() -> PortfolioResponse {
        serde_json::from_str(FIXTURE_ERR).expect("portfolio_error.json should deserialize")
    }

    #[test]
    fn test_deserialize_portfolio_ok() {
        let r = parse_ok();
        assert_eq!(r.status.code, "0");
        assert_eq!(r.status.text, "OK!");
    }

    #[test]
    fn test_deserialize_portfolio_error() {
        let r = parse_err();
        assert_eq!(r.status.code, "1");
        assert!(!r.status.text.is_empty());
    }

    #[test]
    fn test_is_error_false() {
        assert!(!parse_ok().is_error());
    }

    #[test]
    fn test_is_error_true() {
        assert!(parse_err().is_error());
    }

    #[test]
    fn test_portfel_fields() {
        let r = parse_ok();
        let p = r.portfel.as_ref().unwrap();
        assert_eq!(p.wartosc.as_ref().unwrap().as_str().unwrap(), "250000.00");
        // zysk is a float in the fixture
        assert_eq!(p.zysk.as_ref().unwrap().as_f64().unwrap(), 27500.0);
        // zmiana_dzienna is a float (-0.83)
        let zd = p.zmiana_dzienna.as_ref().unwrap().as_f64().unwrap();
        assert!((zd - (-0.83)).abs() < 1e-9);
    }

    #[test]
    fn test_ticker_count() {
        let r = parse_ok();
        let tickers = r.tickers.as_ref().unwrap();
        assert_eq!(tickers.len(), 3);
    }

    #[test]
    fn test_ticker_fields() {
        let r = parse_ok();
        let ticker = r.tickers.as_ref().unwrap().get("1").unwrap();
        assert_eq!(ticker.ticker_clear.as_deref().unwrap(), "NASDAQ_AAPL");
        assert_eq!(ticker.wartosc.as_ref().unwrap().as_str().unwrap(), "85000.00");
        assert_eq!(ticker.zysk.as_ref().unwrap().as_str().unwrap(), "1800.00");
        assert_eq!(ticker.typ.as_deref().unwrap(), "NASDAQ shares");
    }

    #[test]
    fn test_ticker_optional_fields_present() {
        let r = parse_ok();
        let ticker = r.tickers.as_ref().unwrap().get("2").unwrap();
        assert!(ticker.nazwa.is_some());
        assert!(ticker.sektor.is_some());
        assert!(ticker.ryzyko.is_some());
        assert!(ticker.konto_inv_name.is_some());
        assert!(ticker.data_inv_start.is_some());
    }

    #[test]
    fn test_struktura_keys() {
        let r = parse_ok();
        let s = r.struktura.as_ref().unwrap();
        assert!(s.contains_key("NASDAQ shares"));
        assert!(s.contains_key("WSE shares"));
        assert!(s.contains_key("ETFs - international"));
    }

    #[test]
    fn test_timeseries_wartosc_w_czasie() {
        let r = parse_ok();
        let ts = r.wartosc_w_czasie.as_ref().unwrap();
        assert_eq!(ts.len(), 8);
        assert_eq!(ts.get("2024-03-22").unwrap(), "250000.00");
    }

    #[test]
    fn test_timeseries_all_present() {
        let r = parse_ok();
        assert!(r.wartosc_w_czasie.as_ref().map_or(false, |t| !t.is_empty()));
        assert!(r.zysk_w_czasie.as_ref().map_or(false, |t| !t.is_empty()));
        assert!(r.wklad_w_czasie.as_ref().map_or(false, |t| !t.is_empty()));
        assert!(r.bench_w_czasie.as_ref().map_or(false, |t| !t.is_empty()));
        assert!(r.stopa_zwrotu_w_czasie.as_ref().map_or(false, |t| !t.is_empty()));
    }

    #[test]
    fn test_zmiana_dzienna() {
        let r = parse_ok();
        let zd = r.zmiana_dzienna.as_ref().unwrap();
        assert_eq!(zd.len(), 1);
        assert_eq!(zd.get("2024-03-22").unwrap(), "-0.83");
    }
}
