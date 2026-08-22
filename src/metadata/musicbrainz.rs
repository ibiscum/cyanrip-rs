use async_trait::async_trait;
use musicbrainz_rs::entity::artist_credit::ArtistCredit;
use musicbrainz_rs::entity::discid::Discid;
use musicbrainz_rs::entity::release::{Media, Release, ReleasePackaging, ReleaseStatus, Track};

use crate::ReleaseSelection;

const MB_INC: &str = "recordings artist-credits labels";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MusicBrainzTrackMeta {
	pub mbid: Option<String>,
	pub title: String,
	pub artist: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MusicBrainzReleaseMeta {
	pub musicbrainz_albumid: String,
	pub releasecomment: Option<String>,
	pub date: Option<String>,
	pub album: String,
	pub barcode: Option<String>,
	pub packaging: Option<String>,
	pub country: Option<String>,
	pub releasestatus: Option<String>,
	pub catalognumber: Option<String>,
	pub label: Option<String>,
	pub album_artist: Option<String>,
	pub discname: Option<String>,
	pub format: Option<String>,
	pub discnumber: Option<i32>,
	pub totaldiscs: i32,
	pub tracks: Vec<MusicBrainzTrackMeta>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseSummary {
	pub id: String,
	pub album: String,
	pub disambiguation: Option<String>,
	pub country: Option<String>,
	pub date: Option<String>,
	pub num_cds: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MusicBrainzError {
	MissingDiscid,
	Http(String),
	Parse(String),
	NotFound,
	MultipleReleases(Vec<ReleaseSummary>),
	InvalidReleaseIndex { requested: i32, available: i32 },
	ReleaseIdNotFound(String),
	InvalidDiscNumber { requested: i32, available: i32 },
}

#[async_trait]
pub trait MusicBrainzHttpClient: Send + Sync {
	async fn get(&self, url: &str, user_agent: &str) -> Result<String, MusicBrainzError>;
}

#[derive(Debug, Clone)]
pub struct ReqwestMusicBrainzHttpClient {
	client: reqwest::Client,
}

impl Default for ReqwestMusicBrainzHttpClient {
	fn default() -> Self {
		Self {
			client: reqwest::Client::new(),
		}
	}
}

#[async_trait]
impl MusicBrainzHttpClient for ReqwestMusicBrainzHttpClient {
	async fn get(&self, url: &str, user_agent: &str) -> Result<String, MusicBrainzError> {
		let resp = self
			.client
			.get(url)
			.header(reqwest::header::USER_AGENT, user_agent)
			.send()
			.await
			.map_err(|e| MusicBrainzError::Http(e.to_string()))?;

		if !resp.status().is_success() {
			if resp.status().as_u16() == 404 {
				return Err(MusicBrainzError::NotFound);
			}
			return Err(MusicBrainzError::Http(format!(
				"unexpected status {}",
				resp.status()
			)));
		}

		resp.text()
			.await
			.map_err(|e| MusicBrainzError::Http(e.to_string()))
	}
}

#[derive(Debug, Clone)]
pub struct MusicBrainzService<H: MusicBrainzHttpClient> {
	http: H,
	base_url: String,
	user_agent: String,
}

impl<H: MusicBrainzHttpClient> MusicBrainzService<H> {
	pub fn new(http: H, base_url: impl Into<String>, user_agent: impl Into<String>) -> Self {
		Self {
			http,
			base_url: base_url.into().trim_end_matches('/').to_string(),
			user_agent: user_agent.into(),
		}
	}

	pub async fn lookup_release(
		&self,
		discid: &str,
		release_selection: Option<&ReleaseSelection>,
		discnumber: i32,
		nb_cd_tracks: usize,
	) -> Result<MusicBrainzReleaseMeta, MusicBrainzError> {
		if discid.trim().is_empty() {
			return Err(MusicBrainzError::MissingDiscid);
		}

		let url = format!(
			"{}/ws/2/discid/{}?inc={}&fmt=json",
			self.base_url,
			discid,
			MB_INC.replace(' ', "%20")
		);

		let body = self.http.get(&url, &self.user_agent).await?;
		let disc: Discid =
			serde_json::from_str(&body).map_err(|e| MusicBrainzError::Parse(e.to_string()))?;

		let releases = disc.releases.unwrap_or_default();
		if releases.is_empty() {
			return Err(MusicBrainzError::NotFound);
		}

		if releases.len() > 1 && release_selection.is_none() {
			let summaries = releases.iter().map(release_summary).collect();
			return Err(MusicBrainzError::MultipleReleases(summaries));
		}

		let release = pick_release(&releases, release_selection)?;
		map_release(release, discid, discnumber, nb_cd_tracks)
	}
}

fn pick_release<'a>(
	releases: &'a [Release],
	release_selection: Option<&ReleaseSelection>,
) -> Result<&'a Release, MusicBrainzError> {
	match release_selection {
		None => Ok(&releases[0]),
		Some(ReleaseSelection::Index(i)) => {
			if *i < 1 || *i > releases.len() as i32 {
				return Err(MusicBrainzError::InvalidReleaseIndex {
					requested: *i,
					available: releases.len() as i32,
				});
			}
			Ok(&releases[*i as usize - 1])
		}
		Some(ReleaseSelection::Id(id)) => releases
			.iter()
			.find(|r| r.id == *id)
			.ok_or_else(|| MusicBrainzError::ReleaseIdNotFound(id.clone())),
	}
}

fn map_release(
	release: &Release,
	discid: &str,
	discnumber: i32,
	nb_cd_tracks: usize,
) -> Result<MusicBrainzReleaseMeta, MusicBrainzError> {
	let media = release.media.clone().unwrap_or_default();
	let totaldiscs = media.len() as i32;

	let (selected_medium, selected_disc_number) = pick_medium(&media, discid, discnumber)?;

	let (discname, format, tracks, inferred_discnumber) = if let Some(medium) = selected_medium {
		(
			medium.title.clone(),
			medium.format.clone(),
			map_tracks(medium, nb_cd_tracks),
			medium.position.map(|p| p as i32),
		)
	} else {
		(None, None, Vec::new(), None)
	};

	let (label, catalognumber) = first_label_info(release);

	Ok(MusicBrainzReleaseMeta {
		musicbrainz_albumid: release.id.clone(),
		releasecomment: release.disambiguation.clone(),
		date: release.date.clone().map(|d| d.0),
		album: release.title.clone(),
		barcode: release.barcode.clone(),
		packaging: release.packaging.as_ref().map(release_packaging_name),
		country: release.country.clone(),
		releasestatus: release.status.as_ref().map(release_status_name),
		catalognumber,
		label,
		album_artist: release
			.artist_credit
			.as_ref()
			.map(|ac| join_artist_credit(ac))
			.filter(|s| !s.is_empty()),
		discname,
		format,
		discnumber: selected_disc_number.or(inferred_discnumber),
		totaldiscs,
		tracks,
	})
}

fn pick_medium<'a>(
	media: &'a [Media],
	discid: &str,
	discnumber: i32,
) -> Result<(Option<&'a Media>, Option<i32>), MusicBrainzError> {
	if media.is_empty() {
		return Ok((None, None));
	}

	if discnumber != 0 {
		if discnumber < 1 || discnumber > media.len() as i32 {
			return Err(MusicBrainzError::InvalidDiscNumber {
				requested: discnumber,
				available: media.len() as i32,
			});
		}
		return Ok((Some(&media[discnumber as usize - 1]), Some(discnumber)));
	}

	if let Some((idx, m)) = media
		.iter()
		.enumerate()
		.find(|(_, m)| medium_has_discid(m, discid))
	{
		return Ok((Some(m), Some(idx as i32 + 1)));
	}

	if media.len() == 1 {
		return Ok((Some(&media[0]), Some(1)));
	}

	Ok((Some(&media[0]), None))
}

fn medium_has_discid(medium: &Media, discid: &str) -> bool {
	medium
		.discs
		.as_ref()
		.map(|discs| discs.iter().any(|d| d.id == discid))
		.unwrap_or(false)
}

fn map_tracks(medium: &Media, nb_cd_tracks: usize) -> Vec<MusicBrainzTrackMeta> {
	let tracks = medium.tracks.as_ref().map(|v| v.as_slice()).unwrap_or(&[]);
	tracks
		.iter()
		.take(nb_cd_tracks)
		.map(map_track)
		.collect()
}

fn map_track(track: &Track) -> MusicBrainzTrackMeta {
	let rec = track.recording.as_ref();

	let mbid = rec.map(|r| r.id.clone());
	let title = if !track.title.trim().is_empty() {
		track.title.clone()
	} else {
		rec.map(|r| r.title.clone()).unwrap_or_default()
	};

	let artist = track
		.artist_credit
		.as_ref()
		.or_else(|| rec.and_then(|r| r.artist_credit.as_ref()))
		.map(|ac| join_artist_credit(ac))
		.filter(|s| !s.is_empty());

	MusicBrainzTrackMeta {
		mbid,
		title,
		artist,
	}
}

fn join_artist_credit(credits: &[ArtistCredit]) -> String {
	let mut out = String::new();
	for credit in credits {
		if !credit.name.is_empty() {
			out.push_str(&credit.name);
		} else {
			out.push_str(&credit.artist.name);
		}
		if let Some(join) = &credit.joinphrase {
			out.push_str(join);
		}
	}
	out
}

fn first_label_info(release: &Release) -> (Option<String>, Option<String>) {
	let Some(info) = release.label_info.as_ref().and_then(|li| li.first()) else {
		return (None, None);
	};

	let label = info.label.as_ref().map(|l| l.name.clone());
	let catalog = info.catalog_number.clone();
	(label, catalog)
}

fn release_summary(release: &Release) -> ReleaseSummary {
	ReleaseSummary {
		id: release.id.clone(),
		album: release.title.clone(),
		disambiguation: release.disambiguation.clone(),
		country: release.country.clone(),
		date: release.date.clone().map(|d| d.0),
		num_cds: release
			.media
			.as_ref()
			.map(|m| m.len() as i32)
			.unwrap_or_default(),
	}
}

fn release_status_name(status: &ReleaseStatus) -> String {
	match status {
		ReleaseStatus::Official => "Official",
		ReleaseStatus::Promotion => "Promotion",
		ReleaseStatus::Bootleg => "Bootleg",
		ReleaseStatus::PseudoRelease => "Pseudo-Release",
		ReleaseStatus::UnrecognizedReleaseStatus => "Unrecognized",
		_ => "Unknown",
	}
	.to_string()
}

fn release_packaging_name(packaging: &ReleasePackaging) -> String {
	match packaging {
		ReleasePackaging::Book => "Book",
		ReleasePackaging::Box => "Box",
		ReleasePackaging::CardboardPaperSleeve => "Cardboard/Paper Sleeve",
		ReleasePackaging::CassetteCase => "Cassette Case",
		ReleasePackaging::Digibook => "Digibook",
		ReleasePackaging::Digipak => "Digipak",
		ReleasePackaging::DiscboxSlider => "Discbox Slider",
		ReleasePackaging::Fatbox => "Fatbox",
		ReleasePackaging::GatefoldCover => "Gatefold Cover",
		ReleasePackaging::JewelCase => "Jewel Case",
		ReleasePackaging::KeepCase => "Keep Case",
		ReleasePackaging::PlasticSleeve => "Plastic Sleeve",
		ReleasePackaging::Slidepack => "Slidepack",
		ReleasePackaging::SlimJewelCase => "Slim Jewel Case",
		ReleasePackaging::SnapCase => "Snap Case",
		ReleasePackaging::Snappack => "SnapPack",
		ReleasePackaging::SuperJewelBox => "Super Jewel Box",
		ReleasePackaging::Other => "Other",
		ReleasePackaging::None => "None",
		ReleasePackaging::UnrecognizedReleasePackaging => "Unrecognized",
		_ => "Unknown",
	}
	.to_string()
}

#[cfg(test)]
mod tests {
	use std::fs;

	use wiremock::matchers::{header, method, path, query_param};
	use wiremock::{Mock, MockServer, ResponseTemplate};

	use super::*;

	fn fixture(name: &str) -> String {
		fs::read_to_string(format!("tests/fixtures/musicbrainz/{name}"))
			.expect("fixture should exist")
	}

	#[tokio::test]
	async fn lookup_maps_release_and_tracks_from_wiremock_fixture() {
		let server = MockServer::start().await;
		Mock::given(method("GET"))
			.and(path("/ws/2/discid/TESTDISC"))
			.and(query_param("inc", MB_INC))
			.and(query_param("fmt", "json"))
			.and(header("user-agent", "cyanrip-rs-test/0.1"))
			.respond_with(ResponseTemplate::new(200).set_body_string(fixture("discid_multi_release.json")))
			.mount(&server)
			.await;

		let http = ReqwestMusicBrainzHttpClient::default();
		let svc = MusicBrainzService::new(http, server.uri(), "cyanrip-rs-test/0.1");

		let out = svc
			.lookup_release(
				"TESTDISC",
				Some(&ReleaseSelection::Id("rel-2".to_string())),
				0,
				99,
			)
			.await
			.expect("lookup should succeed");

		assert_eq!(out.musicbrainz_albumid, "rel-2");
		assert_eq!(out.album, "Album Two");
		assert_eq!(out.album_artist.as_deref(), Some("The Artist feat. Guest"));
		assert_eq!(out.discnumber, Some(2));
		assert_eq!(out.totaldiscs, 2);
		assert_eq!(out.discname.as_deref(), Some("Bonus Disc"));
		assert_eq!(out.format.as_deref(), Some("CD"));
		assert_eq!(out.tracks.len(), 2);
		assert_eq!(out.tracks[0].mbid.as_deref(), Some("rec-21"));
		assert_eq!(out.tracks[0].title, "Song A");
		assert_eq!(out.tracks[0].artist.as_deref(), Some("Track Artist"));
		assert_eq!(out.tracks[1].title, "Fallback Title");
		assert_eq!(out.tracks[1].artist.as_deref(), Some("Track Fallback Artist"));
	}

	#[tokio::test]
	async fn lookup_requires_selection_for_multiple_releases() {
		let server = MockServer::start().await;
		Mock::given(method("GET"))
			.and(path("/ws/2/discid/TESTDISC"))
			.and(query_param("inc", MB_INC))
			.and(query_param("fmt", "json"))
			.respond_with(ResponseTemplate::new(200).set_body_string(fixture("discid_multi_release.json")))
			.mount(&server)
			.await;

		let http = ReqwestMusicBrainzHttpClient::default();
		let svc = MusicBrainzService::new(http, server.uri(), "cyanrip-rs-test/0.1");

		let err = svc
			.lookup_release("TESTDISC", None, 0, 99)
			.await
			.expect_err("should require release selection");

		match err {
			MusicBrainzError::MultipleReleases(list) => {
				assert_eq!(list.len(), 2);
				assert_eq!(list[0].id, "rel-1");
				assert_eq!(list[1].id, "rel-2");
			}
			other => panic!("unexpected error: {other:?}"),
		}
	}

	#[tokio::test]
	async fn lookup_live_multi_release_fixture_requires_selection() {
		let server = MockServer::start().await;
		Mock::given(method("GET"))
			.and(path("/ws/2/discid/BKkzOxbdODYWFIOEEZ3b.b_nm64-"))
			.and(query_param("inc", MB_INC))
			.and(query_param("fmt", "json"))
			.respond_with(
				ResponseTemplate::new(200)
					.set_body_string(fixture("discid_bkkz_multi_release_live.json")),
			)
			.mount(&server)
			.await;

		let http = ReqwestMusicBrainzHttpClient::default();
		let svc = MusicBrainzService::new(http, server.uri(), "cyanrip-rs-test/0.1");

		let err = svc
			.lookup_release("BKkzOxbdODYWFIOEEZ3b.b_nm64-", None, 0, 99)
			.await
			.expect_err("should require release selection for captured live fixture");

		match err {
			MusicBrainzError::MultipleReleases(list) => {
				assert!(
					list.len() >= 2,
					"expected at least two releases in live fixture"
				);
				assert_eq!(list[0].id, "4c63d77d-6348-4ae1-9616-f25e625fa0d7");
				assert_eq!(list[1].id, "1f504c20-5423-47fb-8d25-243ce749b92c");
			}
			other => panic!("unexpected error: {other:?}"),
		}
	}

	#[tokio::test]
	async fn lookup_live_multi_release_fixture_release_index_1_maps_expected_release() {
		let server = MockServer::start().await;
		Mock::given(method("GET"))
			.and(path("/ws/2/discid/BKkzOxbdODYWFIOEEZ3b.b_nm64-"))
			.and(query_param("inc", MB_INC))
			.and(query_param("fmt", "json"))
			.respond_with(
				ResponseTemplate::new(200)
					.set_body_string(fixture("discid_bkkz_multi_release_live.json")),
			)
			.mount(&server)
			.await;

		let http = ReqwestMusicBrainzHttpClient::default();
		let svc = MusicBrainzService::new(http, server.uri(), "cyanrip-rs-test/0.1");

		let out = svc
			.lookup_release(
				"BKkzOxbdODYWFIOEEZ3b.b_nm64-",
				Some(&ReleaseSelection::Index(1)),
				0,
				10,
			)
			.await
			.expect("lookup should succeed");

		assert_eq!(out.musicbrainz_albumid, "4c63d77d-6348-4ae1-9616-f25e625fa0d7");
		assert_eq!(
			out.album,
			"Power Classics! Classical Music for Your Active Lifestyle, Volume 3"
		);
		assert_eq!(out.totaldiscs, 1);
		assert_eq!(out.discnumber, Some(1));
		assert_eq!(out.barcode.as_deref(), Some("018111414920"));
	}

	#[tokio::test]
	async fn lookup_live_multi_release_fixture_release_index_2_maps_expected_release() {
		let server = MockServer::start().await;
		Mock::given(method("GET"))
			.and(path("/ws/2/discid/BKkzOxbdODYWFIOEEZ3b.b_nm64-"))
			.and(query_param("inc", MB_INC))
			.and(query_param("fmt", "json"))
			.respond_with(
				ResponseTemplate::new(200)
					.set_body_string(fixture("discid_bkkz_multi_release_live.json")),
			)
			.mount(&server)
			.await;

		let http = ReqwestMusicBrainzHttpClient::default();
		let svc = MusicBrainzService::new(http, server.uri(), "cyanrip-rs-test/0.1");

		let out = svc
			.lookup_release(
				"BKkzOxbdODYWFIOEEZ3b.b_nm64-",
				Some(&ReleaseSelection::Index(2)),
				0,
				10,
			)
			.await
			.expect("lookup should succeed");

		assert_eq!(out.musicbrainz_albumid, "1f504c20-5423-47fb-8d25-243ce749b92c");
		assert_eq!(
			out.album,
			"Power Classics! Classical Music for your Active Lifestyle"
		);
		assert_eq!(out.totaldiscs, 10);
		assert_eq!(out.discnumber, Some(3));
		assert_eq!(out.barcode.as_deref(), Some("018111584821"));
	}

	#[tokio::test]
	async fn lookup_handles_invalid_discnumber() {
		let server = MockServer::start().await;
		Mock::given(method("GET"))
			.and(path("/ws/2/discid/TESTDISC"))
			.and(query_param("inc", MB_INC))
			.and(query_param("fmt", "json"))
			.respond_with(ResponseTemplate::new(200).set_body_string(fixture("discid_single_release.json")))
			.mount(&server)
			.await;

		let http = ReqwestMusicBrainzHttpClient::default();
		let svc = MusicBrainzService::new(http, server.uri(), "cyanrip-rs-test/0.1");

		let err = svc
			.lookup_release("TESTDISC", Some(&ReleaseSelection::Index(1)), 3, 99)
			.await
			.expect_err("discnumber should be invalid");

		assert_eq!(
			err,
			MusicBrainzError::InvalidDiscNumber {
				requested: 3,
				available: 1
			}
		);
	}

	#[tokio::test]
	async fn lookup_maps_not_found_from_http_404() {
		let server = MockServer::start().await;
		Mock::given(method("GET"))
			.and(path("/ws/2/discid/MISSING"))
			.respond_with(ResponseTemplate::new(404))
			.mount(&server)
			.await;

		let http = ReqwestMusicBrainzHttpClient::default();
		let svc = MusicBrainzService::new(http, server.uri(), "cyanrip-rs-test/0.1");

		let err = svc
			.lookup_release("MISSING", Some(&ReleaseSelection::Index(1)), 0, 99)
			.await
			.expect_err("missing discid should map to not found");
		assert_eq!(err, MusicBrainzError::NotFound);
	}

	#[tokio::test]
	async fn lookup_maps_not_found_from_empty_releases_fixture() {
		let server = MockServer::start().await;
		Mock::given(method("GET"))
			.and(path("/ws/2/discid/EMPTY"))
			.respond_with(ResponseTemplate::new(200).set_body_string(fixture("discid_no_releases.json")))
			.mount(&server)
			.await;

		let http = ReqwestMusicBrainzHttpClient::default();
		let svc = MusicBrainzService::new(http, server.uri(), "cyanrip-rs-test/0.1");

		let err = svc
			.lookup_release("EMPTY", Some(&ReleaseSelection::Index(1)), 0, 99)
			.await
			.expect_err("empty releases should map to not found");
		assert_eq!(err, MusicBrainzError::NotFound);
	}
}
