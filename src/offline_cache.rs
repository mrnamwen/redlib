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

fn cache_file(path: &str) -> Option<PathBuf> {
	if !path.contains(".json") {
		return None;
	}

	let root = cache_root()?;
	Some(root.join(format!("{}.json", URL_SAFE_NO_PAD.encode(path))))
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
