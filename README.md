# myfund-mcp

An MCP (Model Context Protocol) server that exposes your [myfund.pl](https://myfund.pl) investment portfolio data as tools for AI assistants such as Claude Desktop or GitHub Copilot.

## Features

Three focused MCP tools:

| Tool | Description |
|------|-------------|
| `list_portfolios` | Lists the portfolio names configured via the env var |
| `get_portfolio_summary` | Returns portfolio totals, all ticker positions, asset-class breakdown, and today's daily change — no time-series, keeping the response small |
| `get_portfolio_timeseries` | Returns one historical time-series with optional `from`/`to` date filtering |

The raw myfund.pl API response is ~660 KB (years of daily data). Splitting it into these tools keeps each AI context window usage reasonable.

## Prerequisites

- Rust 1.75+ (`rustup update stable`)
- A myfund.pl account with at least one portfolio

## Getting your API key

1. Log in to myfund.pl
2. Go to **Menu → Account → Account Settings**
3. Find the **API Key** section and click **Generate**

> **Note:** Each click of Generate invalidates the previous key.

## Building

```bash
git clone https://github.com/piontec/myfund-mcp
cd myfund-mcp
cargo build --release
# binary is at target/release/myfund-mcp
```

## Configuration

| Environment Variable | Required | Description |
|----------------------|----------|-------------|
| `MYFUND_API_KEY` | ✅ | API key from your myfund.pl account settings |
| `MYFUND_PORTFOLIOS` | optional | Comma-separated portfolio names, e.g. `MyStocks,Crypto` — enables `list_portfolios` |

Portfolio names are **case-sensitive** and must match exactly as shown on myfund.pl.

## Claude Desktop integration

Add to `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS) or `%APPDATA%\Claude\claude_desktop_config.json` (Windows):

```json
{
  "mcpServers": {
    "myfund": {
      "command": "/absolute/path/to/myfund-mcp",
      "env": {
        "MYFUND_API_KEY": "your-api-key-here",
        "MYFUND_PORTFOLIOS": "MyPortfolio,AnotherPortfolio"
      }
    }
  }
}
```

## GitHub Copilot (VS Code) integration

Add to your VS Code `settings.json`:

```json
{
  "github.copilot.chat.mcp.servers": {
    "myfund": {
      "command": "/absolute/path/to/myfund-mcp",
      "env": {
        "MYFUND_API_KEY": "your-api-key-here",
        "MYFUND_PORTFOLIOS": "MyPortfolio"
      }
    }
  }
}
```

## Tool reference

### `list_portfolios`
No parameters. Returns the comma-separated names from `MYFUND_PORTFOLIOS`, or a hint to set the variable if it is not configured.

### `get_portfolio_summary`
| Parameter | Type | Description |
|-----------|------|-------------|
| `name` | string | Portfolio name (case-sensitive) |

Returns JSON with `portfel` (totals), `tickers` (all positions), `struktura` (asset-class shares), and `zmianaDzienna` (today's change).

### `get_portfolio_timeseries`
| Parameter | Type | Description |
|-----------|------|-------------|
| `name` | string | Portfolio name (case-sensitive) |
| `series` | string | One of: `wartoscWCzasie`, `zyskWCzasie`, `wkladWCzasie`, `benchWCzasie`, `stopaZwrotuWCzasie` |
| `from` | string? | Start date filter `YYYY-MM-DD` (inclusive) |
| `to` | string? | End date filter `YYYY-MM-DD` (inclusive) |

Returns a JSON object mapping dates to values, sorted chronologically.

## Running tests

```bash
cargo test
```

## Notes

- myfund.pl caches API responses for **5 minutes** server-side.
- Logs are written to **stderr**; stdout is reserved for the MCP stdio channel.
