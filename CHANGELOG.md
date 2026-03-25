# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2025-03-25

### Added
- Initial release of `myfund-mcp`, an MCP server exposing myfund.pl investment portfolio data as tools for AI assistants.
- `list_portfolios` tool: returns configured portfolio names from the `MYFUND_PORTFOLIOS` environment variable.
- `get_portfolio_summary` tool: returns portfolio totals, ticker positions, asset structure, and today's daily change (no time-series data to keep response size small).
- `get_portfolio_timeseries` tool: returns a named time-series (`wartoscWCzasie`, `zyskWCzasie`, `wkladWCzasie`, `benchWCzasie`, `stopaZwrotuWCzasie`) with optional date-range filtering.
- Structured logging to stderr via `tracing` (stdout is reserved for the MCP stdio channel).
- Configuration via `MYFUND_API_KEY` (required) and `MYFUND_PORTFOLIOS` (optional) environment variables.
