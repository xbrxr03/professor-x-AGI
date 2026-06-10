use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};
use tokio::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EditVerification {
    pub check: String,
    pub accepted: bool,
    pub reason: String,
}

impl EditVerification {
    fn accepted(check: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            check: check.into(),
            accepted: true,
            reason: reason.into(),
        }
    }

    fn rejected(check: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            check: check.into(),
            accepted: false,
            reason: reason.into(),
        }
    }
}

#[derive(Debug, Clone)]
struct ExistingFile {
    path: PathBuf,
    content: Option<Vec<u8>>,
}

impl ExistingFile {
    fn capture(path: &Path) -> Result<Self> {
        let content = match std::fs::read(path) {
            Ok(bytes) => Some(bytes),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
            Err(err) => return Err(err).with_context(|| format!("read {}", path.display())),
        };
        Ok(Self {
            path: path.to_path_buf(),
            content,
        })
    }

    fn restore(&self) -> Result<()> {
        match &self.content {
            Some(content) => {
                if let Some(parent) = self.path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&self.path, content)?;
            }
            None => match std::fs::remove_file(&self.path) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => {
                    return Err(err).with_context(|| format!("remove {}", self.path.display()))
                }
            },
        }
        Ok(())
    }
}

pub async fn verify_candidate_content(
    workspace_root: &Path,
    path: &Path,
    candidate: &str,
) -> Result<EditVerification> {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("json") => match serde_json::from_str::<serde_json::Value>(candidate) {
            Ok(_) => Ok(EditVerification::accepted("json_parse", "JSON parsed")),
            Err(err) => Ok(EditVerification::rejected("json_parse", err.to_string())),
        },
        Some("toml") => match toml::from_str::<toml::Value>(candidate) {
            Ok(_) => Ok(EditVerification::accepted("toml_parse", "TOML parsed")),
            Err(err) => Ok(EditVerification::rejected("toml_parse", err.to_string())),
        },
        Some("py") => {
            verify_by_transient_write(
                workspace_root,
                path,
                candidate,
                "python_py_compile",
                python_compile_at,
            )
            .await
        }
        Some("rs") => {
            if let Some(cargo_root) = nearest_cargo_root(workspace_root, path) {
                verify_by_transient_write(
                    workspace_root,
                    path,
                    candidate,
                    "cargo_check",
                    move |_| cargo_check_at(cargo_root.clone()),
                )
                .await
            } else {
                verify_rust_standalone(path, candidate).await
            }
        }
        _ => Ok(EditVerification::accepted(
            "verification_skipped",
            "no syntax verifier for file type",
        )),
    }
}

async fn verify_by_transient_write<F, Fut>(
    _workspace_root: &Path,
    path: &Path,
    candidate: &str,
    check: &str,
    run_check: F,
) -> Result<EditVerification>
where
    F: FnOnce(PathBuf) -> Fut,
    Fut: std::future::Future<Output = Result<EditVerification>>,
{
    let original = ExistingFile::capture(path)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, candidate)?;
    let result = run_check(path.to_path_buf()).await;
    original.restore()?;
    match result {
        Ok(verification) => Ok(verification),
        Err(err) => Ok(EditVerification::rejected(check, err.to_string())),
    }
}

async fn cargo_check_at(cargo_root: PathBuf) -> Result<EditVerification> {
    let output = Command::new("cargo")
        .args(["check", "--quiet"])
        .current_dir(&cargo_root)
        .output()
        .await
        .with_context(|| format!("run cargo check in {}", cargo_root.display()))?;
    if output.status.success() {
        return Ok(EditVerification::accepted(
            "cargo_check",
            "cargo check passed",
        ));
    }
    bail!(
        "cargo check failed: {}",
        first_lines(&String::from_utf8_lossy(&output.stderr), 10)
    );
}

async fn python_compile_at(path: PathBuf) -> Result<EditVerification> {
    let output = Command::new("python3")
        .args(["-m", "py_compile"])
        .arg(&path)
        .output()
        .await
        .with_context(|| format!("run python py_compile on {}", path.display()))?;
    if output.status.success() {
        return Ok(EditVerification::accepted(
            "python_py_compile",
            "python py_compile passed",
        ));
    }
    bail!(
        "python py_compile failed: {}",
        first_lines(&String::from_utf8_lossy(&output.stderr), 10)
    );
}

async fn verify_rust_standalone(path: &Path, candidate: &str) -> Result<EditVerification> {
    let temp_dir = std::env::temp_dir().join(format!("px-rust-verify-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("candidate.rs");
    let temp_file = temp_dir.join(file_name);
    std::fs::write(&temp_file, candidate)?;
    let output = Command::new("rustc")
        .args(["--crate-type", "lib", "--emit", "metadata"])
        .arg(&temp_file)
        .arg("-o")
        .arg(temp_dir.join("candidate.rmeta"))
        .output()
        .await
        .with_context(|| format!("run rustc syntax check on {}", path.display()))?;
    let _ = std::fs::remove_dir_all(&temp_dir);
    if output.status.success() {
        return Ok(EditVerification::accepted(
            "rustc_standalone",
            "rustc standalone check passed",
        ));
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !looks_like_rust_syntax_error(&stderr) {
        return Ok(EditVerification::accepted(
            "rustc_standalone",
            "rustc reached semantic checks; no syntax-class error detected",
        ));
    }
    Ok(EditVerification::rejected(
        "rustc_standalone",
        format!(
            "rustc standalone check failed: {}",
            first_lines(&stderr, 10)
        ),
    ))
}

fn nearest_cargo_root(workspace_root: &Path, path: &Path) -> Option<PathBuf> {
    let mut dir = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent()?.to_path_buf()
    };
    loop {
        if dir.join("Cargo.toml").exists() {
            return Some(dir);
        }
        if dir == workspace_root || !dir.pop() {
            break;
        }
    }

    let nested = workspace_root.join("professor-x");
    if nested.join("Cargo.toml").exists() {
        Some(nested)
    } else if workspace_root.join("Cargo.toml").exists() {
        Some(workspace_root.to_path_buf())
    } else {
        None
    }
}

fn first_lines(text: &str, max_lines: usize) -> String {
    let summary = text.lines().take(max_lines).collect::<Vec<_>>().join(" ");
    if summary.trim().is_empty() {
        "<no stderr>".to_string()
    } else {
        summary
    }
}

fn looks_like_rust_syntax_error(stderr: &str) -> bool {
    let stderr = stderr.to_lowercase();
    [
        "expected one of",
        "expected identifier",
        "expected expression",
        "expected parameter name",
        "expected pattern",
        "expected item",
        "unclosed delimiter",
        "mismatched closing delimiter",
        "unexpected closing delimiter",
        "this file contains an unclosed delimiter",
    ]
    .iter()
    .any(|needle| stderr.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn json_candidates_are_parsed_without_touching_disk() {
        let root = std::env::temp_dir().join(format!("px-json-verify-{}", uuid::Uuid::new_v4()));
        let path = root.join("data.json");
        let ok = verify_candidate_content(&root, &path, "{\"ok\":true}")
            .await
            .unwrap();
        assert!(ok.accepted);

        let err = verify_candidate_content(&root, &path, "{\"ok\":")
            .await
            .unwrap();
        assert!(!err.accepted);
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn standalone_rust_rejects_syntax_errors_without_writing_target() {
        let root = std::env::temp_dir().join(format!("px-rust-verify-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let path = root.join("src/lib.rs");
        std::fs::write(&path, "pub fn x() {}\n").unwrap();

        let verification = verify_candidate_content(&root, &path, "pub fn x( {\n")
            .await
            .unwrap();
        assert!(!verification.accepted);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "pub fn x() {}\n");

        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn standalone_rust_accepts_semantic_errors_after_parse() {
        let root = std::env::temp_dir().join(format!("px-rust-verify-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let path = root.join("src/lib.rs");
        std::fs::write(&path, "pub fn x() {}\n").unwrap();

        let verification = verify_candidate_content(&root, &path, "pub fn x() { 1 }\n")
            .await
            .unwrap();
        assert!(verification.accepted, "{verification:?}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "pub fn x() {}\n");

        let _ = std::fs::remove_dir_all(root);
    }
}
