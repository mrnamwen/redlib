use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use log::warn;
use std::{
	fs,
	io::ErrorKind,
	path::PathBuf,
	sync::{LazyLock, Mutex},
	time::Duration,
};

const DEFAULT_JSON_CACHE_MAX_AGE_SECS: u64 = 86_400;

static JSON_CACHE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn cache_root() -> Option<PathBuf> {
	crate::config::get_setting("REDLIB_JSON_CACHE_PATH")
		.map(|value| value.trim().to_string())
		.filter(|value| !value.is_empty())
		.map(PathBuf::from)
}

fn cache_max_age() -> Duration {
	crate::config::get_setting("REDLIB_JSON_CACHE_MAX_AGE_SECS")
		.and_then(|value| value.parse::<u64>().ok())
		.map(Duration::from_secs)
		.unwrap_or(Duration::from_secs(DEFAULT_JSON_CACHE_MAX_AGE_SECS))
}

/// Maximum length of the base64 cache key before it is shortened. Filesystem
/// name limits are 255 bytes, and `.json`/`.tmp` suffixes must still fit.
const MAX_CACHE_KEY_LEN: usize = 160;

/// FNV-1a, used only to keep long cache keys unique after shortening.
fn fnv1a(bytes: &[u8]) -> u64 {
	let mut hash = 0xcbf2_9ce4_8422_2325_u64;
	for byte in bytes {
		hash ^= u64::from(*byte);
		hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
	}
	hash
}

fn cache_file(path: &str) -> Option<PathBuf> {
	if !path.contains(".json") {
		return None;
	}

	let root = cache_root()?;
	let encoded = URL_SAFE_NO_PAD.encode(path);
	let name = if encoded.len() <= MAX_CACHE_KEY_LEN {
		format!("{encoded}.json")
	} else {
		// Long multireddit/subscription paths would exceed the filesystem's
		// filename limit, so keep a short prefix and a stable digest.
		format!("{}-{:016x}.json", &encoded[..MAX_CACHE_KEY_LEN - 24], fnv1a(path.as_bytes()))
	};
	Some(root.join(name))
}

pub fn load_json(path: &str) -> Option<String> {
	let file = cache_file(path)?;
	let max_age = cache_max_age();
	let _guard = JSON_CACHE_LOCK.lock().ok()?;

	let metadata = match fs::metadata(&file) {
		Ok(metadata) => metadata,
		Err(err) if err.kind() == ErrorKind::NotFound => return None,
		Err(err) => {
			warn!("Failed to stat cached Reddit JSON {}: {}", file.display(), err);
			return None;
		}
	};

	let modified = match metadata.modified() {
		Ok(modified) => modified,
		Err(err) => {
			warn!("Failed to read cache timestamp {}: {}", file.display(), err);
			return None;
		}
	};

	let age = match modified.elapsed() {
		Ok(age) => age,
		Err(err) => {
			warn!("Failed to compute cache age {}: {}", file.display(), err);
			return None;
		}
	};

	if age > max_age {
		return None;
	}

	match fs::read_to_string(&file) {
		Ok(body) => Some(body),
		Err(err) => {
			warn!("Failed to read cached Reddit JSON {}: {}", file.display(), err);
			None
		}
	}
}

pub fn store_json(path: &str, body: &str) {
	let Some(file) = cache_file(path) else {
		return;
	};

	let _guard = match JSON_CACHE_LOCK.lock() {
		Ok(guard) => guard,
		Err(_) => {
			warn!("Failed to lock JSON cache for {}", file.display());
			return;
		}
	};

	if let Some(parent) = file.parent() {
		if let Err(err) = fs::create_dir_all(parent) {
			warn!("Failed to create JSON cache directory {}: {}", parent.display(), err);
			return;
		}
	}

	let temp_path = file.with_extension("tmp");
	if let Err(err) = fs::write(&temp_path, body) {
		warn!("Failed to write cached Reddit JSON {}: {}", temp_path.display(), err);
		return;
	}

	if let Err(err) = fs::rename(&temp_path, &file) {
		warn!("Failed to replace cached Reddit JSON {} with {}: {}", file.display(), temp_path.display(), err);
	}
}

#[cfg(test)]
use sealed_test::prelude::*;

#[test]
#[sealed_test(env = [("REDLIB_JSON_CACHE_PATH", "/tmp/redlib-json-cache-test"), ("REDLIB_JSON_CACHE_MAX_AGE_SECS", "60")])]
fn test_json_cache_round_trip() {
	let path = "/r/rust/hot.json?raw_json=1";
	let body = "{\"data\":{\"children\":[]}}";

	store_json(path, body);

	assert_eq!(load_json(path), Some(body.to_string()));
}

#[test]
#[sealed_test(env = [("REDLIB_JSON_CACHE_PATH", "/tmp/redlib-json-cache-long-test"), ("REDLIB_JSON_CACHE_MAX_AGE_SECS", "60")])]
fn test_long_json_cache_path_round_trip() {
	let path = format!("/r/{}/hot.json?&raw_json=1", vec!["averylongsubredditname"; 40].join("+"));
	let body = "{\"data\":{\"children\":[]}}";

	store_json(&path, body);

	assert_eq!(load_json(&path), Some(body.to_string()));
	assert!(cache_file(&path).unwrap().file_name().unwrap().len() <= 255);
	assert_ne!(cache_file(&path), cache_file(&format!("{path}2")));
}
