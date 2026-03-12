use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::{DirEntry, WalkDir};

const NOISE_FILES: [&str; 3] = [".DS_Store", "Thumbs.db", "desktop.ini"];
const NOISE_DIRS: [&str; 2] = [".git", ".skilldeck-sync-snapshots"];

fn is_noise_file(name: &str) -> bool {
    NOISE_FILES.iter().any(|n| n.eq_ignore_ascii_case(name))
}

fn is_noise_dir(name: &str) -> bool {
    NOISE_DIRS.iter().any(|n| n.eq_ignore_ascii_case(name))
}

fn should_walk_entry(entry: &DirEntry) -> bool {
    let name = entry.file_name().to_string_lossy();
    if entry.file_type().is_dir() && is_noise_dir(&name) {
        return false;
    }
    true
}

fn normalize_relative_path(path: &Path) -> String {
    path.components()
        .map(|c| c.as_os_str().to_string_lossy().replace('\\', "/"))
        .collect::<Vec<_>>()
        .join("/")
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    digest.iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn collect_file_hashes(root: &Path) -> Result<BTreeMap<String, String>, String> {
    let mut files: BTreeMap<String, String> = BTreeMap::new();
    if !root.exists() || !root.is_dir() {
        return Ok(files);
    }

    let root = root.canonicalize().unwrap_or_else(|_| PathBuf::from(root));

    for entry in WalkDir::new(&root)
        .follow_links(false)
        .into_iter()
        .filter_entry(should_walk_entry)
    {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().is_file() {
            continue;
        }

        let name = entry.file_name().to_string_lossy();
        if is_noise_file(&name) {
            continue;
        }

        let rel = entry
            .path()
            .strip_prefix(&root)
            .map_err(|e| e.to_string())?;
        let rel_norm = normalize_relative_path(rel);
        let content = fs::read(entry.path()).map_err(|e| e.to_string())?;
        files.insert(rel_norm, hash_bytes(&content));
    }

    Ok(files)
}

pub fn compute_tree_hash(root: &Path) -> Result<String, String> {
    let file_hashes = collect_file_hashes(root)?;
    let mut manifest = String::new();
    for (path, hash) in file_hashes {
        manifest.push_str(&path);
        manifest.push(':');
        manifest.push_str(&hash);
        manifest.push('\n');
    }
    Ok(hash_bytes(manifest.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_noise_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(temp.path().join("a.txt"), b"alpha").expect("write a");
        fs::write(temp.path().join(".DS_Store"), b"noise").expect("write noise");
        let hash_with_noise = compute_tree_hash(temp.path()).expect("hash with noise");

        fs::remove_file(temp.path().join(".DS_Store")).expect("remove noise");
        let hash_without_noise = compute_tree_hash(temp.path()).expect("hash without noise");

        assert_eq!(hash_with_noise, hash_without_noise);
    }

    #[test]
    fn stable_for_same_content() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(temp.path().join("scripts")).expect("mkdir scripts");
        fs::write(temp.path().join("SKILL.md"), b"# hi").expect("write skill");
        fs::write(temp.path().join("scripts").join("tool.sh"), b"echo hi").expect("write script");
        let hash1 = compute_tree_hash(temp.path()).expect("hash1");
        let hash2 = compute_tree_hash(temp.path()).expect("hash2");
        assert_eq!(hash1, hash2);
    }
}
