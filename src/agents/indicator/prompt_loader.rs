use sha2::{Digest, Sha256};

/// Embedded fallback prompt (compiled into the binary).
const EMBEDDED_PROMPT: &str = include_str!("../../../prompts/indicator_system.md");

/// Where the system prompt was loaded from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptSource {
    /// Loaded from an external file at the given path.
    File(String),
    /// Using the compiled-in embedded prompt.
    Embedded,
}

impl std::fmt::Display for PromptSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PromptSource::File(path) => write!(f, "file:{path}"),
            PromptSource::Embedded => write!(f, "embedded"),
        }
    }
}

/// Loaded prompt text with provenance metadata.
#[derive(Debug, Clone)]
pub struct LoadedPrompt {
    pub text: String,
    /// First 16 hex chars of SHA-256 over raw file bytes (no normalization).
    pub hash: String,
    pub source: PromptSource,
}

/// Load the system prompt from an optional file path, falling back to the
/// embedded prompt on any error (file not found, I/O error, empty file).
///
/// Hashing uses raw file bytes — no whitespace normalization — so any edit
/// (including trailing newlines) changes the hash.
pub fn load(prompt_path: Option<&str>) -> LoadedPrompt {
    if let Some(path) = prompt_path {
        match std::fs::read_to_string(path) {
            Ok(contents) if !contents.trim().is_empty() => {
                let hash = sha256_short(&contents);
                eprintln!("  Loaded prompt from {path} (hash: {hash})");
                return LoadedPrompt {
                    text: contents,
                    hash,
                    source: PromptSource::File(path.to_string()),
                };
            }
            Ok(_) => {
                eprintln!("  WARN: prompt file {path} is empty — using embedded prompt");
            }
            Err(e) => {
                eprintln!("  WARN: failed to load prompt from {path}: {e} — using embedded prompt");
            }
        }
    }

    let hash = sha256_short(EMBEDDED_PROMPT);
    LoadedPrompt {
        text: EMBEDDED_PROMPT.to_string(),
        hash,
        source: PromptSource::Embedded,
    }
}

/// SHA-256 of raw bytes, truncated to 16 hex chars (64 bits).
pub fn sha256_short(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    format!("{:x}", digest)[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_fallback() {
        let loaded = load(None);
        assert_eq!(loaded.source, PromptSource::Embedded);
        assert!(!loaded.text.is_empty());
        assert_eq!(loaded.hash.len(), 16);
    }

    #[test]
    fn file_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_prompt.txt");
        std::fs::write(&path, "You are a test analyst.").unwrap();

        let loaded = load(Some(path.to_str().unwrap()));
        assert!(matches!(loaded.source, PromptSource::File(_)));
        assert_eq!(loaded.text, "You are a test analyst.");
        assert_eq!(loaded.hash.len(), 16);
    }

    #[test]
    fn missing_file_falls_back_to_embedded() {
        let loaded = load(Some("/nonexistent/prompt.txt"));
        assert_eq!(loaded.source, PromptSource::Embedded);
        assert!(!loaded.text.is_empty());
    }

    #[test]
    fn empty_file_falls_back_to_embedded() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.txt");
        std::fs::write(&path, "   \n  ").unwrap();

        let loaded = load(Some(path.to_str().unwrap()));
        assert_eq!(loaded.source, PromptSource::Embedded);
    }

    #[test]
    fn hash_is_deterministic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prompt.txt");
        std::fs::write(&path, "Hello world").unwrap();

        let a = load(Some(path.to_str().unwrap()));
        let b = load(Some(path.to_str().unwrap()));
        assert_eq!(a.hash, b.hash);
    }

    #[test]
    fn hash_changes_with_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prompt.txt");

        std::fs::write(&path, "version 1").unwrap();
        let h1 = load(Some(path.to_str().unwrap())).hash;

        std::fs::write(&path, "version 2").unwrap();
        let h2 = load(Some(path.to_str().unwrap())).hash;

        assert_ne!(h1, h2);
    }

    #[test]
    fn source_display() {
        assert_eq!(
            PromptSource::File("/foo/bar.txt".into()).to_string(),
            "file:/foo/bar.txt"
        );
        assert_eq!(PromptSource::Embedded.to_string(), "embedded");
    }
}
