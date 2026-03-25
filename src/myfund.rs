use anyhow::{anyhow, Result};
use reqwest::Client;

use crate::models::PortfolioResponse;

const API_BASE: &str = "https://myfund.pl/API/v1/getPortfel.php";

pub struct MyfundClient {
    api_key: String,
    http: Client,
    base_url: String,
}

impl MyfundClient {
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        Ok(Self {
            api_key: api_key.into(),
            http: Client::builder().build()?,
            base_url: API_BASE.to_string(),
        })
    }

    /// Override the base URL (used in tests with a mock server).
    #[cfg(test)]
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    pub async fn fetch_portfolio(&self, name: &str) -> Result<PortfolioResponse> {
        let response = self
            .http
            .get(&self.base_url)
            .query(&[
                ("portfel", name),
                ("apiKey", &self.api_key),
                ("format", "json"),
            ])
            .send()
            .await?
            .error_for_status()?
            .json::<PortfolioResponse>()
            .await?;

        if response.is_error() {
            return Err(anyhow!("myfund.pl API error: {}", response.status.text));
        }

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const FIXTURE_OK: &str = include_str!("../tests/fixtures/portfolio_ok.json");
    const FIXTURE_ERR: &str = include_str!("../tests/fixtures/portfolio_error.json");

    async fn setup_mock(body: &'static str, status: u16) -> (MockServer, MyfundClient) {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(status).set_body_raw(body, "application/json"),
            )
            .mount(&server)
            .await;
        let client = MyfundClient::new("test-key")
            .unwrap()
            .with_base_url(server.uri());
        (server, client)
    }

    #[tokio::test]
    async fn test_fetch_portfolio_success() {
        let (_server, client) = setup_mock(FIXTURE_OK, 200).await;
        let result = client.fetch_portfolio("test").await;
        assert!(result.is_ok());
        let r = result.unwrap();
        assert_eq!(r.status.code, "0");
    }

    #[tokio::test]
    async fn test_fetch_portfolio_api_error() {
        let (_server, client) = setup_mock(FIXTURE_ERR, 200).await;
        let result = client.fetch_portfolio("bad").await;
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("myfund.pl API error"));
        assert!(msg.contains("Nie jesteś zalogowany"));
    }

    #[tokio::test]
    async fn test_fetch_portfolio_http_error() {
        let (_server, client) = setup_mock(FIXTURE_OK, 500).await;
        let result = client.fetch_portfolio("test").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_api_key_in_request() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/"))
            .and(query_param("apiKey", "my-secret-key"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(FIXTURE_OK, "application/json"))
            .mount(&server)
            .await;
        let client = MyfundClient::new("my-secret-key")
            .unwrap()
            .with_base_url(server.uri());
        assert!(client.fetch_portfolio("test").await.is_ok());
    }

    #[tokio::test]
    async fn test_portfolio_name_in_request() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/"))
            .and(query_param("portfel", "my-portfolio"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(FIXTURE_OK, "application/json"))
            .mount(&server)
            .await;
        let client = MyfundClient::new("key")
            .unwrap()
            .with_base_url(server.uri());
        assert!(client.fetch_portfolio("my-portfolio").await.is_ok());
    }

    #[tokio::test]
    async fn test_format_json_in_request() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/"))
            .and(query_param("format", "json"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(FIXTURE_OK, "application/json"))
            .mount(&server)
            .await;
        let client = MyfundClient::new("key")
            .unwrap()
            .with_base_url(server.uri());
        assert!(client.fetch_portfolio("test").await.is_ok());
    }
}
