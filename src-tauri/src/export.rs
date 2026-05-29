//! Export synthesised audio to the filesystem.
//!
//! Writes:
//! - `{out_dir}/{title}_full.mp3` — the complete concatenated recording
//! - `{out_dir}/{filename}` — one file per part (filenames supplied by caller)

use std::fs;
use std::path::Path;

/// Write the full MP3 and per-part MP3s to `out_dir`.
///
/// Returns the list of absolute paths that were written.
///
/// # Errors
/// Returns an `Err` string if `out_dir` cannot be created or any file write
/// fails.
pub fn save_audio(
    full: &[u8],
    parts: &[(String, Vec<u8>)],
    out_dir: &str,
    title: &str,
) -> Result<Vec<String>, String> {
    let dir = Path::new(out_dir);
    fs::create_dir_all(dir)
        .map_err(|e| format!("create output dir '{}': {e}", dir.display()))?;

    let mut written: Vec<String> = Vec::new();

    // Full MP3.
    let full_filename = format!("{}_full.mp3", sanitize_title(title));
    let full_path = dir.join(&full_filename);
    fs::write(&full_path, full)
        .map_err(|e| format!("write '{}': {e}", full_path.display()))?;
    written.push(full_path.to_string_lossy().to_string());

    // Per-part MP3s.
    for (filename, bytes) in parts {
        let part_path = dir.join(filename);
        fs::write(&part_path, bytes)
            .map_err(|e| format!("write '{}': {e}", part_path.display()))?;
        written.push(part_path.to_string_lossy().to_string());
    }

    Ok(written)
}

/// Sanitize a project title into a safe filename component.
fn sanitize_title(title: &str) -> String {
    let raw: String = title
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let collapsed = raw
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    if collapsed.is_empty() {
        "project".to_string()
    } else {
        collapsed.chars().take(60).collect()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn save_audio_writes_files() {
        let dir = tempdir().expect("tempdir");
        let out_dir = dir.path().to_string_lossy().to_string();

        let full: Vec<u8> = vec![0xFF, 0xFB, 0x90, 0x00]; // fake mp3 header
        let parts: Vec<(String, Vec<u8>)> = vec![
            ("01_Part_One.mp3".to_string(), vec![0xFF, 0xFB, 0x90, 0x01]),
            ("02_Part_Two.mp3".to_string(), vec![0xFF, 0xFB, 0x90, 0x02]),
        ];

        let written = save_audio(&full, &parts, &out_dir, "MyProject")
            .expect("save_audio failed");

        assert_eq!(written.len(), 3, "should write 3 files");

        // Full MP3 must exist.
        assert!(
            written[0].contains("MyProject_full.mp3"),
            "full path: {}",
            written[0]
        );
        assert!(
            std::path::Path::new(&written[0]).exists(),
            "full mp3 missing: {}",
            written[0]
        );

        // Parts must exist.
        for path in &written[1..] {
            assert!(
                std::path::Path::new(path).exists(),
                "part mp3 missing: {path}"
            );
        }
    }

    #[test]
    fn save_audio_creates_missing_dir() {
        let dir = tempdir().expect("tempdir");
        let nested = dir.path().join("audio").join("output");
        let out_dir = nested.to_string_lossy().to_string();

        let result = save_audio(&[0u8], &[], &out_dir, "Test");
        assert!(result.is_ok(), "should create nested dirs: {:?}", result);
    }

    #[test]
    fn sanitize_title_basic() {
        assert_eq!(sanitize_title("My Project!"), "My_Project");
        assert_eq!(sanitize_title("Unit-2_Test"), "Unit-2_Test");
    }

    #[test]
    fn sanitize_title_empty_fallback() {
        assert_eq!(sanitize_title(""), "project");
        assert_eq!(sanitize_title("___"), "project");
    }

    #[test]
    fn sanitize_title_truncates() {
        let long = "A".repeat(100);
        let result = sanitize_title(&long);
        assert!(result.len() <= 60, "should truncate to 60: {}", result.len());
    }
}
