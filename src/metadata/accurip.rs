use async_trait::async_trait;

const ACCURIP_DB_BASE_URL: &str = "http://www.accuraterip.com/accuraterip";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccuDbStatus {
    Disabled,
    NotFound,
    Found,
    Mismatch,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccuRipTrackInput {
    pub number: u32,
    pub start_lsn: u32,
    pub end_lsn: u32,
    pub track_is_data: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccuRipDiscIds {
    pub audio_tracks: usize,
    pub id_type_1: u32,
    pub id_type_2: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccuRipDbEntry {
    pub confidence: u8,
    pub checksum: u32,
    pub checksum_450: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccuRipTrackMatches {
    pub entries: Vec<AccuRipDbEntry>,
    pub max_confidence: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccuRipLookupResult {
    pub status: AccuDbStatus,
    pub request_url: String,
    pub disc_ids: AccuRipDiscIds,
    pub track_matches: Vec<AccuRipTrackMatches>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccuRipError {
    NoAudioTracks,
    ParseError(String),
    Http(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccuRipHttpResponse {
    pub status_code: u16,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
}

#[async_trait]
pub trait AccuRipHttpClient: Send + Sync {
    async fn get(&self, url: &str, user_agent: &str) -> Result<AccuRipHttpResponse, AccuRipError>;
}

#[derive(Debug, Clone)]
pub struct ReqwestAccuRipHttpClient {
    client: reqwest::Client,
}

impl Default for ReqwestAccuRipHttpClient {
    fn default() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl AccuRipHttpClient for ReqwestAccuRipHttpClient {
    async fn get(&self, url: &str, user_agent: &str) -> Result<AccuRipHttpResponse, AccuRipError> {
        let resp = self
            .client
            .get(url)
            .header(reqwest::header::USER_AGENT, user_agent)
            .send()
            .await
            .map_err(|e| AccuRipError::Http(e.to_string()))?;

        let status_code = resp.status().as_u16();
        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(ToString::to_string);
        let body = resp
            .bytes()
            .await
            .map_err(|e| AccuRipError::Http(e.to_string()))?
            .to_vec();

        Ok(AccuRipHttpResponse {
            status_code,
            content_type,
            body,
        })
    }
}

#[derive(Debug, Clone)]
pub struct AccuRipService<H: AccuRipHttpClient> {
    http: H,
    base_url: String,
    user_agent: String,
}

impl<H: AccuRipHttpClient> AccuRipService<H> {
    pub fn new(http: H, base_url: impl Into<String>, user_agent: impl Into<String>) -> Self {
        Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            user_agent: user_agent.into(),
        }
    }

    pub async fn lookup(
        &self,
        tracks: &[AccuRipTrackInput],
        cddb_id: u32,
    ) -> Result<AccuRipLookupResult, AccuRipError> {
        let disc_ids = compute_accurip_ids(tracks)?;
        let request_url = build_accurip_request_url(&self.base_url, &disc_ids, cddb_id);
        let resp = self.http.get(&request_url, &self.user_agent).await?;

        if resp.status_code == 404 {
            return Ok(AccuRipLookupResult {
                status: AccuDbStatus::NotFound,
                request_url,
                disc_ids,
                track_matches: vec![
                    AccuRipTrackMatches {
                        entries: Vec::new(),
                        max_confidence: 0,
                    };
                    disc_ids.audio_tracks
                ],
            });
        }

        if !(200..300).contains(&resp.status_code) {
            return Ok(AccuRipLookupResult {
                status: AccuDbStatus::Error,
                request_url,
                disc_ids,
                track_matches: vec![
                    AccuRipTrackMatches {
                        entries: Vec::new(),
                        max_confidence: 0,
                    };
                    disc_ids.audio_tracks
                ],
            });
        }

        if is_probable_html_miss(resp.content_type.as_deref(), &resp.body) {
            return Ok(AccuRipLookupResult {
                status: AccuDbStatus::NotFound,
                request_url,
                disc_ids,
                track_matches: vec![
                    AccuRipTrackMatches {
                        entries: Vec::new(),
                        max_confidence: 0,
                    };
                    disc_ids.audio_tracks
                ],
            });
        }

        let parsed = parse_accurip_db_blob(&resp.body, disc_ids, cddb_id)?;
        Ok(AccuRipLookupResult {
            request_url,
            ..parsed
        })
    }
}

impl Default for AccuRipService<ReqwestAccuRipHttpClient> {
    fn default() -> Self {
        Self::new(
            ReqwestAccuRipHttpClient::default(),
            ACCURIP_DB_BASE_URL,
            "cyanrip-rs/0.1",
        )
    }
}

pub fn compute_accurip_ids(tracks: &[AccuRipTrackInput]) -> Result<AccuRipDiscIds, AccuRipError> {
    let audio: Vec<&AccuRipTrackInput> = tracks.iter().filter(|t| !t.track_is_data).collect();
    if audio.is_empty() {
        return Err(AccuRipError::NoAudioTracks);
    }

    let mut idt1 = 0u64;
    let mut idt2 = 0u64;

    for t in &audio {
        idt1 = idt1.wrapping_add(t.start_lsn as u64);
        let start_or_one = if t.start_lsn == 0 { 1 } else { t.start_lsn };
        idt2 = idt2.wrapping_add((start_or_one as u64).wrapping_mul(t.number as u64));
    }

    let last = audio[audio.len() - 1].end_lsn.wrapping_add(1);
    idt1 = idt1.wrapping_add(last as u64);
    idt2 = idt2.wrapping_add((last as u64).wrapping_mul((audio.len() as u64) + 1));

    Ok(AccuRipDiscIds {
        audio_tracks: audio.len(),
        id_type_1: (idt1 & 0xFFFF_FFFF) as u32,
        id_type_2: (idt2 & 0xFFFF_FFFF) as u32,
    })
}

pub fn build_accurip_request_url(base_url: &str, ids: &AccuRipDiscIds, cddb_id: u32) -> String {
    let id_type_1_s = format!("{:08x}", ids.id_type_1);
    let b = id_type_1_s.as_bytes();
    format!(
        "{}/{}/{}/{}/dBAR-{:03}-{}-{:08x}-{:08x}.bin",
        base_url,
        b[7] as char,
        b[6] as char,
        b[5] as char,
        ids.audio_tracks,
        id_type_1_s,
        ids.id_type_2,
        cddb_id,
    )
}

pub fn parse_accurip_db_blob(
    data: &[u8],
    disc_ids: AccuRipDiscIds,
    cddb_id: u32,
) -> Result<AccuRipLookupResult, AccuRipError> {
    let entry_size = 1usize + 12usize + disc_ids.audio_tracks * (1usize + 8usize);
    if entry_size == 0 || !data.len().is_multiple_of(entry_size) {
        return Err(AccuRipError::ParseError(
            "unexpected number of bytes".to_string(),
        ));
    }

    let mut track_matches = vec![
        AccuRipTrackMatches {
            entries: Vec::new(),
            max_confidence: 0,
        };
        disc_ids.audio_tracks
    ];

    let mut off = 0usize;
    let mut status = AccuDbStatus::Found;

    while off < data.len() {
        let start = off;
        let ntracks = data[off] as usize;
        off += 1;
        let id1 = read_le_u32(data, &mut off)?;
        let id2 = read_le_u32(data, &mut off)?;
        let cddb = read_le_u32(data, &mut off)?;

        if ntracks != disc_ids.audio_tracks
            || id1 != disc_ids.id_type_1
            || id2 != disc_ids.id_type_2
            || cddb != cddb_id
        {
            // Preserve C behavior: mismatching entries are skipped and status remains found.
            off = start + entry_size;
            continue;
        }

        for tm in track_matches.iter_mut().take(disc_ids.audio_tracks) {
            let confidence = read_u8(data, &mut off)?;
            let checksum = read_le_u32(data, &mut off)?;
            let checksum_450 = read_le_u32(data, &mut off)?;

            tm.entries.push(AccuRipDbEntry {
                confidence,
                checksum,
                checksum_450,
            });
            tm.max_confidence = tm.max_confidence.max(confidence as i32);
        }
    }

    for tm in &mut track_matches {
        tm.entries.sort_by_key(|e| e.confidence);
    }

    if track_matches.iter().all(|t| t.entries.is_empty()) {
        status = AccuDbStatus::Mismatch;
    }

    Ok(AccuRipLookupResult {
        status,
        request_url: String::new(),
        disc_ids,
        track_matches,
    })
}

pub fn find_accurip_confidence(
    status: AccuDbStatus,
    entries: &[AccuRipDbEntry],
    checksum: u32,
    is_450: bool,
) -> i32 {
    if status != AccuDbStatus::Found {
        return 0;
    }

    let mut best = -1i32;
    for e in entries {
        if is_450 {
            if e.checksum_450 == checksum {
                best = best.max(e.confidence as i32);
            }
        } else if e.checksum == checksum {
            best = best.max(e.confidence as i32);
        }
    }

    best
}

fn is_probable_html_miss(content_type: Option<&str>, body: &[u8]) -> bool {
    let ct_is_octet = content_type
        .map(|ct| ct.starts_with("application/octet-stream"))
        .unwrap_or(false);
    if ct_is_octet {
        return false;
    }

    let limit = body.len().min(64);
    let head = &body[..limit];
    head.windows(4).any(|w| w == b"html")
}

fn read_u8(data: &[u8], off: &mut usize) -> Result<u8, AccuRipError> {
    if *off >= data.len() {
        return Err(AccuRipError::ParseError(
            "unexpected end of data".to_string(),
        ));
    }
    let v = data[*off];
    *off += 1;
    Ok(v)
}

fn read_le_u32(data: &[u8], off: &mut usize) -> Result<u32, AccuRipError> {
    if *off + 4 > data.len() {
        return Err(AccuRipError::ParseError(
            "unexpected end of data".to_string(),
        ));
    }
    let v = u32::from_le_bytes([data[*off], data[*off + 1], data[*off + 2], data[*off + 3]]);
    *off += 4;
    Ok(v)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    fn fixture(name: &str) -> Vec<u8> {
        fs::read(format!("tests/fixtures/accurip/{name}")).expect("fixture should exist")
    }

    fn test_tracks() -> Vec<AccuRipTrackInput> {
        vec![
            AccuRipTrackInput {
                number: 1,
                start_lsn: 0,
                end_lsn: 10_000,
                track_is_data: false,
            },
            AccuRipTrackInput {
                number: 2,
                start_lsn: 15_000,
                end_lsn: 20_000,
                track_is_data: false,
            },
        ]
    }

    fn expected_path(ids: AccuRipDiscIds, cddb_id: u32) -> String {
        let id1 = format!("{:08x}", ids.id_type_1);
        format!(
            "/db/{}/{}/{}/dBAR-{:03}-{:08x}-{:08x}-{:08x}.bin",
            id1.chars().nth(7).expect("id1 must be 8 chars"),
            id1.chars().nth(6).expect("id1 must be 8 chars"),
            id1.chars().nth(5).expect("id1 must be 8 chars"),
            ids.audio_tracks,
            ids.id_type_1,
            ids.id_type_2,
            cddb_id,
        )
    }

    fn build_valid_blob_for(ids: AccuRipDiscIds, cddb_id: u32) -> Vec<u8> {
        let mut out = Vec::new();
        for (c1, ck1, ck1450, c2, ck2, ck2450) in [
            (
                5u8,
                0x11111111u32,
                0xAAAAAAAAu32,
                3u8,
                0x22222222u32,
                0xBBBBBBBBu32,
            ),
            (
                9u8,
                0x33333333u32,
                0xCCCCCCCCu32,
                8u8,
                0x22222222u32,
                0xDDDDDDDDu32,
            ),
        ] {
            out.push(ids.audio_tracks as u8);
            out.extend_from_slice(&ids.id_type_1.to_le_bytes());
            out.extend_from_slice(&ids.id_type_2.to_le_bytes());
            out.extend_from_slice(&cddb_id.to_le_bytes());

            out.push(c1);
            out.extend_from_slice(&ck1.to_le_bytes());
            out.extend_from_slice(&ck1450.to_le_bytes());

            out.push(c2);
            out.extend_from_slice(&ck2.to_le_bytes());
            out.extend_from_slice(&ck2450.to_le_bytes());
        }

        out
    }

    #[test]
    fn computes_disc_ids_like_c_formula() {
        let ids = compute_accurip_ids(&test_tracks()).expect("ids should compute");
        assert_eq!(ids.audio_tracks, 2);
        assert_eq!(ids.id_type_1, 0x000088B9);
        assert_eq!(ids.id_type_2, 0x00015F94);
    }

    #[test]
    fn builds_request_url_like_c_path_template() {
        let ids = AccuRipDiscIds {
            audio_tracks: 2,
            id_type_1: 0x11223344,
            id_type_2: 0x55667788,
        };
        let url = build_accurip_request_url("http://host/accuraterip", &ids, 0xAABBCCDD);
        assert_eq!(
            url,
            "http://host/accuraterip/4/4/3/dBAR-002-11223344-55667788-aabbccdd.bin"
        );
    }

    #[test]
    fn parses_fixture_blob_and_sorts_track_entries() {
        let ids = AccuRipDiscIds {
            audio_tracks: 2,
            id_type_1: 0x11223344,
            id_type_2: 0x55667788,
        };
        let out = parse_accurip_db_blob(&fixture("db_valid.bin"), ids, 0xAABBCCDD)
            .expect("valid fixture should parse");

        assert_eq!(out.status, AccuDbStatus::Found);
        assert_eq!(out.track_matches.len(), 2);
        assert_eq!(out.track_matches[0].entries.len(), 2);
        assert_eq!(out.track_matches[0].entries[0].confidence, 5);
        assert_eq!(out.track_matches[0].entries[1].confidence, 9);
        assert_eq!(out.track_matches[1].entries[0].confidence, 3);
        assert_eq!(out.track_matches[1].entries[1].confidence, 8);
        assert_eq!(out.track_matches[1].max_confidence, 8);
    }

    #[test]
    fn parse_rejects_truncated_fixture_blob() {
        let ids = AccuRipDiscIds {
            audio_tracks: 2,
            id_type_1: 0x11223344,
            id_type_2: 0x55667788,
        };
        let err = parse_accurip_db_blob(&fixture("db_truncated.bin"), ids, 0xAABBCCDD)
            .expect_err("truncated fixture must fail");
        assert_eq!(
            err,
            AccuRipError::ParseError("unexpected number of bytes".to_string())
        );
    }

    #[test]
    fn find_confidence_matches_c_semantics() {
        let entries = vec![
            AccuRipDbEntry {
                confidence: 2,
                checksum: 0x1111,
                checksum_450: 0xAAAA,
            },
            AccuRipDbEntry {
                confidence: 7,
                checksum: 0x1111,
                checksum_450: 0xBBBB,
            },
        ];
        assert_eq!(
            find_accurip_confidence(AccuDbStatus::Found, &entries, 0x1111, false),
            7
        );
        assert_eq!(
            find_accurip_confidence(AccuDbStatus::Found, &entries, 0xAAAA, true),
            2
        );
        assert_eq!(
            find_accurip_confidence(AccuDbStatus::Found, &entries, 0x9999, false),
            -1
        );
        assert_eq!(
            find_accurip_confidence(AccuDbStatus::NotFound, &entries, 0x1111, false),
            0
        );
    }

    #[tokio::test]
    async fn lookup_maps_404_to_not_found() {
        let server = MockServer::start().await;
        let tracks = test_tracks();
        let ids = compute_accurip_ids(&tracks).expect("ids should compute");
        let exp_path = expected_path(ids, 0xAABBCCDD);

        Mock::given(method("GET"))
            .and(path(exp_path.as_str()))
            .and(header("user-agent", "cyanrip-rs-test/0.1"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let svc = AccuRipService::new(
            ReqwestAccuRipHttpClient::default(),
            format!("{}/db", server.uri()),
            "cyanrip-rs-test/0.1",
        );

        let out = svc
            .lookup(&tracks, 0xAABBCCDD)
            .await
            .expect("lookup should work");
        assert_eq!(out.status, AccuDbStatus::NotFound);
    }

    #[tokio::test]
    async fn lookup_parses_valid_fixture_from_wiremock() {
        let server = MockServer::start().await;
        let tracks = vec![
            AccuRipTrackInput {
                number: 1,
                start_lsn: 0,
                end_lsn: 100,
                track_is_data: false,
            },
            AccuRipTrackInput {
                number: 2,
                start_lsn: 150,
                end_lsn: 200,
                track_is_data: false,
            },
        ];
        let ids = compute_accurip_ids(&tracks).expect("ids should compute");
        let expected_path = expected_path(ids, 0xAABBCCDD);
        let body = build_valid_blob_for(ids, 0xAABBCCDD);

        Mock::given(method("GET"))
            .and(path(expected_path.as_str()))
            .and(header("user-agent", "cyanrip-rs-test/0.1"))
            .respond_with(
                ResponseTemplate::new(200)
                    .append_header("content-type", "application/octet-stream")
                    .set_body_bytes(body),
            )
            .mount(&server)
            .await;

        let svc = AccuRipService::new(
            ReqwestAccuRipHttpClient::default(),
            format!("{}/db", server.uri()),
            "cyanrip-rs-test/0.1",
        );

        let out = svc
            .lookup(&tracks, 0xAABBCCDD)
            .await
            .expect("lookup should work");
        assert_eq!(out.status, AccuDbStatus::Found);
        assert_eq!(out.track_matches.len(), 2);
    }

    #[tokio::test]
    async fn lookup_uses_html_heuristic_for_not_found() {
        let server = MockServer::start().await;
        let tracks = vec![
            AccuRipTrackInput {
                number: 1,
                start_lsn: 0,
                end_lsn: 100,
                track_is_data: false,
            },
            AccuRipTrackInput {
                number: 2,
                start_lsn: 150,
                end_lsn: 200,
                track_is_data: false,
            },
        ];
        let ids = compute_accurip_ids(&tracks).expect("ids should compute");
        let expected_path = expected_path(ids, 0xAABBCCDD);

        Mock::given(method("GET"))
            .and(path(expected_path.as_str()))
            .respond_with(
                ResponseTemplate::new(200)
                    .append_header("content-type", "text/html")
                    .set_body_bytes(fixture("db_html_error.bin")),
            )
            .mount(&server)
            .await;

        let svc = AccuRipService::new(
            ReqwestAccuRipHttpClient::default(),
            format!("{}/db", server.uri()),
            "cyanrip-rs-test/0.1",
        );

        let out = svc
            .lookup(&tracks, 0xAABBCCDD)
            .await
            .expect("lookup should work");
        assert_eq!(out.status, AccuDbStatus::NotFound);
    }
}
