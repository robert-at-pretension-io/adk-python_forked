# ScrapingBee Agent

The ScrapingBee Agent is an ADK agent that provides web scraping capabilities using the ScrapingBee API. It can extract and process content from websites, handling both static and JavaScript-rendered pages.

## Features

- Extract text content from any webpage
- JavaScript rendering support for dynamic sites
- Automatic HTML to text conversion
- Content truncation for large pages
- Error handling and timeout protection
- Support for premium proxies and ad blocking

## Prerequisites

- ScrapingBee API key (get one at https://www.scrapingbee.com)
- Set the `SCRAPINGBEE_API_KEY` environment variable

## Usage

### Starting the Agent

```bash
# Set your API key
export SCRAPINGBEE_API_KEY="your_api_key_here"

# From the agents/scraping_bee directory:
adk run

# Or from anywhere:
adk run agents/scraping_bee
```

### Example Interactions

**Scrape a static webpage:**
```
You: Scrape the content from https://example.com
Agent: [Returns extracted text content from the page]
```

**Scrape a JavaScript-rendered page:**
```
You: Extract the content from https://news.ycombinator.com
Agent: [Returns the rendered page content]
```

**Scrape without JavaScript (faster for static sites):**
```
You: Scrape https://example.com without rendering JavaScript
Agent: [Returns content without JS rendering]
```

**Analyze scraped content:**
```
You: Scrape https://blog.example.com/article and summarize the main points
Agent: [Scrapes the page and provides a summary]
```

## Configuration

The agent's `scrape_url` tool supports the following options:

- `url`: The complete URL of the webpage to scrape (required)
- `render_js`: Whether to render JavaScript (default: true, set to false for static sites)

## API Features Used

- Premium proxies for better reliability
- Ad blocking to reduce clutter
- Resource blocking for faster scraping
- Configurable timeouts based on rendering needs

## Error Handling

The agent handles common scraping scenarios:
- Missing API key
- Network timeouts
- Invalid URLs
- Binary responses (returns error message)
- API errors with descriptive messages

## Development

The agent is built using the ADK framework with a simple function tool. To modify, edit `agent.py` and test using the ADK CLI:

```bash
# Run the agent
adk run

# Test with custom configuration
adk run --config custom_config.yaml
```

The agent extracts clean text from HTML pages using BeautifulSoup and limits content to 25,000 characters for optimal performance.