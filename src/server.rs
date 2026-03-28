// Production code uses .expect() only for infallible operations (json!() macro output,
// BTreeMap<&str,&str> serialization). Suppress the pedantic lint at file level.
#![allow(clippy::expect_used)]

use std::collections::BTreeMap;
use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::myfund::MyfundClient;

pub struct MyfundServer {
    client: Arc<MyfundClient>,
    portfolios: Vec<String>,
    tool_router: ToolRouter<MyfundServer>,
}

impl MyfundServer {
    pub fn new(client: MyfundClient, portfolios: Vec<String>) -> Self {
        Self {
            client: Arc::new(client),
            portfolios,
            tool_router: Self::tool_router(),
        }
    }
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum SeriesName {
    WartoscWCzasie,
    ZyskWCzasie,
    WkladWCzasie,
    BenchWCzasie,
    StopaZwrotuWCzasie,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetPortfolioSummaryParams {
    /// Name of the portfolio on myfund.pl (case-sensitive).
    pub name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetPortfolioTimeseriesParams {
    /// Name of the portfolio on myfund.pl (case-sensitive).
    pub name: String,
    /// Which time-series to fetch.
    pub series: SeriesName,
    /// Optional start date filter (YYYY-MM-DD, inclusive).
    pub from: Option<String>,
    /// Optional end date filter (YYYY-MM-DD, inclusive).
    pub to: Option<String>,
}

#[tool_router]
impl MyfundServer {
    #[tool(description = "List all configured myfund.pl portfolio names.")]
    async fn list_portfolios(&self) -> String {
        if self.portfolios.is_empty() {
            "No portfolios configured. Set the MYFUND_PORTFOLIOS environment variable \
             (comma-separated portfolio names)."
                .to_string()
        } else {
            self.portfolios.join(", ")
        }
    }

    #[tool(
        description = "Fetch portfolio summary: totals, key ticker fields (symbol, name, unit count), asset structure breakdown, and today's daily change. Does not include time-series data."
    )]
    async fn get_portfolio_summary(
        &self,
        Parameters(params): Parameters<GetPortfolioSummaryParams>,
    ) -> String {
        match self.client.fetch_portfolio(&params.name).await {
            Err(e) => format!("Error: {e}"),
            Ok(r) => {
                let slim_tickers: Option<std::collections::HashMap<String, serde_json::Value>> =
                    r.tickers.map(|tickers| {
                        tickers
                            .into_iter()
                            .map(|(id, t)| {
                                let slim = json!({
                                    "tickerClear": t.ticker_clear,
                                    "nazwa": t.nazwa,
                                    "liczbaJednostek": t.liczba_jednostek,
                                });
                                (id, slim)
                            })
                            .collect()
                    });
                let summary = json!({
                    "portfel": r.portfel,
                    "tickers": slim_tickers,
                    "struktura": r.struktura,
                    "zmianaDzienna": r.zmiana_dzienna,
                });
                serde_json::to_string_pretty(&summary)
                    .expect("json!() macro output is always serializable")
            }
        }
    }

    #[tool(
        description = "Fetch a time-series for a portfolio. Valid series names: wartoscWCzasie (portfolio value), zykWCzasie (profit), wkladWCzasie (contributions), benchWCzasie (benchmark return), stopaZwrotuWCzasie (rate of return). Optionally filter by from/to dates (YYYY-MM-DD)."
    )]
    async fn get_portfolio_timeseries(
        &self,
        Parameters(params): Parameters<GetPortfolioTimeseriesParams>,
    ) -> String {
        let response = match self.client.fetch_portfolio(&params.name).await {
            Err(e) => return format!("Error: {e}"),
            Ok(r) => r,
        };

        let series = match params.series {
            SeriesName::WartoscWCzasie => response.wartosc_w_czasie.as_ref(),
            SeriesName::ZyskWCzasie => response.zysk_w_czasie.as_ref(),
            SeriesName::WkladWCzasie => response.wklad_w_czasie.as_ref(),
            SeriesName::BenchWCzasie => response.bench_w_czasie.as_ref(),
            SeriesName::StopaZwrotuWCzasie => response.stopa_zwrotu_w_czasie.as_ref(),
        };

                let Some(series) = series else {
            return "No data available for this series.".to_string();
        };

        let map: BTreeMap<&str, &Value> = series
            .iter()
            .filter(|(date, _)| {
                params.from.as_ref().is_none_or(|f| date.as_str() >= f.as_str())
                    && params.to.as_ref().is_none_or(|t| date.as_str() <= t.as_str())
            })
            .map(|(d, v)| (d.as_str(), v))
            .collect();

        serde_json::to_string_pretty(&map)
            .expect("BTreeMap<&str, &str> serialization is infallible")
    }
}

#[tool_handler]
impl ServerHandler for MyfundServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::LATEST,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: env!("CARGO_PKG_NAME").to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                ..Default::default()
            },
            instructions: Some(
                "Provides access to myfund.pl investment portfolio data. \
                 Use list_portfolios to discover available portfolios, \
                 get_portfolio_summary for positions and totals, and \
                 get_portfolio_timeseries for historical data."
                    .to_string(),
            ),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::myfund::MyfundClient;

    const FIXTURE_OK: &str = include_str!("../tests/fixtures/portfolio_ok.json");
    const FIXTURE_ERR: &str = include_str!("../tests/fixtures/portfolio_error.json");

    async fn make_server(fixture: &'static str, http_status: u16) -> (MockServer, MyfundServer) {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(http_status)
                    .set_body_raw(fixture, "application/json"),
            )
            .mount(&server)
            .await;
        let client = MyfundClient::new("test-key")
            .unwrap()
            .with_base_url(server.uri());
        let mcp = MyfundServer::new(client, vec!["main".to_string(), "crypto".to_string()]);
        (server, mcp)
    }

    #[tokio::test]
    async fn test_list_portfolios_configured() {
        let (_mock, s) = make_server(FIXTURE_OK, 200).await;
        let result = s.list_portfolios().await;
        assert!(result.contains("main"));
        assert!(result.contains("crypto"));
    }

    #[tokio::test]
    async fn test_list_portfolios_empty() {
        let server = MockServer::start().await;
        let client = MyfundClient::new("key").unwrap().with_base_url(server.uri());
        let s = MyfundServer::new(client, vec![]);
        let result = s.list_portfolios().await;
        assert!(result.to_lowercase().contains("myfund_portfolios"));
    }

    #[tokio::test]
    async fn test_get_portfolio_summary_contains_portfel() {
        let (_mock, s) = make_server(FIXTURE_OK, 200).await;
        let result = s
            .get_portfolio_summary(Parameters(GetPortfolioSummaryParams {
                name: "main".to_string(),
            }))
            .await;
        assert!(result.contains("wartosc"));
        assert!(result.contains("250000.00"));
    }

    #[tokio::test]
    async fn test_get_portfolio_summary_contains_tickers() {
        let (_mock, s) = make_server(FIXTURE_OK, 200).await;
        let result = s
            .get_portfolio_summary(Parameters(GetPortfolioSummaryParams {
                name: "main".to_string(),
            }))
            .await;
        assert!(result.contains("NASDAQ_AAPL"));
        assert!(result.contains("WSE_PKN"));
        assert!(result.contains("ETF_VWCE"));
    }

    #[tokio::test]
    async fn test_get_portfolio_summary_no_timeseries() {
        let (_mock, s) = make_server(FIXTURE_OK, 200).await;
        let result = s
            .get_portfolio_summary(Parameters(GetPortfolioSummaryParams {
                name: "main".to_string(),
            }))
            .await;
        assert!(!result.contains("wartoscWCzasie"));
        assert!(!result.contains("zyskWCzasie"));
        assert!(!result.contains("stopaZwrotuWCzasie"));
    }

    #[tokio::test]
    async fn test_get_portfolio_summary_contains_struktura() {
        let (_mock, s) = make_server(FIXTURE_OK, 200).await;
        let result = s
            .get_portfolio_summary(Parameters(GetPortfolioSummaryParams {
                name: "main".to_string(),
            }))
            .await;
        assert!(result.contains("struktura"));
        assert!(result.contains("NASDAQ shares"));
        assert!(!result.contains("strukturaWalory"));
    }

    #[tokio::test]
    async fn test_get_portfolio_summary_tickers_slim_fields() {
        let (_mock, s) = make_server(FIXTURE_OK, 200).await;
        let result = s
            .get_portfolio_summary(Parameters(GetPortfolioSummaryParams {
                name: "main".to_string(),
            }))
            .await;
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        let tickers = v["tickers"].as_object().unwrap();
        for ticker in tickers.values() {
            let obj = ticker.as_object().unwrap();
            assert!(obj.contains_key("tickerClear"));
            assert!(obj.contains_key("nazwa"));
            assert!(obj.contains_key("liczbaJednostek"));
            // Ensure no extra fields leaked through
            assert!(!obj.contains_key("wartosc"));
            assert!(!obj.contains_key("zysk"));
            assert!(!obj.contains_key("typ"));
            assert!(!obj.contains_key("udzial"));
        }
    }

    #[tokio::test]
    async fn test_get_portfolio_summary_propagates_api_error() {
        let (_mock, s) = make_server(FIXTURE_ERR, 200).await;
        let result = s
            .get_portfolio_summary(Parameters(GetPortfolioSummaryParams {
                name: "bad".to_string(),
            }))
            .await;
        assert!(result.starts_with("Error:"));
    }

    #[tokio::test]
    async fn test_get_portfolio_timeseries_all_entries() {
        let (_mock, s) = make_server(FIXTURE_OK, 200).await;
        let result = s
            .get_portfolio_timeseries(Parameters(GetPortfolioTimeseriesParams {
                name: "main".to_string(),
                series: SeriesName::WartoscWCzasie,
                from: None,
                to: None,
            }))
            .await;
        let map: std::collections::BTreeMap<String, String> = serde_json::from_str(&result).unwrap();
        assert_eq!(map.len(), 8);
    }

    #[tokio::test]
    async fn test_get_portfolio_timeseries_date_filter_from() {
        let (_mock, s) = make_server(FIXTURE_OK, 200).await;
        let result = s
            .get_portfolio_timeseries(Parameters(GetPortfolioTimeseriesParams {
                name: "main".to_string(),
                series: SeriesName::WartoscWCzasie,
                from: Some("2023-06-15".to_string()),
                to: None,
            }))
            .await;
        let map: std::collections::BTreeMap<String, serde_json::Value> = serde_json::from_str(&result).unwrap();
        // 2023-06-15, 2023-09-01, 2024-01-01, 2024-03-01, 2024-03-22 = 5
        assert_eq!(map.len(), 5);
        assert!(map.contains_key("2023-06-15"));
        assert!(!map.contains_key("2022-01-10"));
    }

    #[tokio::test]
    async fn test_get_portfolio_timeseries_date_filter_range() {
        let (_mock, s) = make_server(FIXTURE_OK, 200).await;
        let result = s
            .get_portfolio_timeseries(Parameters(GetPortfolioTimeseriesParams {
                name: "main".to_string(),
                series: SeriesName::WartoscWCzasie,
                from: Some("2023-01-02".to_string()),
                to: Some("2024-01-01".to_string()),
            }))
            .await;
        let map: std::collections::BTreeMap<String, serde_json::Value> = serde_json::from_str(&result).unwrap();
        // 2023-01-02, 2023-06-15, 2023-09-01, 2024-01-01 = 4
        assert_eq!(map.len(), 4);
    }

    #[tokio::test]
    async fn test_get_portfolio_timeseries_empty_range() {
        let (_mock, s) = make_server(FIXTURE_OK, 200).await;
        let result = s
            .get_portfolio_timeseries(Parameters(GetPortfolioTimeseriesParams {
                name: "main".to_string(),
                series: SeriesName::WartoscWCzasie,
                from: Some("2025-01-01".to_string()),
                to: Some("2025-12-31".to_string()),
            }))
            .await;
        let map: std::collections::BTreeMap<String, serde_json::Value> = serde_json::from_str(&result).unwrap();
        assert!(map.is_empty());
    }
}
