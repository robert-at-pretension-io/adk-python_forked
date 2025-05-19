# Copyright 2025 Google LLC
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

import os
import json
from typing import Optional

from google.adk import Agent
from google.adk.tools.tool_context import ToolContext


def scrape_url(
    url: str,
    render_js: bool = True,
    tool_context: ToolContext = None
) -> str:
    """Scrape and extract text content from a webpage using ScrapingBee API.
    
    Args:
        url: The complete URL of the webpage to read and analyze
        render_js: Whether to render JavaScript (default: true; set to false for faster scraping of static sites)
        tool_context: ADK tool context for access to services
        
    Returns:
        Extracted text content from the webpage in a readable format
    """
    import requests
    from bs4 import BeautifulSoup
    
    # Get API key from environment
    api_key = os.environ.get('SCRAPINGBEE_API_KEY')
    if not api_key:
        return "Error: SCRAPINGBEE_API_KEY environment variable must be set"
    
    # Calculate timeout based on render_js
    timeout_ms = 15000 if render_js else 8000
    
    # Build query parameters
    params = {
        'api_key': api_key,
        'url': url,
        'render_js': str(render_js).lower(),
        'premium_proxy': 'true',
        'block_ads': 'true',
        'block_resources': 'true',
        'timeout': str(timeout_ms)
    }
    
    try:
        # Make the request with timeout
        response = requests.get(
            'https://app.scrapingbee.com/api/v1/',
            params=params,
            timeout=20
        )
        
        if not response.ok:
            return f"Error: ScrapingBee API failed with status {response.status_code}: {response.text}"
        
        # Get content type
        content_type = response.headers.get('content-type', '')
        
        if content_type.startswith('text') or 'json' in content_type:
            # Process text content
            html_content = response.text
            
            # Extract readable text from HTML using BeautifulSoup
            soup = BeautifulSoup(html_content, 'html.parser')
            
            # Remove script and style elements
            for script in soup(['script', 'style']):
                script.decompose()
            
            # Get the text content
            text_content = soup.get_text(separator='\n', strip=True)
            
            # Clean up excessive newlines
            lines = [line.strip() for line in text_content.splitlines() if line.strip()]
            text_content = '\n'.join(lines)
            
            # Truncate if too long
            MAX_CHARS = 25000
            if len(text_content) > MAX_CHARS:
                text_content = text_content[:MAX_CHARS] + "\n\n... (content truncated)"
            
            # Add title and source URL if available
            title = soup.find('title')
            if title:
                text_content = f"# {title.get_text(strip=True)}\n\nSource: {url}\n\n{text_content}"
            else:
                text_content = f"Source: {url}\n\n{text_content}"
                
            return text_content
        else:
            return "Error: Cannot process binary response from URL"
            
    except requests.Timeout:
        return "Error: Request to ScrapingBee timed out after 20 seconds"
    except requests.RequestException as e:
        return f"Error: Failed to connect to ScrapingBee API: {e}"
    except Exception as e:
        return f"Error: {e}"


root_agent = Agent(
    model='gemini-2.0-flash',
    name='scraping_bee_agent', 
    description='Web scraping agent using ScrapingBee API',
    instruction="""
    You are a web scraping assistant that uses the ScrapingBee API to extract and process 
    content from websites.
    
    When a user requests to scrape a webpage:
    1. Extract the URL from their message
    2. Determine if JavaScript rendering is needed (enable by default, disable for static sites)
    3. Call the scrape_url tool with the appropriate parameters
    4. Present the extracted content in a readable format
    
    If the user needs help understanding the content, you can analyze and summarize the 
    extracted text. Always mention the source URL when presenting scraped content.
    
    Note: This tool requires a SCRAPINGBEE_API_KEY environment variable to be set.
    """,
    tools=[scrape_url],
)