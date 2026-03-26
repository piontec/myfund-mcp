use std::collections::HashMap;
use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;

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

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetPortfolioSummaryParams {
    /// Name of the portfolio on myfund.pl (case-sensitive).
    pub name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetPortfolioTimeseriesParams {
    /// Name of the portfolio on myfund.pl (case-sensitive).
    pub name: String,
    /// Which time-series to fetch. Valid values: wartoscWCzasie, zyskWCzasie,
    /// wkladWCzasie, benchWCzasie, stopaZwrotuWCzasie
    pub series: String,
    /// Optional start date filter (YYYY-MM-DD, inclusive).
    pub from: Option<String>,
    /// Optional end date filter (YYYY-MM-DD, inclusive).
    pub to: Option<String>,
}

#[tool_router]
impl MyfundServer {
    /// List all configured myfund.pl portfolio names.
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

    /// Fetch portfolio totals, all ticker positions, asset structure, and today's
    /// daily change. Time-series data is intentionally excluded to keep the
    /// response size manageable -- use get_portfolio_timeseries for that.
    #[tool(
        description = "Fetch portfolio summary: totals, all ticker positions, asset structure breakdown, and today's daily change. Does not include time-series data."
    )]
    async fn get_portfolio_summary(
        &self,
        Parameters(params): Parameters<GetPortfolioSummaryParams>,
    ) -> String {
        match self.client.fetch_portfolio(&params.name).await {
            Err(e) => format!("Error: {e}"),
            Ok(r) => {
                let slim_tickers: Option<HashMap<String, serde_json::Value>> =
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
                    "zmianaDzienna": r.zmiana_dzienna,
                });
                serde_json::to_string_pretty(&summary)
                    .unwrap_or_else(|e| format!("Serialization error: {e}"))
            }
        }
    }

    /// Fetch a single named time-series for a portfolio, with optional date-range
    /// filtering. Use get_portfolio_summary first to understand the portfolio.
    #[tool(
        description = "Fetch a time-series for a portfolio. Valid series names: wartoscWCzasie (portfolio value), zyskWCzasie (profit), wkladWCzasie (contributions), benchWCzasie (benchmark return), stopaZwrotuWCzasie (rate of return). Optionally filter by from/to dates (YYYY-MM-DD)."
    )]
    async fn get_portfolio_timeseries(
        &self,
        Parameters(params): Parameters<GetPortfolioTimeseriesParams>,
    ) -> String {
        let response = match self.client.fetch_portfolio(&params.name).await {
            Err(e) => return format!("Error: {e}"),
            Ok(r) => r,
        };

        let series: Option<&HashMap<String, String>> = match params.series.as_str() {
            "wartoscWCzasie" => response.wartosc_w_czasie.as_ref(),
            "zyskWCzasie" => response.zysk_w_czasie.as_ref(),
            "wkladWCzasie" => response.wklad_w_czasie.as_ref(),
            "benchWCzasie" => response.bench_w_czasie.as_ref(),
            "stopaZwrotuWCzasie" => response.stopa_zwrotu_w_czasie.as_ref(),
            other => {
                return format!(
                    "Unknown series '{}'. Valid values: wartoscWCzasie, zyskWCzasie, \
                     wkladWCzasie, benchWCzasie, stopaZwrotuWCzasie",
                    other
                )
            }
        };

        let Some(series) = series else {
            return "No data available for this series.".to_string();
        };

        let mut filtered: Vec<(&String, &String)> = series
            .iter()
            .filter(|(date, _)| {
                if let Some(from) = &params.from {
                    if date.as_str() < from.as_str() {
                        return false;
                    }
                }
                if let Some(to) = &params.to {
                    if date.as_str() > to.as_str() {
                        return false;
                    }
                }
                true
            })
            .collect();

        filtered.sort_by_key(|(date, _)| *date);

        let map: HashMap<&str, &str> =
            filtered.iter().map(|(d, v)| (d.as_str(), v.as_str())).collect();

        serde_json::to_string_pretty(&map)
            .unwrap_or_else(|e| format!("Serialization error: {e}"))
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
        let (_, s) = make_server(FIXTURE_OK, 200).await;
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
        let (_, s) = make_server(FIXTURE_OK, 200).await;
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
        let (_, s) = make_server(FIXTURE_OK, 200).await;
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
        let (_, s) = make_server(FIXTURE_OK, 200).await;
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
    async fn test_get_portfolio_summary_no_struktura() {
        let (_, s) = make_server(FIXTURE_OK, 200).await;
        let result = s
            .get_portfolio_summary(Parameters(GetPortfolioSummaryParams {
                name: "main".to_string(),
            }))
            .await;
        assert!(!result.contains("struktura"));
        assert!(!result.contains("strukturaWalory"));
    }

    #[tokio::test]
    async fn test_get_portfolio_summary_tickers_slim_fields() {
        let (_, s) = make_server(FIXTURE_OK, 200).await;
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
        let (_, s) = make_server(FIXTURE_ERR, 200).await;
        let result = s
            .get_portfolio_summary(Parameters(GetPortfolioSummaryParams {
                name: "bad".to_string(),
            }))
            .await;
        assert!(result.starts_with("Error:"));
    }

    #[tokio::test]
    async fn test_get_portfolio_timeseries_all_entries() {
        let (_, s) = make_server(FIXTURE_OK, 200).await;
        let result = s
            .get_portfolio_timeseries(Parameters(GetPortfolioTimeseriesParams {
                name: "main".to_string(),
                series: "wartoscWCzasie".to_string(),
                from: None,
                to: None,
            }))
            .await;
        let map: HashMap<String, String> = serde_json::from_str(&result).unwrap();
        assert_eq!(map.len(), 8);
    }

    #[tokio::test]
    async fn test_get_portfolio_timeseries_date_filter_from() {
        let (_, s) = make_server(FIXTURE_OK, 200).await;
        let result = s
            .get_portfolio_timeseries(Parameters(GetPortfolioTimeseriesParams {
                name: "main".to_string(),
                series: "wartoscWCzasie".to_string(),
                from: Some("2023-06-15".to_string()),
                to: None,
            }))
            .await;
        let map: HashMap<String, String> = serde_json::from_str(&result).unwrap();
        // 2023-06-15, 2023-09-01, 2024-01-01, 2024-03-01, 2024-03-22 = 5
        assert_eq!(map.len(), 5);
        assert!(map.contains_key("2023-06-15"));
        assert!(!map.contains_key("2022-01-10"));
    }

    #[tokio::test]
    async fn test_get_portfolio_timeseries_date_filter_range() {
        let (_, s) = make_server(FIXTURE_OK, 200).await;
        let result = s
            .get_portfolio_timeseries(Parameters(GetPortfolioTimeseriesParams {
                name: "main".to_string(),
                series: "wartoscWCzasie".to_string(),
                from: Some("2023-01-02".to_string()),
                to: Some("2024-01-01".to_string()),
            }))
            .await;
        let map: HashMap<String, String> = serde_json::from_str(&result).unwrap();
        // 2023-01-02, 2023-06-15, 2023-09-01, 2024-01-01 = 4
        assert_eq!(map.len(), 4);
    }

    #[tokio::test]
    async fn test_get_portfolio_timeseries_invalid_series() {
        let (_, s) = make_server(FIXTURE_OK, 200).await;
        let result = s
            .get_portfolio_timeseries(Parameters(GetPortfolioTimeseriesParams {
                name: "main".to_string(),
                series: "notASeries".to_string(),
                from: None,
                to: None,
            }))
            .await;
        assert!(result.contains("Unknown series"));
        assert!(result.contains("notASeries"));
    }

    #[tokio::test]
    async fn test_get_portfolio_timeseries_empty_range() {
        let (_, s) = make_server(FIXTURE_OK, 200).await;
        let result = s
            .get_portfolio_timeseries(Parameters(GetPortfolioTimeseriesParams {
                name: "main".to_string(),
                series: "wartoscWCzasie".to_string(),
                from: Some("2025-01-01".to_string()),
                to: Some("2025-12-31".to_string()),
            }))
            .await;
        let map: HashMap<String, String> = serde_json::from_str(&result).unwrap();
        assert!(map.is_empty());
    }
}
