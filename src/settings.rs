#![allow(clippy::cmp_owned)]

use std::collections::HashMap;

// CRATES
use crate::server::ResponseExt;
use crate::subreddit::join_until_size_limit;
use crate::utils::{current_persistent_preferences, deflate_decompress, persistent_preferences_enabled, redirect, save_persistent_preferences, template, Preferences};
use askama::Template;
use cookie::Cookie;
use futures_lite::StreamExt;
use hyper::{Body, Request, Response};
use time::{Duration, OffsetDateTime};
use tokio::time::timeout;
use url::form_urlencoded;

// STRUCTS
#[derive(Template)]
#[template(path = "settings.html")]
struct SettingsTemplate {
	prefs: Preferences,
	url: String,
	persistent_preferences_enabled: bool,
}

// CONSTANTS

const PREFS: [&str; 19] = [
	"theme",
	"front_page",
	"layout",
	"wide",
	"comment_sort",
	"post_sort",
	"blur_spoiler",
	"show_nsfw",
	"blur_nsfw",
	"use_hls",
	"hide_hls_notification",
	"autoplay_videos",
	"hide_sidebar_and_summary",
	"fixed_navbar",
	"hide_awards",
	"hide_score",
	"disable_visit_reddit_confirmation",
	"video_quality",
	"remove_default_feeds",
];

// FUNCTIONS

/// Retrieve cookies from request "Cookie" header
pub async fn get(req: Request<Body>) -> Result<Response<Body>, String> {
	let url = req.uri().to_string();
	Ok(template(&SettingsTemplate {
		prefs: Preferences::new(&req),
		url,
		persistent_preferences_enabled: persistent_preferences_enabled(),
	}))
}

/// Set cookies using response "Set-Cookie" header
pub async fn set(req: Request<Body>) -> Result<Response<Body>, String> {
	// Split the body into parts
	let (parts, mut body) = req.into_parts();
	let req = Request::from_parts(parts, Body::empty());

	// Aggregate the body...
	// let whole_body = hyper::body::aggregate(req).await.map_err(|e| e.to_string())?;
	let body_bytes = body
		.try_fold(Vec::new(), |mut data, chunk| {
			data.extend_from_slice(&chunk);
			Ok(data)
		})
		.await
		.map_err(|e| e.to_string())?;

	let form = url::form_urlencoded::parse(&body_bytes).collect::<HashMap<_, _>>();

	let mut response = redirect("/settings");
	let mut persistent_preferences = current_persistent_preferences(&req);

	for &name in &PREFS {
		match form.get(name) {
			Some(value) => response.insert_cookie(
				Cookie::build((name.to_owned(), value.clone()))
					.path("/")
					.http_only(true)
					.expires(OffsetDateTime::now_utc() + Duration::weeks(52))
					.into(),
			),
			None => response.remove_cookie(name.to_string()),
		};

		match form.get(name) {
			Some(value) => {
				persistent_preferences.insert(name.to_string(), value.to_string());
			}
			None => {
				persistent_preferences.remove(name);
			}
		}
	}

	save_persistent_preferences(&persistent_preferences)?;

	Ok(response)
}

fn set_cookies_method(req: Request<Body>, remove_cookies: bool) -> Result<Response<Body>, String> {
	// Split the body into parts
	let (parts, _) = req.into_parts();

	let query = parts.uri.query().unwrap_or_default().to_string();
	let form = url::form_urlencoded::parse(query.as_bytes()).collect::<HashMap<_, _>>();
	let cookies_string = parts
		.headers
		.get("cookie")
		.map(|hv| hv.to_str().unwrap_or("").to_string()) // Return String
		.unwrap_or_else(String::new); // Return an empty string if None
	let req = Request::from_parts(parts, Body::empty());
	let mut persistent_preferences = current_persistent_preferences(&req);

	let path = match form.get("redirect") {
		Some(value) => {
			let value = value.replace("%26", "&").replace("%23", "#");
			if value.starts_with('/') {
				value
			} else {
				format!("/{value}")
			}
		}
		None => "/".to_string(),
	};

	let mut response = redirect(&path);

	for name in PREFS {
		match form.get(name) {
			Some(value) => response.insert_cookie(
				Cookie::build((name.to_owned(), value.clone()))
					.path("/")
					.http_only(true)
					.expires(OffsetDateTime::now_utc() + Duration::weeks(52))
					.into(),
			),
			None => {
				if remove_cookies {
					response.remove_cookie(name.to_string());
				}
			}
		};

		match form.get(name) {
			Some(value) => {
				persistent_preferences.insert(name.to_string(), value.to_string());
			}
			None => {
				if remove_cookies {
					persistent_preferences.remove(name);
				}
			}
		}
	}

	// Get subscriptions/filters to restore from query string
	let subscriptions = form.get("subscriptions");
	let filters = form.get("filters");

	// If there are subscriptions to restore set them and delete any old subscriptions cookies, otherwise delete them all
	if let Some(subscriptions) = subscriptions {
		let sub_list: Vec<String> = subscriptions.split('+').map(str::to_string).collect();

		// Start at 0 to keep track of what number we need to start deleting old subscription cookies from
		let mut subscriptions_number_to_delete_from = 0;

		// Starting at 0 so we handle the subscription cookie without a number first
		for (subscriptions_number, list) in join_until_size_limit(&sub_list).into_iter().enumerate() {
			let subscriptions_cookie = if subscriptions_number == 0 {
				"subscriptions".to_string()
			} else {
				format!("subscriptions{subscriptions_number}")
			};

			response.insert_cookie(
				Cookie::build((subscriptions_cookie, list))
					.path("/")
					.http_only(true)
					.expires(OffsetDateTime::now_utc() + Duration::weeks(52))
					.into(),
			);

			subscriptions_number_to_delete_from += 1;
		}

		// While subscriptionsNUMBER= is in the string of cookies add a response removing that cookie
		while cookies_string.contains(&format!("subscriptions{subscriptions_number_to_delete_from}=")) {
			// Remove that subscriptions cookie
			response.remove_cookie(format!("subscriptions{subscriptions_number_to_delete_from}"));

			// Increment subscriptions cookie number
			subscriptions_number_to_delete_from += 1;
		}

		if subscriptions.is_empty() {
			persistent_preferences.remove("subscriptions");
		} else {
			persistent_preferences.insert("subscriptions".to_string(), subscriptions.to_string());
		}
	} else {
		if remove_cookies {
			// Remove unnumbered subscriptions cookie
			response.remove_cookie("subscriptions".to_string());

			// Starts at one to deal with the first numbered subscription cookie and onwards
			let mut subscriptions_number_to_delete_from = 1;

			// While subscriptionsNUMBER= is in the string of cookies add a response removing that cookie
			while cookies_string.contains(&format!("subscriptions{subscriptions_number_to_delete_from}=")) {
				// Remove that subscriptions cookie
				response.remove_cookie(format!("subscriptions{subscriptions_number_to_delete_from}"));

				// Increment subscriptions cookie number
				subscriptions_number_to_delete_from += 1;
			}

			persistent_preferences.remove("subscriptions");
		}
	}

	// If there are filters to restore set them and delete any old filters cookies, otherwise delete them all
	if let Some(filters) = filters {
		let filters_list: Vec<String> = filters.split('+').map(str::to_string).collect();

		// Start at 0 to keep track of what number we need to start deleting old subscription cookies from
		let mut filters_number_to_delete_from = 0;

		// Starting at 0 so we handle the subscription cookie without a number first
		for (filters_number, list) in join_until_size_limit(&filters_list).into_iter().enumerate() {
			let filters_cookie = if filters_number == 0 {
				"filters".to_string()
			} else {
				format!("filters{filters_number}")
			};

			response.insert_cookie(
				Cookie::build((filters_cookie, list))
					.path("/")
					.http_only(true)
					.expires(OffsetDateTime::now_utc() + Duration::weeks(52))
					.into(),
			);

			filters_number_to_delete_from += 1;
		}

		// While filtersNUMBER= is in the string of cookies add a response removing that cookie
		while cookies_string.contains(&format!("filters{filters_number_to_delete_from}=")) {
			// Remove that filters cookie
			response.remove_cookie(format!("filters{filters_number_to_delete_from}"));

			// Increment filters cookie number
			filters_number_to_delete_from += 1;
		}

		if filters.is_empty() {
			persistent_preferences.remove("filters");
		} else {
			persistent_preferences.insert("filters".to_string(), filters.to_string());
		}
	} else {
		if remove_cookies {
			// Remove unnumbered filters cookie
			response.remove_cookie("filters".to_string());

			// Starts at one to deal with the first numbered subscription cookie and onwards
			let mut filters_number_to_delete_from = 1;

			// While filtersNUMBER= is in the string of cookies add a response removing that cookie
			while cookies_string.contains(&format!("filters{filters_number_to_delete_from}=")) {
				// Remove that sfilters cookie
				response.remove_cookie(format!("filters{filters_number_to_delete_from}"));

				// Increment filters cookie number
				filters_number_to_delete_from += 1;
			}

			persistent_preferences.remove("filters");
		}
	}

	save_persistent_preferences(&persistent_preferences)?;

	Ok(response)
}

/// Set cookies using response "Set-Cookie" header
pub async fn restore(req: Request<Body>) -> Result<Response<Body>, String> {
	set_cookies_method(req, true)
}

pub async fn update(req: Request<Body>) -> Result<Response<Body>, String> {
	set_cookies_method(req, false)
}

pub async fn encoded_restore(req: Request<Body>) -> Result<Response<Body>, String> {
	let body = hyper::body::to_bytes(req.into_body())
		.await
		.map_err(|e| format!("Failed to get bytes from request body: {e}"))?;

	if body.len() > 1024 * 1024 {
		return Err("Request body too large".to_string());
	}

	let encoded_prefs = form_urlencoded::parse(&body)
		.find(|(key, _)| key == "encoded_prefs")
		.map(|(_, value)| value)
		.ok_or_else(|| "encoded_prefs parameter not found in request body".to_string())?;

	let bytes = base2048::decode(&encoded_prefs).ok_or_else(|| "Failed to decode base2048 encoded preferences".to_string())?;

	let out = timeout(std::time::Duration::from_secs(1), async { deflate_decompress(bytes) })
		.await
		.map_err(|e| format!("Failed to decompress bytes: {e}"))??;

	let mut prefs: Preferences = timeout(std::time::Duration::from_secs(1), async { bincode::deserialize(&out) })
		.await
		.map_err(|e| format!("Failed to deserialize preferences: {e}"))?
		.map_err(|e| format!("Failed to deserialize bytes into Preferences struct: {e}"))?;

	prefs.available_themes = vec![];

	let url = format!("/settings/restore/?{}", prefs.to_urlencoded()?);

	Ok(redirect(&url))
}
