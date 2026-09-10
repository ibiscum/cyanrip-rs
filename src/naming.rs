use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::SanitizeMethod;

#[derive(Debug, Clone)]
pub struct NamingContext {
    pub sanitize_method: SanitizeMethod,
    pub nb_tracks: usize,
}

#[derive(Debug, Clone)]
struct CharReplacement {
    from: char,
    to: char,
    to_u: &'static str,
    is_avail_locally: bool,
}

const CHAR_REPLACEMENTS: &[CharReplacement] = &[
    CharReplacement {
        from: '<',
        to: '_',
        to_u: "‹",
        is_avail_locally: true,
    },
    CharReplacement {
        from: '>',
        to: '_',
        to_u: "›",
        is_avail_locally: true,
    },
    CharReplacement {
        from: ':',
        to: '_',
        to_u: "∶",
        is_avail_locally: true,
    },
    CharReplacement {
        from: '|',
        to: '_',
        to_u: "│",
        is_avail_locally: true,
    },
    CharReplacement {
        from: '?',
        to: '_',
        to_u: "？",
        is_avail_locally: true,
    },
    CharReplacement {
        from: '*',
        to: '_',
        to_u: "∗",
        is_avail_locally: true,
    },
    CharReplacement {
        from: '/',
        to: '_',
        to_u: "∕",
        is_avail_locally: false,
    },
    CharReplacement {
        from: '\\',
        to: '_',
        to_u: "⧹",
        is_avail_locally: true,
    },
    CharReplacement {
        from: '"',
        to: '\'',
        to_u: "“",
        is_avail_locally: true,
    },
    CharReplacement {
        from: '"',
        to: '\'',
        to_u: "”",
        is_avail_locally: true,
    },
];

pub fn append_missing_keys(src: &str, key1: &str, key2: &str) -> String {
    let mut chars: Vec<char> = src.chars().collect();
    let mut add_key1_offset = None;
    let mut add_key2_offset = None;

    let mut count = 0;
    let mut has_key = false;
    let mut esc = false;
    let mut entry_start = 0usize;

    for (i, c) in src.char_indices() {
        if count >= 2 {
            break;
        }

        if esc {
            esc = false;
            continue;
        }

        if c == '\\' {
            esc = true;
            continue;
        }
        if c == '=' {
            has_key = true;
            continue;
        }

        if c == ':' {
            if !has_key && i > entry_start {
                if count == 0 {
                    add_key1_offset = Some(entry_start);
                } else {
                    add_key2_offset = Some(entry_start);
                }
            }
            count += 1;
            entry_start = i + c.len_utf8();
            has_key = false;
        }
    }

    if count < 2 && !has_key && src.len() > entry_start {
        if count == 0 {
            add_key1_offset = Some(entry_start);
        } else {
            add_key2_offset = Some(entry_start);
        }
    }

    let mut out = src.to_string();

    if let Some(off) = add_key1_offset {
        out.insert_str(off, key1);
        if let Some(k2) = add_key2_offset
            && k2 >= off
        {
            add_key2_offset = Some(k2 + key1.len());
        }
    }

    if let Some(off) = add_key2_offset {
        out.insert_str(off, key2);
    }

    let _ = &mut chars;
    out
}

pub fn is_integer(src: &str) -> bool {
    !src.is_empty() && src.bytes().all(|b| b.is_ascii_digit())
}

pub fn sanitize_text(ctx: &NamingContext, input: &str, sanitize_fwdslash: bool) -> String {
    let os_sanitize = matches!(
        ctx.sanitize_method,
        SanitizeMethod::OsSimple | SanitizeMethod::OsUnicode
    );

    let mut out = String::with_capacity(input.len());
    let mut quote_match = false;

    for ch in input.chars() {
        let rep = if ch == '"' {
            let r = if quote_match {
                &CHAR_REPLACEMENTS[9]
            } else {
                &CHAR_REPLACEMENTS[8]
            };
            quote_match = !quote_match;
            Some(r)
        } else {
            CHAR_REPLACEMENTS.iter().take(8).find(|r| r.from == ch)
        };

        let Some(rep) = rep else {
            out.push(ch);
            continue;
        };

        let skip_sanitation = os_sanitize && rep.is_avail_locally;
        let passthrough_slash = rep.from == std::path::MAIN_SEPARATOR && !sanitize_fwdslash;

        if skip_sanitation || passthrough_slash {
            out.push(ch);
            continue;
        }

        match ctx.sanitize_method {
            SanitizeMethod::Simple | SanitizeMethod::OsSimple => out.push(rep.to),
            SanitizeMethod::Unicode | SanitizeMethod::OsUnicode => out.push_str(rep.to_u),
        }
    }

    out
}

fn get_tag_value(
    ctx: &NamingContext,
    meta: &HashMap<String, String>,
    output_format: &str,
    key: &str,
) -> Option<String> {
    match key {
        "year" => meta
            .get("date")
            .map(|d| d.split([':', '-']).next().unwrap_or_default().to_string()),
        "format" => Some(output_format.to_string()),
        "track" => {
            let track = meta.get("track")?;
            if is_integer(track) {
                let digits = track.len();
                let mut pad = 0;
                if (digits + pad) < 2 && ctx.nb_tracks > 9 {
                    pad += 1;
                }
                if (digits + pad) < 3 && ctx.nb_tracks > 99 {
                    pad += 1;
                }
                Some(format!("{}{}", "0".repeat(pad), track))
            } else {
                Some(track.to_string())
            }
        }
        _ => meta.get(key).cloned(),
    }
}

fn eval_cond(
    val1: &str,
    op: &str,
    val2: &str,
    val1_from_tag: bool,
    val2_from_tag: bool,
) -> Result<bool, String> {
    match op {
        "==" => Ok(val1 == val2),
        "!=" => Ok(val1 != val2),
        ">" | "<" => {
            let v1i = is_integer(val1);
            let v2i = is_integer(val2);

            if v1i && v2i {
                let a: i64 = val1.parse().unwrap_or(0);
                let b: i64 = val2.parse().unwrap_or(0);
                Ok(if op == ">" { a > b } else { a < b })
            } else if !v1i && !v2i {
                Ok(if op == ">" { val1 > val2 } else { val1 < val2 })
            } else {
                let a = if v1i {
                    val1.parse::<i64>().unwrap_or(0)
                } else if val1_from_tag {
                    1
                } else {
                    0
                };
                let b = if v2i {
                    val2.parse::<i64>().unwrap_or(0)
                } else if val2_from_tag {
                    1
                } else {
                    0
                };
                Ok(if op == ">" { a > b } else { a < b })
            }
        }
        _ => Err("Invalid condition syntax!".to_string()),
    }
}

pub fn render_scheme(
    ctx: &NamingContext,
    meta: &HashMap<String, String>,
    output_format: &str,
    scheme: &str,
) -> Result<String, String> {
    let mut out = String::new();
    let mut pos = 0usize;

    while pos < scheme.len() {
        let remain = &scheme[pos..];
        let Some(open_rel) = remain.find('{') else {
            out.push_str(&sanitize_text(ctx, remain, false));
            break;
        };

        if open_rel > 0 {
            out.push_str(&sanitize_text(ctx, &remain[..open_rel], false));
        }

        let token_start = pos + open_rel + 1;
        let token_end = scheme[token_start..]
            .find('}')
            .map(|i| token_start + i)
            .ok_or_else(|| "Invalid scheme syntax, unterminated \"{\"!".to_string())?;
        let token = &scheme[token_start..token_end];
        pos = token_end + 1;

        if token.starts_with("if") && token.contains('#') {
            let mut segs = token.split('#');
            let _if_head = segs.next();
            let val1_key = segs
                .next()
                .ok_or_else(|| "Invalid scheme syntax, no \"#\"!".to_string())?
                .trim();
            let op = segs
                .next()
                .ok_or_else(|| "Invalid scheme syntax, no terminating \"#\"!".to_string())?
                .trim();
            let val2_key = segs
                .next()
                .ok_or_else(|| "Invalid scheme syntax, no terminating \"#\"!".to_string())?
                .trim();
            let true_expr = segs
                .next()
                .ok_or_else(|| "Invalid scheme syntax, no terminating \"#\"!".to_string())?;

            let (val1, val1_from_tag) =
                if let Some(v) = get_tag_value(ctx, meta, output_format, val1_key) {
                    (v, true)
                } else {
                    (val1_key.to_string(), false)
                };
            let (val2, val2_from_tag) =
                if let Some(v) = get_tag_value(ctx, meta, output_format, val2_key) {
                    (v, true)
                } else {
                    (val2_key.to_string(), false)
                };

            if eval_cond(&val1, op, &val2, val1_from_tag, val2_from_tag)? {
                for part in true_expr.split('|') {
                    if part.is_empty() {
                        continue;
                    }
                    let (rendered, from_tag) =
                        if let Some(v) = get_tag_value(ctx, meta, output_format, part) {
                            (v, true)
                        } else {
                            (part.to_string(), false)
                        };
                    out.push_str(&sanitize_text(ctx, &rendered, from_tag));
                }
            }
            continue;
        }

        let (rendered, from_tag) = if let Some(v) = get_tag_value(ctx, meta, output_format, token) {
            (v, true)
        } else {
            (token.to_string(), false)
        };
        out.push_str(&sanitize_text(ctx, &rendered, from_tag));
    }

    Ok(out)
}

pub fn trim_path_components(path: &str, separator: char) -> String {
    let mut components = Vec::new();
    for comp in path.split(separator) {
        components.push(comp.trim_matches([' ', '\t']).to_string());
    }

    let mut out = components.join(&separator.to_string());
    if let Some(dot_pos) = out.rfind('.') {
        let (left, right) = out.split_at(dot_pos);
        let left = left.trim_end_matches([' ', '\t']);
        out = format!("{left}{right}");
    }
    out
}

pub fn build_track_relative_path(
    ctx: &NamingContext,
    album_meta: &HashMap<String, String>,
    track_meta: &HashMap<String, String>,
    folder_scheme: &str,
    track_scheme: &str,
    format_suffix: &str,
    extension: &str,
) -> Result<String, String> {
    let folder = render_scheme(ctx, album_meta, format_suffix, folder_scheme)?;
    let file = render_scheme(ctx, track_meta, format_suffix, track_scheme)?;
    let path = format!("{folder}/{file}.{extension}");
    Ok(trim_path_components(&path, '/'))
}

pub fn build_log_relative_path(
    ctx: &NamingContext,
    album_meta: &HashMap<String, String>,
    folder_scheme: &str,
    log_scheme: &str,
    format_suffix: &str,
) -> Result<String, String> {
    let folder = render_scheme(ctx, album_meta, format_suffix, folder_scheme)?;
    let file = render_scheme(ctx, album_meta, format_suffix, log_scheme)?;
    let path = format!("{folder}/{file}.log");
    Ok(trim_path_components(&path, '/'))
}

pub fn build_cue_relative_path(
    ctx: &NamingContext,
    album_meta: &HashMap<String, String>,
    folder_scheme: &str,
    cue_scheme: &str,
    format_suffix: &str,
) -> Result<String, String> {
    let folder = render_scheme(ctx, album_meta, format_suffix, folder_scheme)?;
    let file = render_scheme(ctx, album_meta, format_suffix, cue_scheme)?;
    let path = format!("{folder}/{file}.cue");
    Ok(trim_path_components(&path, '/'))
}

pub fn build_cover_relative_path(
    ctx: &NamingContext,
    album_meta: &HashMap<String, String>,
    folder_scheme: &str,
    format_suffix: &str,
    title: &str,
    extension: Option<&str>,
) -> Result<String, String> {
    let folder = render_scheme(ctx, album_meta, format_suffix, folder_scheme)?;
    let sanitized_title = sanitize_text(ctx, title, true);
    let ext = extension
        .map(|e| e.trim_start_matches('.'))
        .filter(|e| !e.trim().is_empty())
        .unwrap_or("bin");
    let path = format!("{folder}/{sanitized_title}.{ext}");
    Ok(trim_path_components(&path, '/'))
}

pub fn resolve_output_path(
    output_root: &Path,
    relative_path: &str,
    create_dirs: bool,
) -> Result<PathBuf, std::io::Error> {
    let absolute = output_root.join(relative_path);
    if create_dirs && let Some(parent) = absolute.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(absolute)
}

pub fn detect_track_path_collisions(entries: &[(u32, String)]) -> Vec<(u32, u32, String)> {
    let mut seen: HashMap<&str, u32> = HashMap::new();
    let mut out = Vec::new();

    for (track, path) in entries {
        if let Some(first_track) = seen.get(path.as_str()) {
            out.push((*first_track, *track, path.clone()));
        } else {
            seen.insert(path.as_str(), *track);
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SanitizeMethod;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn mkctx(method: SanitizeMethod, tracks: usize) -> NamingContext {
        NamingContext {
            sanitize_method: method,
            nb_tracks: tracks,
        }
    }

    fn map(entries: &[(&str, &str)]) -> HashMap<String, String> {
        entries
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn prepends_missing_keys() {
        assert_eq!(
            append_missing_keys("Foo:Bar", "album=", "album_artist="),
            "album=Foo:album_artist=Bar"
        );
    }

    #[test]
    fn keeps_existing_keys() {
        assert_eq!(
            append_missing_keys("album=Foo:artist=Bar", "album=", "album_artist="),
            "album=Foo:artist=Bar"
        );
    }

    #[test]
    fn prepends_keys_with_escapes_like_c() {
        assert_eq!(
            append_missing_keys("Foo\\:Bar:Baz", "album=", "album_artist="),
            "album=Foo\\:Bar:album_artist=Baz"
        );
        assert_eq!(
            append_missing_keys("title=Foo\\=Bar:Baz", "title=", "artist="),
            "title=Foo\\=Bar:artist=Baz"
        );
    }

    #[test]
    fn integer_check() {
        assert!(is_integer("123"));
        assert!(!is_integer("12a"));
        assert!(!is_integer(""));
    }

    #[test]
    fn sanitize_simple_and_unicode() {
        let s = "A<B>:C?D*E/F\\G\"H\"";
        let simple = sanitize_text(&mkctx(SanitizeMethod::Simple, 10), s, true);
        assert_eq!(simple, "A_B__C_D_E_F_G'H'");

        let unicode = sanitize_text(&mkctx(SanitizeMethod::Unicode, 10), s, true);
        assert!(unicode.contains('‹'));
        assert!(unicode.contains('›'));
        assert!(unicode.contains("“H”"));
    }

    #[test]
    fn sanitize_keeps_dir_separator_for_literal_text() {
        let out = sanitize_text(&mkctx(SanitizeMethod::Simple, 10), "Disc/Track", false);
        assert_eq!(out, "Disc/Track");
    }

    #[test]
    fn render_default_folder_condition() {
        let ctx = mkctx(SanitizeMethod::Unicode, 12);
        let meta = map(&[("album", "The Wall")]);
        let s = "{album}{if #releasecomment# > #0# (|releasecomment|)} [{format}]";
        let out = render_scheme(&ctx, &meta, "FLAC", s).unwrap();
        assert_eq!(out, "The Wall [FLAC]");

        let meta2 = map(&[("album", "The Wall"), ("releasecomment", "Remaster")]);
        let out2 = render_scheme(&ctx, &meta2, "FLAC", s).unwrap();
        assert_eq!(out2, "The Wall (Remaster) [FLAC]");
    }

    #[test]
    fn render_year_and_padded_track() {
        let ctx = mkctx(SanitizeMethod::Unicode, 12);
        let meta = map(&[("date", "1979-11-30"), ("track", "1")]);
        assert_eq!(
            render_scheme(&ctx, &meta, "FLAC", "{year}").unwrap(),
            "1979"
        );
        assert_eq!(render_scheme(&ctx, &meta, "FLAC", "{track}").unwrap(), "01");
    }

    #[test]
    fn trims_path_components_and_extension_spacing() {
        let p = trim_path_components("  Album / 01 - Intro   .flac", '/');
        assert_eq!(p, "Album/01 - Intro.flac");
    }

    #[test]
    fn builds_track_path_from_fixture_example() {
        let ctx = mkctx(SanitizeMethod::Unicode, 12);
        let album = map(&[("album", "Example Album")]);
        let track = map(&[("track", "01"), ("title", "Intro")]);
        let out = build_track_relative_path(
            &ctx,
            &album,
            &track,
            "{album} [{format}]",
            "{track} - {title}",
            "FLAC",
            "flac",
        )
        .unwrap();
        assert_eq!(out, "Example Album [FLAC]/01 - Intro.flac");
    }

    #[test]
    fn builds_log_and_cue_paths_from_schemes() {
        let ctx = mkctx(SanitizeMethod::Unicode, 12);
        let album = map(&[
            ("album", "Example Album"),
            ("disc", "1"),
            ("totaldiscs", "2"),
        ]);

        let log_path = build_log_relative_path(
            &ctx,
            &album,
            "{album} [{format}]",
            "{album}{if #totaldiscs# > #1# CD|disc|}",
            "FLAC",
        )
        .unwrap();
        assert_eq!(log_path, "Example Album [FLAC]/Example Album CD1.log");

        let cue_path = build_cue_relative_path(
            &ctx,
            &album,
            "{album} [{format}]",
            "{album}{if #totaldiscs# > #1# CD|disc|}",
            "FLAC",
        )
        .unwrap();
        assert_eq!(cue_path, "Example Album [FLAC]/Example Album CD1.cue");
    }

    #[test]
    fn builds_cover_path_with_sanitized_title_and_extension() {
        let ctx = mkctx(SanitizeMethod::Simple, 12);
        let album = map(&[("album", "Example Album")]);
        let out = build_cover_relative_path(
            &ctx,
            &album,
            "{album} [{format}]",
            "FLAC",
            "Front:Cover",
            Some("jpg"),
        )
        .unwrap();

        assert_eq!(out, "Example Album [FLAC]/Front_Cover.jpg");
    }

    #[test]
    fn resolve_output_path_creates_parent_directories_when_requested() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("cyanrip-rs-naming-path-{now}"));
        let rel = "Album [FLAC]/01 - Intro.flac";

        let abs = resolve_output_path(&root, rel, true).expect("path resolution should succeed");
        assert_eq!(abs, root.join(rel));
        assert!(
            abs.parent().is_some_and(|p| p.exists()),
            "parent directories should exist"
        );

        let cleanup = std::fs::remove_dir_all(&root);
        assert!(cleanup.is_ok(), "temporary output root should be removable");
    }

    #[test]
    fn detects_track_path_collisions() {
        let collisions = detect_track_path_collisions(&[
            (1, "same/file.flac".to_string()),
            (2, "other/file.flac".to_string()),
            (3, "same/file.flac".to_string()),
        ]);

        assert_eq!(collisions.len(), 1);
        assert_eq!(collisions[0], (1, 3, "same/file.flac".to_string()));
    }
}
