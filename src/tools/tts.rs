//! Agent-callable TTS tool.
//!
//! Wraps the existing [`TtsManager`](crate::channels::tts::TtsManager) to
//! expose text-to-speech synthesis as an LLM tool. Audio is written to a
//! workspace-relative file and the path is returned to the agent.

use super::traits::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use crate::channels::tts::TtsManager;
use crate::config::TtsConfig;

/// Agent-callable text-to-speech tool.
///
/// Synthesizes text into audio files using the configured TTS provider
/// (OpenAI, ElevenLabs, Google Cloud, Edge, or Piper).
pub struct TtsTool {
    config: TtsConfig,
    workspace_dir: PathBuf,
    /// Lazily initialized manager — avoids errors at startup when TTS
    /// is configured but the provider key is missing or invalid.
    manager: OnceLock<Arc<TtsManager>>,
}

impl TtsTool {
    pub fn new(config: TtsConfig, workspace_dir: PathBuf) -> Self {
        Self {
            config,
            workspace_dir,
            manager: OnceLock::new(),
        }
    }

    fn get_manager(&self) -> anyhow::Result<&Arc<TtsManager>> {
        if let Some(m) = self.manager.get() {
            return Ok(m);
        }
        let m = TtsManager::new(&self.config)
            .map(Arc::new)
            .map_err(|e| anyhow::anyhow!("TTS initialization failed: {e}"))?;
        let _ = self.manager.set(m);
        Ok(self.manager.get().unwrap())
    }
}

#[async_trait]
impl Tool for TtsTool {
    fn name(&self) -> &str {
        "tts"
    }

    fn description(&self) -> &str {
        "Synthesize text into speech audio. Returns the path to the generated \
         audio file. Supports multiple providers (openai, elevenlabs, google, \
         edge, piper) and voices. Use this when the user asks you to read \
         something aloud, create an audio message, or generate speech."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "The text to synthesize into speech. Max ~4096 characters."
                },
                "provider": {
                    "type": "string",
                    "description": "TTS provider to use. If omitted, uses the default from config.",
                    "enum": ["openai", "elevenlabs", "google", "edge", "piper"]
                },
                "voice": {
                    "type": "string",
                    "description": "Voice identifier. Provider-specific (e.g. 'alloy' for OpenAI, 'Rachel' for ElevenLabs). If omitted, uses the default voice."
                },
                "filename": {
                    "type": "string",
                    "description": "Output filename (without path). Defaults to 'tts_output.mp3'."
                }
            },
            "required": ["text"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let text = args["text"].as_str().unwrap_or_default().trim();

        if text.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("text parameter is required and must not be empty".into()),
            });
        }

        let manager = match self.get_manager() {
            Ok(m) => m,
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("TTS not available: {e}")),
                });
            }
        };

        let provider = args["provider"].as_str();
        let voice = args["voice"].as_str();
        let filename = args["filename"].as_str().unwrap_or("tts_output.mp3");

        // Sanitize filename — no path separators allowed.
        let safe_filename = filename
            .replace(['/', '\\', '.'], "_")
            .trim_start_matches('.')
            .to_string();
        let safe_filename = if safe_filename.is_empty() {
            "tts_output.mp3".to_string()
        } else {
            safe_filename
        };

        let audio_bytes: anyhow::Result<Vec<u8>> = match (provider, voice) {
            (Some(p), Some(v)) => manager.synthesize_with_provider(text, p, v).await,
            (Some(p), None) => {
                // Use provider with a sensible default voice.
                manager.synthesize_with_provider(text, p, "alloy").await
            }
            _ => manager.synthesize(text).await,
        };

        match audio_bytes {
            Ok(bytes) => {
                let out_dir = self.workspace_dir.join("tts_output");
                if let Err(e) = tokio::fs::create_dir_all(&out_dir).await {
                    return Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Failed to create tts_output dir: {e}")),
                    });
                }

                let out_path = out_dir.join(&safe_filename);
                if let Err(e) = tokio::fs::write(&out_path, &bytes).await {
                    return Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Failed to write audio file: {e}")),
                    });
                }

                let size_kb = bytes.len() / 1024;
                Ok(ToolResult {
                    success: true,
                    output: format!("Audio saved to {} ({} KB)", out_path.display(), size_kb),
                    error: None,
                })
            }
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("TTS synthesis failed: {e}")),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_tool() -> TtsTool {
        TtsTool::new(TtsConfig::default(), PathBuf::from("/tmp/test-workspace"))
    }

    #[test]
    fn tool_name() {
        assert_eq!(test_tool().name(), "tts");
    }

    #[test]
    fn tool_description_mentions_speech() {
        assert!(test_tool().description().contains("speech"));
    }

    #[test]
    fn schema_requires_text() {
        let schema = test_tool().parameters_schema();
        let required = schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("text")));
    }

    #[test]
    fn schema_has_provider_enum() {
        let schema = test_tool().parameters_schema();
        let provider_enum = schema["properties"]["provider"]["enum"].as_array().unwrap();
        assert!(provider_enum.len() >= 4);
    }

    #[tokio::test]
    async fn empty_text_returns_error() {
        let tool = test_tool();
        let result = tool.execute(json!({"text": ""})).await.unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("empty"));
    }

    #[tokio::test]
    async fn whitespace_only_returns_error() {
        let tool = test_tool();
        let result = tool.execute(json!({"text": "   "})).await.unwrap();
        assert!(!result.success);
    }
}
