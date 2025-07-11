use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::process::Command;
use tracing::{debug, error, info};
use schemars::JsonSchema;

// Import rmcp SDK components
use rmcp::tool;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AiderParams {
    #[schemars(description = "The directory path where aider should run (must exist and contain code files)")]
    pub directory: String,
    
    #[schemars(description = "Detailed instructions for what changes aider should make to the code")]
    pub message: String,
    
    #[serde(default)]
    #[schemars(description = "Optional: A space-separated string of additional command-line options to pass to aider (e.g., '--show-diff --verbose'). Leave empty for none.")]
    pub options: String, // Changed back from Option<String>

    #[serde(default)]
    #[schemars(description = "Optional: The provider to use (e.g., 'anthropic', 'openai', 'gemini'). Leave empty to auto-detect based on available API keys.")]
    pub provider: String, // Changed from Option<String>

    #[serde(default)]
    #[schemars(description = "Optional: The model to use (e.g., 'claude-3-opus-20240229'). Leave empty to use AIDER_MODEL env var or provider default.")]
    pub model: String, // Changed from Option<String>

    #[serde(default)]
    #[schemars(description = "Optional: Reasoning effort level for OpenAI models. Values: 'low', 'medium', 'high'. Defaults to 'high' if empty.")]
    pub reasoning_effort: String, // Changed from Option<String>
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AiderResult {
    /// Whether the aider command completed successfully
    pub success: bool,
    /// The exit status code
    pub status: i32,
    /// Standard output from aider
    pub stdout: String,
    /// Standard error from aider
    pub stderr: String,
    /// The directory the command was run in
    pub directory: String,
    /// The message that was sent to aider
    pub message: String,
    /// The provider that was used (e.g., "anthropic", "openai", "gemini")
    pub provider: String,
    /// The model that was used (e.g., "claude-3-opus-20240229", "gemini/gemini-1.5-pro-latest")
    pub model: Option<String>,
}

pub struct AiderExecutor;

impl AiderExecutor {
    pub fn new() -> Self {
        AiderExecutor
    }

    /// Helper method to build command arguments for testing
    pub fn build_command_args(&self, params: &AiderParams) -> Vec<String> {
        // Determine provider: use explicit parameter if not empty, otherwise detect
        let provider = if !params.provider.trim().is_empty() {
            let p_l = params.provider.trim().to_lowercase();
            // Validate provider name
            if !["anthropic", "openai", "gemini"].contains(&p_l.as_str()) {
                error!("Unsupported provider '{}' specified. Defaulting to 'anthropic'.", params.provider);
                "anthropic".to_string() // Default to anthropic on invalid input
            } else {
                p_l // Use the valid specified provider
            }
        } else {
            debug!("No provider specified, attempting auto-detection.");
            Self::detect_provider() // Auto-detect if empty
        };

        // Retrieve API key: provider-specific or AIDER_API_KEY
        let provider_env_key = format!("{}_API_KEY", provider.to_uppercase());
        let api_key = std::env::var(&provider_env_key)
            .or_else(|_| {
                debug!("Provider-specific API key {} not found, falling back to AIDER_API_KEY", provider_env_key);
                std::env::var("AIDER_API_KEY")
            })
            .unwrap_or_default(); // Use empty string if no key is found

        // Warn if no API key is found
        if api_key.is_empty() {
            error!("No API key found for provider '{}'. Checked {} and AIDER_API_KEY", provider, provider_env_key);
        }

        // Get model: use param if not empty, else env var, else provider default
        let model = if !params.model.trim().is_empty() {
            Some(params.model.trim().to_string())
        } else {
            std::env::var("AIDER_MODEL").ok().or_else(|| {
                // Set default models based on provider if env var is also empty
                match provider.as_str() { // provider is already lowercase String
                    "anthropic" => {
                        debug!("Using default Anthropic model: anthropic/claude-3-7-sonnet-20250219");
                        Some("anthropic/claude-3-7-sonnet-20250219".to_string())
                    },
                    "openai" => {
                        debug!("Using default OpenAI model: openai/o3-mini");
                        Some("openai/o3-mini".to_string())
                    },
                    "gemini" => {
                        debug!("Using default Gemini model: gemini/gemini-2.5-pro-preview-03-25");
                        Some("gemini/gemini-2.5-pro-preview-03-25".to_string())
                    }
                    _ => {
                        error!("Cannot determine default model for unknown provider: {}", provider);
                        None
                    }
                }
            })
        };

        // Build the command
        let mut cmd_args = vec![
            "--message".to_string(),
            params.message.clone(),
            "--yes-always".to_string(),
            "--no-detect-urls".to_string(),
        ];

        // Always include the API key flag, even if key is empty
        cmd_args.push("--api-key".to_string());
        cmd_args.push(format!("{}={}", provider, api_key));

        // Add model if available
        if let Some(m) = &model {
            cmd_args.push("--model".to_string());
            cmd_args.push(m.clone());
            info!("Using provider '{}' with model '{}'", provider, m);
        } else {
            info!("Using provider '{}' with no specific model", provider);
        }

        // Add reasoning effort for OpenAI models
        if provider == "openai" { // provider is already lowercase String
            let effort = if params.reasoning_effort.trim().is_empty() {
                "high" // Default if param is empty
            } else {
                params.reasoning_effort.trim()
            };
            // Validate reasoning_effort - only allow "low", "medium", "high"
            let valid_efforts = ["low", "medium", "high"];
            let validated_effort = if valid_efforts.contains(&effort.to_lowercase().as_str()) {
                effort.to_lowercase() // Use validated lowercase effort
            } else {
                error!("Invalid reasoning_effort '{}' specified. Defaulting to 'high'", effort);
                "high".to_string() // Default to high if invalid
            };

            cmd_args.push("--reasoning-effort".to_string());
            cmd_args.push(validated_effort.clone());
            debug!("Using reasoning_effort: {}", validated_effort);
        }

        // Add any additional options from the options string if it's not empty
        if !params.options.trim().is_empty() {
            match shellwords::split(&params.options) { // Use params.options directly
                Ok(split_options) => {
                    debug!("Adding additional aider options: {:?}", split_options);
                    cmd_args.extend(split_options);
                }
                Err(e) => {
                    error!("Failed to parse additional aider options string '{}': {}. Options ignored.", params.options, e);
                    // Optionally, you could add the error to the stderr of the result later
                }
            }
        }

        cmd_args
    
    }
    
    /// Detects the provider based on available API keys in the environment.
    /// Prioritizes Gemini > Anthropic > OpenAI if multiple keys are present. Defaults to Gemini.
    fn detect_provider() -> String {
        let has_gemini = std::env::var("GEMINI_API_KEY").is_ok();
        let has_anthropic = std::env::var("ANTHROPIC_API_KEY").is_ok();
        let has_openai = std::env::var("OPENAI_API_KEY").is_ok();

        if has_gemini {
            debug!("Detected GEMINI_API_KEY, selecting 'gemini' provider.");
            "gemini".to_string()
        } else if has_anthropic {
            debug!("Detected ANTHROPIC_API_KEY, selecting 'anthropic' provider.");
            "anthropic".to_string()
        } else if has_openai {
            debug!("Detected OPENAI_API_KEY, selecting 'openai' provider.");
            "openai".to_string()
        } else {
            debug!("No specific provider API key found. Defaulting to 'gemini'.");
            "gemini".to_string() // Default if no keys are found
        }
    }

    pub async fn execute(&self, params: AiderParams) -> Result<AiderResult> {
        // Validate directory exists
        let dir_path = PathBuf::from(&params.directory);
        if !dir_path.exists() {
            return Err(anyhow!("Directory '{}' does not exist", params.directory));
        }
        if !dir_path.is_dir() {
            return Err(anyhow!("Path '{}' is not a directory", params.directory));
        }

        // Basic validation of the message
        if params.message.trim().is_empty() {
            return Err(anyhow!("Message cannot be empty"));
        }

        // Build command arguments (this also determines the provider)
        let cmd_args = self.build_command_args(&params);
        
        // Extract provider and model used (determined during arg building)
        // This is a bit indirect, ideally build_command_args would return them too.
        // We re-determine provider here for the result struct.
        let provider = if !params.provider.trim().is_empty() {
             let p_l = params.provider.trim().to_lowercase();
             if ["anthropic", "openai", "gemini"].contains(&p_l.as_str()) { p_l } else { Self::detect_provider() }
        } else {
            Self::detect_provider()
        };

        // Re-determine model used for the result struct
        let model = if !params.model.trim().is_empty() {
            Some(params.model.trim().to_string())
        } else {
            std::env::var("AIDER_MODEL").ok().or_else(|| {
                match provider.as_str() {
                    "anthropic" => Some("anthropic/claude-3-7-sonnet-20250219".to_string()),
                    "openai" => Some("openai/o3-mini".to_string()),
                    "gemini" => Some("gemini/gemini-2.5-pro-preview-03-25".to_string()), // Updated default model
                    _ => None,
                }
            })
        }; // <-- Add missing semicolon here

        debug!("Running aider with args: {:?}", cmd_args);
        info!("Executing aider in directory: {}", params.directory);

        // Execute aider command
        let output = Command::new("aider")
            .args(&cmd_args)
            .current_dir(&params.directory)
            .output()
            .await
            .map_err(|e| anyhow!("Failed to execute aider: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        // Log results
        if !output.status.success() {
            error!("Aider command failed with status: {:?}", output.status);
            if !stderr.is_empty() {
                error!("Stderr: {}", stderr);
            }
        } else {
            info!("Aider command completed successfully");
            debug!("Stdout length: {}", stdout.len());
        }

        Ok(AiderResult {
            success: output.status.success(),
            status: output.status.code().unwrap_or(-1),
            stdout,
            stderr,
            directory: params.directory,
            message: params.message,
            provider, // Use the determined provider
            model,    // Use the determined model
        })
    }
}

#[derive(Debug, Clone)]
pub struct AiderTool;

impl AiderTool {
    pub fn new() -> Self {
        Self
    }
}

#[tool(tool_box)]
impl AiderTool {
    #[tool(description = "AI pair programming tool for making targeted code changes. Requires VERY SPECIFIC instructions to perform well. It has NO CONTEXT from the conversation; all necessary details must be in the 'message'. Use for implementing new features, adding tests, fixing bugs, refactoring code, or making structural changes across multiple files.")]
    pub async fn aider(
        &self,
        #[tool(aggr)] params: AiderParams
    ) -> String {
        info!("Running aider in directory: {} with provider: {:?}", 
             params.directory, params.provider);
        
        let executor = AiderExecutor::new();
        
        match executor.execute(params).await {
            Ok(result) => {
                // Format a nice response
                let model_info = match &result.model {
                    Some(model) => format!("Provider: {} | Model: {}", result.provider, model),
                    None => format!("Provider: {}", result.provider),
                };
                
                format!(
                    "Aider execution {} [{}]\n\nDirectory: {}\nExit status: {}\n\nSTDOUT:\n{}\n\nSTDERR:\n{}",
                    if result.success { "succeeded" } else { "failed" },
                    model_info,
                    result.directory,
                    result.status,
                    result.stdout,
                    result.stderr
                )
            },
            Err(e) => {
                error!("Aider execution failed: {}", e);
                format!("Error executing aider: {}", e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::path::Path;
    use tokio::fs;
    use tokio::runtime::Runtime;

    // Helper function to create a temporary directory for testing
    async fn create_temp_dir() -> Result<String> {
        let temp_dir = format!("/tmp/aider_test_{}", std::process::id());
        if !Path::new(&temp_dir).exists() {
            fs::create_dir_all(&temp_dir).await?;
        }
        Ok(temp_dir)
    }

    // Test provider validation logic
    #[test]
    fn test_provider_validation() {
        let rt = Runtime::new().unwrap();
        
        rt.block_on(async {
            let temp_dir = create_temp_dir().await.unwrap();
            let executor = AiderExecutor::new();
            
            // Test with valid provider: anthropic
            let params = AiderParams {
                directory: temp_dir.clone(),
                message: "Test message".to_string(),
                options: "".to_string(), // Changed back from None
                provider: "anthropic".to_string(),
                model: "".to_string(),
                reasoning_effort: "".to_string(),
            };
            // We don't actually execute the command, just check the validation logic
            // by inspecting the command that would be built
            let cmd_args = executor.build_command_args(&params);
            assert!(cmd_args.contains(&"--api-key".to_string()));
            
            // Test with valid provider: openai
            let params = AiderParams {
                directory: temp_dir.clone(),
                message: "Test message".to_string(),
                options: "".to_string(), // Changed back from None
                provider: "openai".to_string(),
                model: "".to_string(),
                reasoning_effort: "".to_string(),
            };
            let cmd_args = executor.build_command_args(&params);
            assert!(cmd_args.contains(&"--api-key".to_string()));
            
            // Test with invalid provider - should default to anthropic
            let params = AiderParams {
                directory: temp_dir.clone(),
                message: "Test message".to_string(),
                options: "".to_string(), // Changed back from None
                provider: "invalid_provider".to_string(),
                model: "".to_string(),
                reasoning_effort: "".to_string(),
            };
            let cmd_args = executor.build_command_args(&params);
            // The provider should be defaulted to anthropic
            assert!(cmd_args.iter().any(|arg| arg.contains("anthropic=")));
            
            // Handle cleanup gracefully
            let _ = fs::remove_dir_all(temp_dir).await;
        });
    }

    // Test provider detection logic
    #[test]
    fn test_provider_detection() {
        // Test priority: Gemini > Anthropic > OpenAI > Default (Gemini)
        
        // Case 1: Only Gemini key
        env::remove_var("ANTHROPIC_API_KEY");
        env::remove_var("OPENAI_API_KEY");
        env::set_var("GEMINI_API_KEY", "test_key");
        assert_eq!(AiderExecutor::detect_provider(), "gemini");

        // Case 2: Only Anthropic key
        env::set_var("ANTHROPIC_API_KEY", "test_key");
        env::remove_var("OPENAI_API_KEY");
        env::set_var("ANTHROPIC_API_KEY", "test_key");
        env::remove_var("OPENAI_API_KEY");
        env::remove_var("GEMINI_API_KEY");
        assert_eq!(AiderExecutor::detect_provider(), "anthropic");

        // Case 3: Only OpenAI key
        env::remove_var("ANTHROPIC_API_KEY");
        env::set_var("OPENAI_API_KEY", "test_key");
        env::remove_var("ANTHROPIC_API_KEY");
        env::set_var("OPENAI_API_KEY", "test_key");
        env::remove_var("GEMINI_API_KEY");
        assert_eq!(AiderExecutor::detect_provider(), "openai");

        // Case 4: Gemini and Anthropic keys (Gemini priority)
        env::set_var("GEMINI_API_KEY", "test_key");
        env::set_var("ANTHROPIC_API_KEY", "test_key");
        env::remove_var("OPENAI_API_KEY");
        assert_eq!(AiderExecutor::detect_provider(), "gemini");

        // Case 5: Anthropic and OpenAI keys (Anthropic priority)
        env::remove_var("GEMINI_API_KEY");
        env::set_var("ANTHROPIC_API_KEY", "test_key");
        env::set_var("OPENAI_API_KEY", "test_key");
        assert_eq!(AiderExecutor::detect_provider(), "anthropic");

        // Case 6: All keys (Gemini priority)
        env::set_var("GEMINI_API_KEY", "test_key");
        env::set_var("ANTHROPIC_API_KEY", "test_key");
        env::set_var("OPENAI_API_KEY", "test_key");
        assert_eq!(AiderExecutor::detect_provider(), "gemini");

        // Case 7: No keys (Default to Gemini)
        env::remove_var("GEMINI_API_KEY");
        env::remove_var("ANTHROPIC_API_KEY");
        env::remove_var("OPENAI_API_KEY");
        assert_eq!(AiderExecutor::detect_provider(), "gemini");

        // Clean up env vars
        env::remove_var("GEMINI_API_KEY");
        env::remove_var("ANTHROPIC_API_KEY");
        env::remove_var("OPENAI_API_KEY");
        env::remove_var("GEMINI_API_KEY");
    }
    
    // Test default model selection logic
    #[test]
    fn test_default_model_selection() {
        let rt = Runtime::new().unwrap();
        
        rt.block_on(async {
            let temp_dir = create_temp_dir().await.unwrap();
            let executor = AiderExecutor::new();
            
            // Test default model for anthropic
            let params = AiderParams {
                directory: temp_dir.clone(),
                message: "Test message".to_string(),
                options: "".to_string(), // Changed back from None
                provider: "anthropic".to_string(),
                model: "".to_string(),
                reasoning_effort: "".to_string(),
            };
            let cmd_args = executor.build_command_args(&params);
            assert!(cmd_args.contains(&"--model".to_string()));
            let model_index = cmd_args.iter().position(|arg| arg == "--model").unwrap();
            assert_eq!(cmd_args[model_index + 1], "anthropic/claude-3-7-sonnet-20250219");
            
            // Test default model for openai
            let params = AiderParams {
                directory: temp_dir.clone(),
                message: "Test message".to_string(),
                options: "".to_string(), // Changed back from None
                provider: "openai".to_string(),
                model: "".to_string(),
                reasoning_effort: "".to_string(),
            };
            let cmd_args = executor.build_command_args(&params);
            assert!(cmd_args.contains(&"--model".to_string()));
            let model_index = cmd_args.iter().position(|arg| arg == "--model").unwrap();
            assert_eq!(cmd_args[model_index + 1], "openai/o3-mini");

            // Test default model for gemini
            let params = AiderParams {
                directory: temp_dir.clone(),
                message: "Test message".to_string(),
                options: "".to_string(), // Changed back from None
                provider: "gemini".to_string(),
                model: "".to_string(),
                reasoning_effort: "".to_string(),
            };
            let cmd_args = executor.build_command_args(&params);
            assert!(cmd_args.contains(&"--model".to_string()));
            let model_index = cmd_args.iter().position(|arg| arg == "--model").unwrap();
            assert_eq!(cmd_args[model_index + 1], "gemini/gemini-2.5-pro-preview-03-25"); // Updated default model
            
            // Test custom model overrides default
            let params = AiderParams {
                directory: temp_dir.clone(),
                message: "Test message".to_string(),
                options: "".to_string(), // Changed back from None
                provider: "anthropic".to_string(),
                model: "claude-3-opus-20240229".to_string(),
                reasoning_effort: "".to_string(),
            };
            let cmd_args = executor.build_command_args(&params);
            assert!(cmd_args.contains(&"--model".to_string()));
            let model_index = cmd_args.iter().position(|arg| arg == "--model").unwrap();
            assert_eq!(cmd_args[model_index + 1], "claude-3-opus-20240229");
            
            // Handle cleanup gracefully
            let _ = fs::remove_dir_all(temp_dir).await;
        });
    }
    
    // Test reasoning_effort validation
    #[test]
    fn test_reasoning_effort_validation() {
        let rt = Runtime::new().unwrap();
        
        rt.block_on(async {
            let temp_dir = create_temp_dir().await.unwrap();
            let executor = AiderExecutor::new();
            
            // Test valid reasoning_effort with OpenAI
            let params = AiderParams {
                directory: temp_dir.clone(),
                message: "Test message".to_string(),
                options: "".to_string(), // Changed back from None
                provider: "openai".to_string(),
                model: "".to_string(),
                reasoning_effort: "high".to_string(),
            };
            let cmd_args = executor.build_command_args(&params);
            assert!(cmd_args.contains(&"--reasoning-effort".to_string()));
            let effort_index = cmd_args.iter().position(|arg| arg == "--reasoning-effort").unwrap();
            assert_eq!(cmd_args[effort_index + 1], "high");
            
            // Test invalid reasoning_effort with OpenAI - should default to high
            let params = AiderParams {
                directory: temp_dir.clone(),
                message: "Test message".to_string(),
                options: "".to_string(), // Changed back from None
                provider: "openai".to_string(),
                model: "".to_string(),
                reasoning_effort: "invalid_effort".to_string(),
            };
            let cmd_args = executor.build_command_args(&params);
            assert!(cmd_args.contains(&"--reasoning-effort".to_string()));
            let effort_index = cmd_args.iter().position(|arg| arg == "--reasoning-effort").unwrap();
            assert_eq!(cmd_args[effort_index + 1], "high");
            
            // Test reasoning_effort with Anthropic - should be ignored
            let params = AiderParams {
                directory: temp_dir.clone(),
                message: "Test message".to_string(),
                options: "".to_string(), // Changed back from None
                provider: "anthropic".to_string(),
                model: "".to_string(),
                reasoning_effort: "high".to_string(),
            };
            let cmd_args = executor.build_command_args(&params);
            assert!(!cmd_args.contains(&"--reasoning-effort".to_string()));

            // Test reasoning_effort with Gemini - should be ignored
            let params = AiderParams {
                directory: temp_dir.clone(),
                message: "Test message".to_string(),
                options: "".to_string(), // Changed back from None
                provider: "gemini".to_string(),
                model: "".to_string(),
                reasoning_effort: "high".to_string(),
            };
            let cmd_args = executor.build_command_args(&params);
            assert!(!cmd_args.contains(&"--reasoning-effort".to_string()));
            
            // Handle cleanup gracefully
            let _ = fs::remove_dir_all(temp_dir).await;
        });
    }

    // Test thinking_tokens validation
    #[test]
    fn test_thinking_tokens_validation() {
        let rt = Runtime::new().unwrap();
        
        rt.block_on(async {
            let temp_dir = create_temp_dir().await.unwrap();
            let executor = AiderExecutor::new();
            
            // Test thinking_tokens validation is removed
            // Handle cleanup gracefully
            let _ = fs::remove_dir_all(temp_dir).await;
        });
    }
}
