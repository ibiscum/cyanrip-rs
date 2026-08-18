use async_trait::async_trait;

use crate::CoverArtLookupSize;

const COVERART_DB_URL_BASE: &str = "http://coverartarchive.org/release";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverArtImage {
	pub title: String,
	pub source: Option<String>,
	pub source_url: String,
	pub extension: Option<String>,
	pub data: Option<Vec<u8>>,
	pub content_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoverArtError {
	Http(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoverArtFetchStatus {
	Downloaded,
	NotFound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverArtResponse {
	pub status_code: u16,
	pub content_type: Option<String>,
	pub final_url: String,
	pub body: Vec<u8>,
}

#[async_trait]
pub trait CoverArtHttpClient: Send + Sync {
	async fn get(&self, url: &str, user_agent: &str) -> Result<CoverArtResponse, CoverArtError>;
}

#[derive(Debug, Clone)]
pub struct ReqwestCoverArtHttpClient {
	client: reqwest::Client,
}

impl Default for ReqwestCoverArtHttpClient {
	fn default() -> Self {
		Self {
			client: reqwest::Client::new(),
		}
	}
}

#[async_trait]
impl CoverArtHttpClient for ReqwestCoverArtHttpClient {
	async fn get(&self, url: &str, user_agent: &str) -> Result<CoverArtResponse, CoverArtError> {
		let resp = self
			.client
			.get(url)
			.header(reqwest::header::USER_AGENT, user_agent)
			.send()
			.await
			.map_err(|e| CoverArtError::Http(e.to_string()))?;

		let status_code = resp.status().as_u16();
		let content_type = resp
			.headers()
			.get(reqwest::header::CONTENT_TYPE)
			.and_then(|v| v.to_str().ok())
			.map(ToString::to_string);
		let final_url = resp.url().to_string();
		let body = resp
			.bytes()
			.await
			.map_err(|e| CoverArtError::Http(e.to_string()))?
			.to_vec();

		Ok(CoverArtResponse {
			status_code,
			content_type,
			final_url,
			body,
		})
	}
}

#[derive(Debug, Clone)]
pub struct CoverArtService<H: CoverArtHttpClient> {
	http: H,
	base_url: String,
	user_agent: String,
}

impl<H: CoverArtHttpClient> CoverArtService<H> {
	pub fn new(http: H, base_url: impl Into<String>, user_agent: impl Into<String>) -> Self {
		Self {
			http,
			base_url: base_url.into().trim_end_matches('/').to_string(),
			user_agent: user_agent.into(),
		}
	}

	pub async fn fill_release_coverart(
		&self,
		cover_arts: &mut Vec<CoverArtImage>,
		release_id: Option<&str>,
		disable_coverart_db: bool,
		lookup_size: CoverArtLookupSize,
		info_only: bool,
	) -> Result<(), CoverArtError> {
		let have_front = cover_arts.iter().any(|c| c.title == "Front");
		let have_back = cover_arts.iter().any(|c| c.title == "Back");

		if !have_front || !have_back {
			if !disable_coverart_db {
				if let Some(release_id) = release_id {
					let mut has_err = 0i32;

					if !have_front {
						let front_id = cover_id("front", lookup_size);
						if self
							.fetch_coverart_db_art(cover_arts, release_id, "Front", &front_id, info_only)
							.await?
						{
							has_err = 1;
						}
					}

					// Preserve C behavior: back lookup only runs if front lookup was attempted and succeeded.
					if !have_back && has_err > 0 {
						let back_id = cover_id("back", lookup_size);
						self.fetch_coverart_db_art(cover_arts, release_id, "Back", &back_id, info_only)
							.await?;
					}
				}
			}
		}

		for art in cover_arts.iter_mut() {
			let is_url = string_is_url(&art.source_url);
			if is_url && art.data.is_none() && !info_only {
				let source_url = art.source_url.clone();
				let status = self.fetch_url_into_art(art, &source_url, info_only).await?;
				if matches!(status, CoverArtFetchStatus::NotFound) {
					return Ok(());
				}
			}
		}

		Ok(())
	}

	async fn fetch_coverart_db_art(
		&self,
		cover_arts: &mut Vec<CoverArtImage>,
		release_id: &str,
		title: &str,
		type_id: &str,
		info_only: bool,
	) -> Result<bool, CoverArtError> {
		let url = format!("{}/{}/{}", self.base_url, release_id, type_id);
		let mut art = CoverArtImage {
			title: title.to_string(),
			source: Some("Cover Art DB".to_string()),
			source_url: url.clone(),
			extension: Some("jpg".to_string()),
			data: None,
			content_type: None,
		};

		let status = self.fetch_url_into_art(&mut art, &url, info_only).await?;
		if matches!(status, CoverArtFetchStatus::Downloaded) {
			cover_arts.push(art);
			return Ok(true);
		}

		Ok(false)
	}

	async fn fetch_url_into_art(
		&self,
		art: &mut CoverArtImage,
		url: &str,
		info_only: bool,
	) -> Result<CoverArtFetchStatus, CoverArtError> {
		let resp = self.http.get(url, &self.user_agent).await?;
		if resp.status_code == 404 {
			return Ok(CoverArtFetchStatus::NotFound);
		}
		if !(200..300).contains(&resp.status_code) {
			return Err(CoverArtError::Http(format!(
				"unexpected status {}",
				resp.status_code
			)));
		}

		art.content_type = resp.content_type.clone();
		art.source_url = resp.final_url;
		art.extension = infer_extension(resp.content_type.as_deref()).or_else(|| art.extension.clone());
		art.data = if info_only { None } else { Some(resp.body) };

		Ok(CoverArtFetchStatus::Downloaded)
	}
}

pub fn string_is_url(src: &str) -> bool {
	src.starts_with("http://")
		|| src.starts_with("https://")
		|| src.starts_with("ftp://")
		|| src.starts_with("ftps://")
		|| src.starts_with("sftp://")
		|| src.starts_with("tftp://")
		|| src.starts_with("gopher://")
		|| src.starts_with("telnet://")
}

fn cover_id(base: &str, lookup_size: CoverArtLookupSize) -> String {
	match lookup_size {
		CoverArtLookupSize::Original => base.to_string(),
		CoverArtLookupSize::Px250 => format!("{base}-250"),
		CoverArtLookupSize::Px500 => format!("{base}-500"),
		CoverArtLookupSize::Px1200 => format!("{base}-1200"),
	}
}

fn infer_extension(content_type: Option<&str>) -> Option<String> {
	match content_type {
		Some(v) if v.starts_with("image/jpeg") => Some("jpg".to_string()),
		Some(v) if v.starts_with("image/png") => Some("png".to_string()),
		Some(v) if v.starts_with("image/bmp") => Some("bmp".to_string()),
		Some(v) if v.starts_with("image/tiff") => Some("tiff".to_string()),
		Some(v) if v.starts_with("image/avif") => Some("avif".to_string()),
		Some(v) if v.starts_with("image/heif") => Some("heif".to_string()),
		Some(v) if v.starts_with("image/webp") => Some("webp".to_string()),
		_ => None,
	}
}

impl Default for CoverArtService<ReqwestCoverArtHttpClient> {
	fn default() -> Self {
		Self::new(
			ReqwestCoverArtHttpClient::default(),
			COVERART_DB_URL_BASE,
			"cyanrip-rs/0.1",
		)
	}
}

#[cfg(test)]
mod tests {
	use std::fs;

	use wiremock::matchers::{header, method, path};
	use wiremock::{Mock, MockServer, ResponseTemplate};

	use super::*;

	fn fixture(path: &str) -> Vec<u8> {
		fs::read(format!("tests/fixtures/coverart/{path}")).expect("fixture should exist")
	}

	#[tokio::test]
	async fn fills_missing_front_and_back_from_coverart_db() {
		let server = MockServer::start().await;
		Mock::given(method("GET"))
			.and(path("/release/rel-1/front"))
			.and(header("user-agent", "cyanrip-rs-test/0.1"))
			.respond_with(
				ResponseTemplate::new(200)
					.append_header("content-type", "image/jpeg")
					.set_body_bytes(fixture("front.bin")),
			)
			.mount(&server)
			.await;

		Mock::given(method("GET"))
			.and(path("/release/rel-1/back"))
			.respond_with(
				ResponseTemplate::new(200)
					.append_header("content-type", "image/jpeg")
					.set_body_bytes(fixture("back.bin")),
			)
			.mount(&server)
			.await;

		let mut arts = Vec::new();
		let svc = CoverArtService::new(
			ReqwestCoverArtHttpClient::default(),
			format!("{}/release", server.uri()),
			"cyanrip-rs-test/0.1",
		);

		svc.fill_release_coverart(
			&mut arts,
			Some("rel-1"),
			false,
			CoverArtLookupSize::Original,
			false,
		)
		.await
		.expect("lookup should succeed");

		assert_eq!(arts.len(), 2);
		assert_eq!(arts[0].title, "Front");
		assert_eq!(arts[1].title, "Back");
		assert_eq!(arts[0].source.as_deref(), Some("Cover Art DB"));
		assert_eq!(arts[0].extension.as_deref(), Some("jpg"));
		assert_eq!(arts[0].data.as_deref(), Some(fixture("front.bin").as_slice()));
	}

	#[tokio::test]
	async fn does_not_fetch_back_when_front_missing_fails_like_c() {
		let server = MockServer::start().await;
		Mock::given(method("GET"))
			.and(path("/release/rel-1/front"))
			.respond_with(ResponseTemplate::new(404))
			.mount(&server)
			.await;

		let mut arts = Vec::new();
		let svc = CoverArtService::new(
			ReqwestCoverArtHttpClient::default(),
			format!("{}/release", server.uri()),
			"cyanrip-rs-test/0.1",
		);

		svc.fill_release_coverart(
			&mut arts,
			Some("rel-1"),
			false,
			CoverArtLookupSize::Original,
			false,
		)
		.await
		.expect("not found should be non-fatal");

		assert!(arts.is_empty());
	}

	#[tokio::test]
	async fn does_not_fetch_back_if_front_already_present_like_c() {
		let server = MockServer::start().await;
		let mut arts = vec![CoverArtImage {
			title: "Front".to_string(),
			source: Some("manual".to_string()),
			source_url: "/tmp/front.jpg".to_string(),
			extension: Some("jpg".to_string()),
			data: Some(vec![1, 2, 3]),
			content_type: Some("image/jpeg".to_string()),
		}];

		let svc = CoverArtService::new(
			ReqwestCoverArtHttpClient::default(),
			format!("{}/release", server.uri()),
			"cyanrip-rs-test/0.1",
		);

		svc.fill_release_coverart(
			&mut arts,
			Some("rel-1"),
			false,
			CoverArtLookupSize::Original,
			false,
		)
		.await
		.expect("should succeed");

		assert_eq!(arts.len(), 1);
		assert_eq!(arts[0].title, "Front");
	}

	#[tokio::test]
	async fn hydrates_external_url_when_data_missing() {
		let server = MockServer::start().await;
		Mock::given(method("GET"))
			.and(path("/manual/front.png"))
			.respond_with(
				ResponseTemplate::new(200)
					.append_header("content-type", "image/png")
					.set_body_bytes(fixture("front.bin")),
			)
			.mount(&server)
			.await;

		let mut arts = vec![CoverArtImage {
			title: "Front".to_string(),
			source: Some("manual".to_string()),
			source_url: format!("{}/manual/front.png", server.uri()),
			extension: None,
			data: None,
			content_type: None,
		}];

		let svc = CoverArtService::new(
			ReqwestCoverArtHttpClient::default(),
			format!("{}/release", server.uri()),
			"cyanrip-rs-test/0.1",
		);

		svc.fill_release_coverart(
			&mut arts,
			Some("rel-1"),
			true,
			CoverArtLookupSize::Original,
			false,
		)
		.await
		.expect("url hydration should succeed");

		assert_eq!(arts[0].extension.as_deref(), Some("png"));
		assert_eq!(arts[0].data.as_deref(), Some(fixture("front.bin").as_slice()));
		assert_eq!(arts[0].content_type.as_deref(), Some("image/png"));
	}

	#[test]
	fn url_detection_matches_c_prefixes() {
		assert!(string_is_url("http://a"));
		assert!(string_is_url("https://a"));
		assert!(string_is_url("ftp://a"));
		assert!(string_is_url("ftps://a"));
		assert!(string_is_url("sftp://a"));
		assert!(string_is_url("tftp://a"));
		assert!(string_is_url("gopher://a"));
		assert!(string_is_url("telnet://a"));
		assert!(!string_is_url("/tmp/file.jpg"));
	}
}
