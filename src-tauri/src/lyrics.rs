// Unified multi-provider lyrics lookup (YTMusic + LRCLib + LyricsPlus + NetEase + KuGou + LyricsOVH)

use base64::Engine as _;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Mutex, OnceLock,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const LRCLIB_BASE: &str = "https://lrclib.net/api";
const LYRICSPLUS_URLS: &[&str] = &[
    "https://lyricsplus.prjktla.my.id/v2/lyrics/get",
    "https://lyricsplus.prjktla.workers.dev/v2/lyrics/get",
];
const NETEASE_SEARCH_URL: &str = "https://music.163.com/api/cloudsearch/pc";
const NETEASE_LYRIC_URL: &str = "https://music.163.com/api/song/lyric";
const KUGOU_SEARCH_URL: &str = "https://lyrics.kugou.com/search";
const KUGOU_DOWNLOAD_URL: &str = "https://lyrics.kugou.com/download";
const LYRICSOVH_BASE: &str = "https://api.lyrics.ovh/v1";
const TRANSLATE_URL: &str = "https://translate.googleapis.com/translate_a/single?client=gtx&dt=t";
const YTM_BROWSE_URL: &str = "https://ytmbrowseproxy.zvz.be/browse?prettyPrint=false";

const MAX_BROWSE_ID_LEN: usize = 96;
const MAX_CACHE_ITEMS: usize = 128;
const MAX_TRANSLATE_CACHE_ITEMS: usize = 2048;
const MAX_TRANSLATE_LINES_PER_SONG: usize = 250;
const TRANSLATE_CHUNK_SIZE: usize = 18;
const SYNC_GRACE_MS: u64 = 120;
const RACE_DEADLINE_MS: u64 = 3_200;
const MAX_RETRY_AFTER_SECONDS: u64 = 86_400;
const DEFAULT_RETRY_AFTER_SECONDS: u64 = 60;
const MAX_TRAILING_DURATION_MS: i64 = 3_000;
const LEADING_PAD_THRESHOLD_MS: i64 = 300;

static BLOCKED_UNTIL_MS: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Default, Deserialize)]
pub struct TrackQuery {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub duration: f64,
    #[serde(default)]
    pub browse_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct LyricsResult {
    pub title: String,
    pub artists: Vec<String>,
    pub lines: Option<Vec<LyricLine>>,
    pub lyrics: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LyricLine {
    pub time: String,
    pub time_in_ms: i64,
    pub duration: i64,
    pub text: String,
    pub words: Vec<LyricWord>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LyricWord {
    pub time_in_ms: i64,
    pub word: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct TranslateRequest {
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub lines: Vec<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct TranslateResponse {
    pub translations: Vec<Option<String>>,
}

#[derive(Clone, Copy)]
enum MatchMode {
    Strict,
    Relaxed,
}

#[derive(Clone)]
struct ScoredLyrics {
    priority_bonus: f64,
    match_score: f64,
    result: LyricsResult,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LrcLibItem {
    #[serde(default)]
    track_name: Option<String>,
    #[serde(default)]
    artist_name: Option<String>,
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default)]
    instrumental: bool,
    #[serde(default)]
    synced_lyrics: Option<String>,
    #[serde(default)]
    plain_lyrics: Option<String>,
}

impl LrcLibItem {
    fn has_synced(&self) -> bool {
        !self.instrumental && non_empty(&self.synced_lyrics)
    }

    fn has_lyrics(&self) -> bool {
        !self.instrumental && (non_empty(&self.synced_lyrics) || non_empty(&self.plain_lyrics))
    }

    fn score_with(&self, query: &TrackQuery, mode: MatchMode) -> f64 {
        let base = score_candidate_mode(
            self.artist_name.as_deref().unwrap_or_default(),
            self.track_name.as_deref().unwrap_or_default(),
            self.duration.unwrap_or(0.0),
            query,
            mode,
        );
        if base <= 0.0 {
            return base;
        }
        base + if self.has_synced() { 0.35 } else { 0.0 }
    }
}

fn non_empty(value: &Option<String>) -> bool {
    value
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
}

enum Fetch<T> {
    Found(T),
    Missing,
    RateLimited,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProviderKind {
    YtMusic,
    LyricsPlus,
    LrcLib,
    NetEase,
    KuGou,
}

struct RaceMsg {
    kind: ProviderKind,
    result: Option<ScoredLyrics>,
    ytm_err: Option<String>,
}

fn cache_store() -> &'static Mutex<HashMap<String, LyricsResult>> {
    static CACHE: OnceLock<Mutex<HashMap<String, LyricsResult>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cache_key(query: &TrackQuery) -> String {
    format!(
        "{}|{}|{}|{}",
        query.browse_id.as_deref().unwrap_or(""),
        query.artist.trim().to_lowercase(),
        query.title.trim().to_lowercase(),
        query.duration.round() as i64
    )
}

/// Fast-racing unified lookup: checks instant cache first, launches YTMusic + LRCLib +
/// LyricsPlus + NetEase + KuGou concurrently via mpsc channel and returns immediately on
/// the first synced match (with a tiny grace window for official YTMusic/Apple Music timing).
pub fn lookup(query: &TrackQuery) -> Result<Option<LyricsResult>, String> {
    let key = cache_key(query);
    if !key.trim().is_empty() {
        if let Ok(guard) = cache_store().lock() {
            if let Some(hit) = guard.get(&key) {
                if has_usable_synced_lines(hit) {
                    return Ok(Some(hit.clone()));
                }
            }
        }
    }

    let (tx, rx) = mpsc::channel::<RaceMsg>();
    let mut expected = 0usize;

    if let Some(browse_id) = query.browse_id.clone() {
        let tx_ytm = tx.clone();
        let q = query.clone();
        expected += 1;
        thread::spawn(move || {
            let msg = match lookup_ytm(&browse_id, &q) {
                Ok(Some(ytm)) => RaceMsg {
                    kind: ProviderKind::YtMusic,
                    result: Some(ScoredLyrics {
                        priority_bonus: 0.40,
                        match_score: 1.5,
                        result: ytm,
                    }),
                    ytm_err: None,
                },
                Ok(None) => RaceMsg {
                    kind: ProviderKind::YtMusic,
                    result: None,
                    ytm_err: None,
                },
                Err(err) => RaceMsg {
                    kind: ProviderKind::YtMusic,
                    result: None,
                    ytm_err: Some(err),
                },
            };
            let _ = tx_ytm.send(msg);
        });
    }

    if !query.title.trim().is_empty() {
        let spawn_worker = |kind: ProviderKind, f: fn(&TrackQuery) -> Option<ScoredLyrics>| {
            let tx_w = tx.clone();
            let q = query.clone();
            thread::spawn(move || {
                let _ = tx_w.send(RaceMsg {
                    kind,
                    result: f(&q),
                    ytm_err: None,
                });
            });
        };
        spawn_worker(ProviderKind::LrcLib, lookup_lrclib_scored);
        spawn_worker(ProviderKind::LyricsPlus, lookup_lyricsplus);
        spawn_worker(ProviderKind::NetEase, lookup_netease);
        spawn_worker(ProviderKind::KuGou, lookup_kugou);
        expected += 4;
    }
    drop(tx);

    if expected == 0 {
        return Ok(None);
    }

    let hard_deadline = Instant::now() + Duration::from_millis(RACE_DEADLINE_MS);
    let mut grace_deadline: Option<Instant> = None;
    let mut ytm_fallback: Option<LyricsResult> = None;
    let mut ytm_error: Option<String> = None;
    let mut external: Vec<ScoredLyrics> = Vec::with_capacity(expected);
    let mut received = 0usize;

    while received < expected {
        let now = Instant::now();
        let wait_until = match grace_deadline {
            Some(gd) => gd.min(hard_deadline),
            None => hard_deadline,
        };
        let Some(timeout) = wait_until.checked_duration_since(now) else {
            break;
        };
        let Ok(msg) = rx.recv_timeout(timeout) else {
            break;
        };
        received += 1;

        if msg.kind == ProviderKind::YtMusic {
            ytm_error = msg.ytm_err;
            if let Some(scored) = msg.result {
                if has_usable_synced_lines(&scored.result) {
                    store_cache(&key, &scored.result);
                    return Ok(Some(scored.result));
                }
                ytm_fallback = Some(scored.result);
            }
            continue;
        }

        if let Some(scored) = msg.result {
            let is_synced = has_usable_synced_lines(&scored.result);
            let is_high_prio = msg.kind == ProviderKind::LyricsPlus;
            external.push(scored);
            if is_synced {
                if is_high_prio && query.browse_id.is_none() {
                    break;
                }
                if grace_deadline.is_none() {
                    grace_deadline = Some(Instant::now() + Duration::from_millis(SYNC_GRACE_MS));
                }
            }
        }
    }

    if let Some(err) = ytm_error.filter(|_| query.title.trim().is_empty()) {
        return Err(err);
    }

    if let Some(best) = select_best_unified(ytm_fallback, external) {
        if has_usable_synced_lines(&best) {
            store_cache(&key, &best);
        }
        return Ok(Some(best));
    }

    if !query.title.trim().is_empty() {
        if let Some(ovh) = lookup_lyrics_ovh(query) {
            return Ok(Some(ovh));
        }
    }

    Ok(None)
}

fn store_cache(key: &str, value: &LyricsResult) {
    if key.trim().is_empty() {
        return;
    }
    if let Ok(mut guard) = cache_store().lock() {
        if guard.len() >= MAX_CACHE_ITEMS {
            if let Some(stale) = guard.keys().next().cloned() {
                guard.remove(&stale);
            }
        }
        guard.insert(key.to_string(), value.clone());
    }
}

fn translate_cache_store() -> &'static Mutex<HashMap<String, Option<String>>> {
    static TRANSLATE_CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
    TRANSLATE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn normalize_target_lang(raw: &str) -> String {
    let trimmed = raw.trim().to_ascii_lowercase();
    if trimmed.is_empty()
        || trimmed.len() > 16
        || !trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return "ru".to_string();
    }
    if trimmed.starts_with("zh") {
        return if trimmed.contains("tw") || trimmed.contains("hant") {
            "zh-TW".to_string()
        } else {
            "zh-CN".to_string()
        };
    }
    let base = trimmed.split(['-', '_']).next().unwrap_or("ru");
    if base.is_empty() {
        "ru".to_string()
    } else {
        base.to_string()
    }
}

fn clean_translation_line(original: &str, translated: &str) -> Option<String> {
    let orig = original.trim();
    let mut tr = translated.trim().to_string();
    if tr.is_empty() {
        return None;
    }
    // Strip spurious trailing period added by machine translators when song line has none
    if tr.ends_with('.')
        && !tr.ends_with("..")
        && !orig.ends_with(['.', '!', '?', '…', ':'])
    {
        tr.pop();
        tr = tr.trim_end().to_string();
    }
    if tr.is_empty() {
        return None;
    }
    let norm = |s: &str| -> String {
        s.chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect()
    };
    let n_orig = norm(orig);
    let n_tr = norm(&tr);
    if n_tr.is_empty() || n_orig == n_tr {
        return None;
    }
    Some(tr)
}

fn parse_translate_payload(json: &serde_json::Value) -> (Option<String>, String) {
    let detected = json
        .get(2)
        .and_then(serde_json::Value::as_str)
        .map(normalize_target_lang);
    let text = json
        .get(0)
        .and_then(serde_json::Value::as_array)
        .map(|segments| {
            segments
                .iter()
                .filter_map(|seg| seg.get(0).and_then(serde_json::Value::as_str))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default();
    (detected, text)
}

fn translate_chunk_exact(
    http: &reqwest::blocking::Client,
    target: &str,
    chunk: &[&str],
) -> (bool, Vec<Option<String>>) {
    if chunk.is_empty() {
        return (true, Vec::new());
    }
    let joined = chunk.join("\n");
    let Ok(resp) = http
        .post(TRANSLATE_URL)
        .form(&[("sl", "auto"), ("tl", target), ("q", &joined)])
        .send()
    else {
        return (false, vec![None; chunk.len()]);
    };
    if !resp.status().is_success() {
        return (false, vec![None; chunk.len()]);
    }
    let Ok(json) = resp.json::<serde_json::Value>() else {
        return (false, vec![None; chunk.len()]);
    };

    let (detected_src, full_text) = parse_translate_payload(&json);
    if chunk.len() == 1 && detected_src.as_deref() == Some(target) {
        return (true, vec![None]);
    }

    let split_lines: Vec<&str> = full_text.lines().collect();
    if split_lines.len() == chunk.len() {
        let out = chunk
            .iter()
            .zip(split_lines.iter())
            .map(|(orig, tr)| clean_translation_line(orig, tr))
            .collect();
        return (true, out);
    }

    // Never allow shifted translations if the backend coalesced or added a newline: bisect to exact alignment
    if chunk.len() > 1 {
        let mid = chunk.len() / 2;
        let (ok_left, mut left) = translate_chunk_exact(http, target, &chunk[..mid]);
        let (ok_right, right) = translate_chunk_exact(http, target, &chunk[mid..]);
        left.extend(right);
        return (ok_left && ok_right, left);
    }

    let single = full_text.replace(['\r', '\n'], " ");
    (true, vec![clean_translation_line(chunk[0], &single)])
}

/// Translates unique lyric lines in aligned chunks with per-line caching and automatic language detection.
pub fn translate_lines(request: &TranslateRequest) -> TranslateResponse {
    let total = request.lines.len().min(MAX_TRANSLATE_LINES_PER_SONG);
    let mut results: Vec<Option<String>> = vec![None; request.lines.len()];
    if total == 0 {
        return TranslateResponse { translations: results };
    }
    let target = normalize_target_lang(&request.target);

    let mut unique_indices: HashMap<String, Vec<usize>> = HashMap::new();
    for (idx, line) in request.lines.iter().take(total).enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        unique_indices
            .entry(trimmed.to_string())
            .or_default()
            .push(idx);
    }

    let mut pending: Vec<String> = Vec::new();
    if let Ok(cache) = translate_cache_store().lock() {
        for orig in unique_indices.keys() {
            let key = format!("{target}|{orig}");
            if let Some(cached) = cache.get(&key) {
                if let Some(indices) = unique_indices.get(orig) {
                    for &idx in indices {
                        results[idx] = cached.clone();
                    }
                }
            } else {
                pending.push(orig.clone());
            }
        }
    } else {
        pending.extend(unique_indices.keys().cloned());
    }

    if pending.is_empty() {
        return TranslateResponse { translations: results };
    }

    let Ok(http) = client() else {
        return TranslateResponse { translations: results };
    };

    let mut resolved: Vec<(String, Option<String>)> = Vec::with_capacity(pending.len());
    for chunk in pending.chunks(TRANSLATE_CHUNK_SIZE) {
        let refs: Vec<&str> = chunk.iter().map(String::as_str).collect();
        let (ok, translated) = translate_chunk_exact(http, &target, &refs);
        for (orig, tr) in chunk.iter().zip(translated.into_iter()) {
            if let Some(indices) = unique_indices.get(orig) {
                for &idx in indices {
                    results[idx] = tr.clone();
                }
            }
            if ok || tr.is_some() {
                resolved.push((orig.clone(), tr));
            }
        }
    }

    if let Ok(mut cache) = translate_cache_store().lock() {
        for (orig, tr) in resolved {
            if cache.len() >= MAX_TRANSLATE_CACHE_ITEMS {
                if let Some(stale) = cache.keys().next().cloned() {
                    cache.remove(&stale);
                }
            }
            cache.insert(format!("{target}|{orig}"), tr);
        }
    }

    TranslateResponse { translations: results }
}

fn select_best_unified(
    ytm_fallback: Option<LyricsResult>,
    candidates: Vec<ScoredLyrics>,
) -> Option<LyricsResult> {
    if let Some(ytm) = ytm_fallback.as_ref() {
        if has_usable_synced_lines(ytm) {
            return Some(ytm.clone());
        }
    }

    if let Some(best_synced) = candidates
        .iter()
        .filter(|c| has_usable_synced_lines(&c.result))
        .max_by(|a, b| {
            (a.priority_bonus + a.match_score).total_cmp(&(b.priority_bonus + b.match_score))
        })
    {
        return Some(best_synced.result.clone());
    }

    if let Some(ytm) = ytm_fallback.filter(|r| has_plain_text(r)) {
        return Some(ytm);
    }

    candidates
        .into_iter()
        .filter(|c| has_plain_text(&c.result))
        .max_by(|a, b| {
            (a.priority_bonus + a.match_score).total_cmp(&(b.priority_bonus + b.match_score))
        })
        .map(|best_plain| best_plain.result)
}

fn has_usable_synced_lines(result: &LyricsResult) -> bool {
    result.lines.as_ref().is_some_and(|lines| {
        lines
            .iter()
            .filter(|l| !l.text.trim().is_empty())
            .count()
            >= 2
    })
}

fn has_plain_text(result: &LyricsResult) -> bool {
    result
        .lyrics
        .as_deref()
        .is_some_and(|text| !text.trim().is_empty())
}

fn is_valid_browse_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_BROWSE_ID_LEN
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn lookup_ytm(browse_id: &str, query: &TrackQuery) -> Result<Option<LyricsResult>, String> {
    if !is_valid_browse_id(browse_id) {
        return Err("invalid browse id".to_string());
    }
    let body = serde_json::json!({
        "browseId": browse_id,
        "context": { "client": { "clientName": "26", "clientVersion": "7.01.05" } },
    });
    let response = client()?
        .post(YTM_BROWSE_URL)
        .header(reqwest::header::ACCEPT, "application/json")
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body.to_string())
        .send()
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Ok(None);
    }
    let browse: serde_json::Value = response.json().map_err(|error| error.to_string())?;
    Ok(parse_ytm_browse(&browse, query))
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn join_runs(value: Option<&serde_json::Value>) -> Option<String> {
    let runs = value?.as_array()?;
    let text = runs
        .iter()
        .filter_map(|run| run.get("text").and_then(serde_json::Value::as_str))
        .collect::<Vec<_>>()
        .join("\n");
    (!text.is_empty()).then_some(text)
}

fn ms_field(value: &serde_json::Value) -> Option<i64> {
    match value {
        serde_json::Value::String(text) => text.trim().parse().ok(),
        serde_json::Value::Number(number) => number.as_i64(),
        _ => None,
    }
}

pub fn parse_ytm_browse(browse: &serde_json::Value, query: &TrackQuery) -> Option<LyricsResult> {
    let contents = browse.get("contents")?;
    let timed = contents
        .pointer("/elementRenderer/newElement/type/componentType/model/timedLyricsModel/lyricsData/timedLyricsData")
        .and_then(serde_json::Value::as_array);

    let mut lines: Option<Vec<LyricLine>> = timed
        .filter(|items| {
            items
                .first()
                .is_some_and(|item| item.get("cueRange").is_some())
        })
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let start = ms_field(item.pointer("/cueRange/startTimeMilliseconds")?)?;
                    let end = ms_field(item.pointer("/cueRange/endTimeMilliseconds")?)?;
                    if end < start {
                        return None;
                    }
                    let text = collapse_whitespace(
                        item.get("lyricLine")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or_default(),
                    );
                    Some(LyricLine {
                        time: format_time(start),
                        time_in_ms: start,
                        duration: end - start,
                        text: if text == "\u{266a}" {
                            String::new()
                        } else {
                            text
                        },
                        words: Vec::new(),
                    })
                })
                .collect()
        });
    if let Some(list) = lines.as_mut() {
        if let Some(first) = list.first().map(|line| line.time_in_ms) {
            if first > LEADING_PAD_THRESHOLD_MS {
                list.insert(
                    0,
                    LyricLine {
                        time: format_time(0),
                        time_in_ms: 0,
                        duration: first,
                        text: String::new(),
                        words: Vec::new(),
                    },
                );
            }
        }
    }
    let lines = lines.filter(|list| !list.is_empty());

    let lyrics = if lines.is_some() {
        None
    } else if let Some(items) = timed {
        Some(
            items
                .iter()
                .filter_map(|item| item.get("lyricLine").and_then(serde_json::Value::as_str))
                .collect::<Vec<_>>()
                .join("\n"),
        )
    } else {
        join_runs(contents.pointer("/messageRenderer/text/runs")).or_else(|| {
            join_runs(contents.pointer(
                "/sectionListRenderer/contents/0/musicDescriptionShelfRenderer/description/runs",
            ))
        })
    };
    let lyrics = lyrics.filter(|text| !text.trim().is_empty() && text != "Lyrics not available");

    if lines.is_none() && lyrics.is_none() {
        return None;
    }
    Some(LyricsResult {
        title: query.title.clone(),
        artists: vec![query.artist.clone()],
        lines,
        lyrics,
    })
}

fn lookup_lrclib_scored(query: &TrackQuery) -> Option<ScoredLyrics> {
    if is_rate_limited() || query.title.trim().is_empty() {
        return None;
    }
    let client = client().ok()?;
    let clean_title = clean_song_title(&query.title);
    let clean_art = clean_artist(&query.artist);
    let primary_art = primary_artist_token(&clean_art);

    let mut plain_fallback: Option<(f64, LrcLibItem)> = None;

    if !clean_title.is_empty() && !primary_art.is_empty() {
        let mut params = vec![
            ("track_name", clean_title.clone()),
            ("artist_name", primary_art.clone()),
        ];
        if query.duration > 0.0 {
            params.push(("duration", format!("{}", query.duration.round() as i64)));
        }
        match fetch_json::<LrcLibItem>(&client, "get", &params) {
            Fetch::RateLimited => return None,
            Fetch::Found(item) if item.has_lyrics() => {
                let s = item.score_with(query, MatchMode::Strict);
                if s > 0.0 {
                    if item.has_synced() {
                        return Some(ScoredLyrics {
                            priority_bonus: 0.12,
                            match_score: s,
                            result: into_result(item, query, false),
                        });
                    }
                    plain_fallback = Some((s, item));
                }
            }
            _ => {}
        }
    }

    let artist_or_raw = if clean_art.is_empty() {
        query.artist.clone()
    } else {
        clean_art.clone()
    };
    let title_or_raw = if clean_title.is_empty() {
        query.title.clone()
    } else {
        clean_title.clone()
    };

    let mut searches: Vec<Vec<(&str, String)>> = vec![vec![(
        "q",
        format!("{primary_art} {title_or_raw}").trim().to_string(),
    )]];
    if !clean_title.is_empty() && !primary_art.is_empty() {
        searches.push(vec![
            ("track_name", clean_title.clone()),
            ("artist_name", primary_art.clone()),
        ]);
    }
    if artist_or_raw != primary_art && !clean_title.is_empty() {
        searches.push(vec![(
            "q",
            format!("{artist_or_raw} {title_or_raw}").trim().to_string(),
        )]);
    }

    let mut collected: Vec<LrcLibItem> = Vec::new();
    for params in searches {
        match fetch_json::<Vec<LrcLibItem>>(&client, "search", &params) {
            Fetch::RateLimited => break,
            Fetch::Found(items) if !items.is_empty() => {
                let has_synced_hit = best_lrclib_with_mode(&items, query, MatchMode::Strict)
                    .is_some_and(|(_, item)| item.has_synced());
                collected.extend(items);
                if has_synced_hit {
                    break;
                }
            }
            _ => {}
        }
    }

    if let Some((score, item)) = best_lrclib_candidate(collected, query) {
        if item.has_synced() {
            return Some(ScoredLyrics {
                priority_bonus: 0.12,
                match_score: score,
                result: into_result(item, query, true),
            });
        }
        if plain_fallback
            .as_ref()
            .is_none_or(|(prev_s, _)| score > *prev_s)
        {
            plain_fallback = Some((score, item));
        }
    }

    plain_fallback.map(|(score, item)| ScoredLyrics {
        priority_bonus: 0.08,
        match_score: score,
        result: into_result(item, query, true),
    })
}

fn best_lrclib_with_mode<'a>(
    items: &'a [LrcLibItem],
    query: &TrackQuery,
    mode: MatchMode,
) -> Option<(f64, &'a LrcLibItem)> {
    items
        .iter()
        .filter(|item| item.has_lyrics())
        .map(|item| (item.score_with(query, mode), item))
        .filter(|(score, _)| *score > 0.0)
        .max_by(|(left, _), (right, _)| left.total_cmp(right))
}

fn best_lrclib_candidate(items: Vec<LrcLibItem>, query: &TrackQuery) -> Option<(f64, LrcLibItem)> {
    if let Some((idx, score)) = items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.has_lyrics())
        .map(|(i, item)| (i, item.score_with(query, MatchMode::Strict)))
        .filter(|(_, s)| *s > 0.0)
        .max_by(|(_, a), (_, b)| a.total_cmp(b))
    {
        let chosen = items.into_iter().nth(idx)?;
        if chosen.has_synced() {
            return Some((score, chosen));
        }
        // Fallback pass: if strict only had plain lyrics, check if relaxed duration match has synced lines
        return Some((score, chosen));
    }

    items
        .into_iter()
        .filter(LrcLibItem::has_lyrics)
        .map(|item| (item.score_with(query, MatchMode::Relaxed), item))
        .filter(|(score, _)| *score > 0.0)
        .max_by(|(left, _), (right, _)| left.total_cmp(right))
}

#[cfg(test)]
fn best_candidate(items: Vec<LrcLibItem>, query: &TrackQuery) -> Option<LrcLibItem> {
    best_lrclib_candidate(items, query).map(|(_, item)| item)
}

fn into_result(item: LrcLibItem, query: &TrackQuery, split_artists: bool) -> LyricsResult {
    let artists = match item.artist_name.as_deref().filter(|name| !name.is_empty()) {
        Some(name) if split_artists => name
            .split(['&', ','])
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(str::to_string)
            .collect(),
        Some(name) => vec![name.to_string()],
        None => vec![query.artist.clone()],
    };
    LyricsResult {
        title: item
            .track_name
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| query.title.clone()),
        artists,
        lines: item
            .synced_lyrics
            .filter(|text| !text.trim().is_empty())
            .map(|text| parse_lrc(&text))
            .filter(|lines| !lines.is_empty()),
        lyrics: item.plain_lyrics.filter(|text| !text.trim().is_empty()),
    }
}

/// Queries LyricsPlus (Apple Music / Musixmatch / Spotify aggregator) for line & word timing.
fn lookup_lyricsplus(query: &TrackQuery) -> Option<ScoredLyrics> {
    if query.title.trim().is_empty() || query.artist.trim().is_empty() {
        return None;
    }
    let client = client().ok()?;
    let clean_title = clean_song_title(&query.title);
    let primary_art = primary_artist_token(&clean_artist(&query.artist));
    if clean_title.is_empty() || primary_art.is_empty() {
        return None;
    }

    for base in LYRICSPLUS_URLS {
        let Ok(mut url) = url::Url::parse(base) else {
            continue;
        };
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("title", &clean_title);
            pairs.append_pair("artist", &primary_art);
            if let Some(album) = query.album.as_deref().filter(|a| !a.trim().is_empty()) {
                pairs.append_pair("album", album);
            }
            if query.duration > 0.0 {
                pairs.append_pair("duration", &format!("{}", query.duration.round() as i64));
            }
            pairs.append_pair("source", "apple,lyricsplus,musixmatch,spotify");
        }
        let Ok(resp) = client.get(url).send() else {
            continue;
        };
        if !resp.status().is_success() {
            continue;
        }
        let Ok(json) = resp.json::<serde_json::Value>() else {
            continue;
        };
        if let Some(parsed) = parse_lyricsplus_json(&json, query) {
            return Some(parsed);
        }
    }
    None
}

fn parse_lyricsplus_json(json: &serde_json::Value, query: &TrackQuery) -> Option<ScoredLyrics> {
    if json.get("error").and_then(serde_json::Value::as_bool) == Some(true) {
        return None;
    }

    let meta = json
        .get("selectedSongMetadata")
        .or_else(|| json.get("metadata"));
    let meta_title = meta
        .and_then(|m| m.get("title"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or(&query.title);
    let meta_artist = meta
        .and_then(|m| m.get("artist"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or(&query.artist);
    let meta_duration = meta
        .and_then(|m| m.get("duration"))
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.0);

    let match_score = if meta.is_some()
        && meta
            .and_then(|m| m.get("title"))
            .and_then(serde_json::Value::as_str)
            .is_some()
    {
        let strict = score_candidate_mode(
            meta_artist,
            meta_title,
            meta_duration,
            query,
            MatchMode::Strict,
        );
        if strict > 0.0 {
            strict
        } else {
            score_candidate_mode(
                meta_artist,
                meta_title,
                meta_duration,
                query,
                MatchMode::Relaxed,
            )
        }
    } else {
        0.9
    };

    if match_score <= 0.0 {
        return None;
    }

    let raw_lines = json
        .get("lyrics")
        .and_then(serde_json::Value::as_array)
        .or_else(|| json.get("syncedLyrics").and_then(serde_json::Value::as_array))?;

    let mut lines: Vec<LyricLine> = Vec::with_capacity(raw_lines.len());
    for item in raw_lines {
        let start = item
            .get("time")
            .or_else(|| item.get("startTime"))
            .and_then(ms_field);
        let Some(start_ms) = start else {
            continue;
        };
        let end_ms = item.get("endTime").and_then(ms_field);
        let dur_ms = item
            .get("duration")
            .and_then(ms_field)
            .or_else(|| end_ms.map(|e| (e - start_ms).max(0)))
            .unwrap_or(MAX_TRAILING_DURATION_MS);
        let raw_text = item
            .get("text")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let text = collapse_whitespace(raw_text);
        if text.is_empty() || is_credit_line(&text) {
            continue;
        }
        let words: Vec<LyricWord> = item
            .get("syllabus")
            .or_else(|| item.get("syls"))
            .and_then(serde_json::Value::as_array)
            .map(|syls| {
                syls.iter()
                    .filter_map(|syl| {
                        let w_time = syl
                            .get("time")
                            .or_else(|| syl.get("startTime"))
                            .and_then(ms_field)?;
                        let w_text = syl
                            .get("text")
                            .and_then(serde_json::Value::as_str)?
                            .trim()
                            .to_string();
                        (!w_text.is_empty()).then_some(LyricWord {
                            time_in_ms: w_time,
                            word: w_text,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        lines.push(LyricLine {
            time: format_time(start_ms),
            time_in_ms: start_ms,
            duration: dur_ms.max(0),
            text,
            words,
        });
    }

    if lines.is_empty() {
        return None;
    }

    lines.sort_by_key(|l| l.time_in_ms);
    if let Some(first) = lines.first().map(|l| l.time_in_ms) {
        if first > LEADING_PAD_THRESHOLD_MS {
            lines.insert(
                0,
                LyricLine {
                    time: format_time(0),
                    time_in_ms: 0,
                    duration: first,
                    text: String::new(),
                    words: Vec::new(),
                },
            );
        }
    }

    let res = LyricsResult {
        title: meta_title.to_string(),
        artists: vec![meta_artist.to_string()],
        lines: Some(lines),
        lyrics: None,
    };
    if !has_usable_synced_lines(&res) {
        return None;
    }
    Some(ScoredLyrics {
        priority_bonus: 0.15,
        match_score: match_score + 0.35,
        result: res,
    })
}

/// Queries NetEase Cloud Music (`music.163.com`) for broad indie / Russian / electronic / pop coverage.
fn lookup_netease(query: &TrackQuery) -> Option<ScoredLyrics> {
    if query.title.trim().is_empty() {
        return None;
    }
    let client = client().ok()?;
    let clean_title = clean_song_title(&query.title);
    let clean_art = clean_artist(&query.artist);
    let primary_art = primary_artist_token(&clean_art);
    let search_q = format!(
        "{} {}",
        primary_art,
        if clean_title.is_empty() {
            &query.title
        } else {
            &clean_title
        }
    )
    .trim()
    .to_string();
    if search_q.is_empty() {
        return None;
    }

    let Ok(mut url) = url::Url::parse(NETEASE_SEARCH_URL) else {
        return None;
    };
    url.query_pairs_mut()
        .append_pair("s", &search_q)
        .append_pair("type", "1")
        .append_pair("limit", "8");

    let resp = client
        .post(url)
        .header(reqwest::header::REFERER, "https://music.163.com")
        .header(reqwest::header::CONTENT_LENGTH, "0")
        .body("")
        .send()
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let search_json: serde_json::Value = resp.json().ok()?;
    let songs = search_json.pointer("/result/songs")?.as_array()?;

    let mut ranked: Vec<(f64, u64, String, Vec<String>)> = Vec::new();
    for mode in [MatchMode::Strict, MatchMode::Relaxed] {
        ranked.clear();
        for song in songs {
            let Some(id) = song.get("id").and_then(serde_json::Value::as_u64) else {
                continue;
            };
            let title = song
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let artists: Vec<String> = song
                .get("ar")
                .and_then(serde_json::Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(|a| a.get("name").and_then(serde_json::Value::as_str))
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            let joined_artist = artists.join(", ");
            let dur_sec = song
                .get("dt")
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(0.0)
                / 1000.0;
            let score = score_candidate_mode(&joined_artist, title, dur_sec, query, mode);
            if score > 0.0 {
                ranked.push((score, id, title.to_string(), artists));
            }
        }
        if !ranked.is_empty() {
            break;
        }
    }

    ranked.sort_by(|a, b| b.0.total_cmp(&a.0));

    let mut plain_candidate: Option<ScoredLyrics> = None;
    for (score, id, song_title, artists) in ranked.into_iter().take(2) {
        let Ok(mut lyric_url) = url::Url::parse(NETEASE_LYRIC_URL) else {
            continue;
        };
        lyric_url
            .query_pairs_mut()
            .append_pair("id", &id.to_string())
            .append_pair("lv", "1")
            .append_pair("tv", "-1");
        let Ok(l_resp) = client
            .get(lyric_url)
            .header(reqwest::header::REFERER, "https://music.163.com")
            .send()
        else {
            continue;
        };
        if !l_resp.status().is_success() {
            continue;
        }
        let Ok(l_json) = l_resp.json::<serde_json::Value>() else {
            continue;
        };
        if l_json.get("nolyric").and_then(serde_json::Value::as_bool) == Some(true) {
            continue;
        }
        let Some(raw_lrc) = l_json
            .pointer("/lrc/lyric")
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        let parsed_lines = parse_lrc(raw_lrc);
        let result = LyricsResult {
            title: song_title,
            artists: if artists.is_empty() {
                vec![query.artist.clone()]
            } else {
                artists
            },
            lines: (!parsed_lines.is_empty()).then_some(parsed_lines),
            lyrics: None,
        };
        if has_usable_synced_lines(&result) {
            return Some(ScoredLyrics {
                priority_bonus: 0.08,
                match_score: score + 0.35,
                result,
            });
        }
        let plain = strip_lrc_to_plain(raw_lrc);
        if !plain.is_empty() && plain_candidate.is_none() {
            plain_candidate = Some(ScoredLyrics {
                priority_bonus: 0.04,
                match_score: score,
                result: LyricsResult {
                    title: result.title,
                    artists: result.artists,
                    lines: None,
                    lyrics: Some(plain),
                },
            });
        }
    }

    plain_candidate
}

/// Queries KuGou (`lyrics.kugou.com`) synced LRC catalog as backup global provider.
fn lookup_kugou(query: &TrackQuery) -> Option<ScoredLyrics> {
    if query.title.trim().is_empty() {
        return None;
    }
    let client = client().ok()?;
    let clean_title = clean_song_title(&query.title);
    let primary_art = primary_artist_token(&clean_artist(&query.artist));
    let keyword = if primary_art.is_empty() {
        clean_title.clone()
    } else {
        format!("{primary_art} - {clean_title}")
    };
    if keyword.trim().is_empty() {
        return None;
    }

    let Ok(mut url) = url::Url::parse(KUGOU_SEARCH_URL) else {
        return None;
    };
    {
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("ver", "1");
        pairs.append_pair("man", "yes");
        pairs.append_pair("client", "pc");
        pairs.append_pair("keyword", &keyword);
        if query.duration > 0.0 {
            pairs.append_pair("duration", &format!("{}", (query.duration * 1000.0).round() as i64));
        }
    }

    let resp = client.get(url).send().ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let search_json: serde_json::Value = resp.json().ok()?;
    let candidates = search_json.get("candidates")?.as_array()?;

    let mut ranked: Vec<(f64, String, String, String, String)> = Vec::new();
    for mode in [MatchMode::Strict, MatchMode::Relaxed] {
        ranked.clear();
        for cand in candidates {
            let Some(id) = cand.get("id").and_then(serde_json::Value::as_str) else {
                continue;
            };
            let Some(accesskey) = cand.get("accesskey").and_then(serde_json::Value::as_str) else {
                continue;
            };
            let song = cand
                .get("song")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let singer = cand
                .get("singer")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let dur_sec = cand
                .get("duration")
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(0.0)
                / 1000.0;
            let score = score_candidate_mode(singer, song, dur_sec, query, mode);
            if score > 0.0 {
                ranked.push((
                    score,
                    id.to_string(),
                    accesskey.to_string(),
                    song.to_string(),
                    singer.to_string(),
                ));
            }
        }
        if !ranked.is_empty() {
            break;
        }
    }

    ranked.sort_by(|a, b| b.0.total_cmp(&a.0));

    for (score, id, accesskey, song, singer) in ranked.into_iter().take(2) {
        let Ok(mut dl_url) = url::Url::parse(KUGOU_DOWNLOAD_URL) else {
            continue;
        };
        dl_url
            .query_pairs_mut()
            .append_pair("ver", "1")
            .append_pair("client", "pc")
            .append_pair("id", &id)
            .append_pair("accesskey", &accesskey)
            .append_pair("fmt", "lrc")
            .append_pair("charset", "utf8");
        let Ok(dl_resp) = client.get(dl_url).send() else {
            continue;
        };
        if !dl_resp.status().is_success() {
            continue;
        }
        let Ok(dl_json) = dl_resp.json::<serde_json::Value>() else {
            continue;
        };
        let Some(content_b64) = dl_json.get("content").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(content_b64.trim()) else {
            continue;
        };
        let Ok(raw_lrc) = String::from_utf8(bytes) else {
            continue;
        };
        let parsed_lines = parse_lrc(&raw_lrc);
        let result = LyricsResult {
            title: if song.is_empty() {
                query.title.clone()
            } else {
                song
            },
            artists: vec![if singer.is_empty() {
                query.artist.clone()
            } else {
                singer
            }],
            lines: (!parsed_lines.is_empty()).then_some(parsed_lines),
            lyrics: None,
        };
        if has_usable_synced_lines(&result) {
            return Some(ScoredLyrics {
                priority_bonus: 0.05,
                match_score: score + 0.35,
                result,
            });
        }
    }

    None
}

/// Last-resort plain lyrics lookup via Lyrics.ovh when no source provides synced or plain text.
fn lookup_lyrics_ovh(query: &TrackQuery) -> Option<LyricsResult> {
    let clean_title = clean_song_title(&query.title);
    let primary_art = primary_artist_token(&clean_artist(&query.artist));
    if clean_title.is_empty() || primary_art.is_empty() {
        return None;
    }
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_millis(2500))
        .build()
        .ok()?;
    let Ok(mut url) = url::Url::parse(LYRICSOVH_BASE) else {
        return None;
    };
    if let Ok(mut segs) = url.path_segments_mut() {
        segs.push(&primary_art).push(&clean_title);
    }
    let resp = client.get(url).send().ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let json: serde_json::Value = resp.json().ok()?;
    let raw = json.get("lyrics").and_then(serde_json::Value::as_str)?;
    let cleaned = raw.replace("\r\n", "\n").trim().to_string();
    if cleaned.len() < 12 {
        return None;
    }
    Some(LyricsResult {
        title: query.title.clone(),
        artists: vec![query.artist.clone()],
        lines: None,
        lyrics: Some(cleaned),
    })
}

fn strip_lrc_to_plain(raw: &str) -> String {
    let p = patterns();
    raw.lines()
        .filter_map(|line| {
            let mut rest = line.trim();
            while let Some(c) = p.timestamp.captures(rest) {
                rest = &rest[c[0].len()..];
            }
            if p.tag.is_match(rest) {
                return None;
            }
            let cleaned = p.section.replace(rest.trim(), "").into_owned();
            let cleaned = cleaned.trim();
            if cleaned.is_empty() || is_credit_line(cleaned) {
                None
            } else {
                Some(cleaned.to_string())
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn client() -> Result<&'static reqwest::blocking::Client, String> {
    static CLIENT: OnceLock<Result<reqwest::blocking::Client, String>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::blocking::Client::builder()
                .connect_timeout(Duration::from_secs(2))
                .timeout(Duration::from_secs(4))
                .pool_max_idle_per_host(8)
                .user_agent(concat!(
                    "ytm-tauri/",
                    env!("CARGO_PKG_VERSION"),
                    " (https://github.com/xzelleiv/ytm-tauri)"
                ))
                .build()
                .map_err(|error| error.to_string())
        })
        .as_ref()
        .map_err(Clone::clone)
}

fn fetch_json<T: for<'de> Deserialize<'de>>(
    client: &reqwest::blocking::Client,
    endpoint: &str,
    params: &[(&str, String)],
) -> Fetch<T> {
    if is_rate_limited() {
        return Fetch::RateLimited;
    }
    let Ok(mut url) = url::Url::parse(&format!("{LRCLIB_BASE}/{endpoint}")) else {
        return Fetch::Missing;
    };
    url.query_pairs_mut()
        .extend_pairs(params.iter().map(|(k, v)| (*k, v.as_str())));
    let Ok(response) = client
        .get(url)
        .header(
            "Lrclib-Client",
            concat!("ytm-tauri/", env!("CARGO_PKG_VERSION")),
        )
        .send()
    else {
        return Fetch::Missing;
    };
    if response.status().as_u16() == 429 {
        let retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .map_or(DEFAULT_RETRY_AFTER_SECONDS, parse_retry_after);
        BLOCKED_UNTIL_MS.store(now_ms() + retry_after * 1000, Ordering::Relaxed);
        return Fetch::RateLimited;
    }
    if !response.status().is_success() {
        return Fetch::Missing;
    }
    response.json::<T>().map_or(Fetch::Missing, Fetch::Found)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis() as u64)
        .unwrap_or_default()
}

fn is_rate_limited() -> bool {
    now_ms() < BLOCKED_UNTIL_MS.load(Ordering::Relaxed)
}

fn parse_retry_after(value: &str) -> u64 {
    match value.trim().parse::<u64>() {
        Ok(seconds) if seconds > 0 => seconds.min(MAX_RETRY_AFTER_SECONDS),
        _ => DEFAULT_RETRY_AFTER_SECONDS,
    }
}

struct Patterns {
    suffixes: Vec<Regex>,
    leading_number: Regex,
    brackets: Regex,
    dash_suffix: Regex,
    featuring: Regex,
    whitespace: Regex,
    timestamp: Regex,
    tag: Regex,
    word: Regex,
    section: Regex,
    artist_split: Regex,
    credit_prefix: Regex,
}

fn patterns() -> &'static Patterns {
    static PATTERNS: OnceLock<Patterns> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        let compile = |pattern: &str| Regex::new(pattern).expect("static lyrics regex");
        Patterns {
            suffixes: [
                r"(?i)\s*(- topic)$",
                r"(?i)\s*vevo$",
                r"(?i)\s*[(|\[](official|audio|video|lyrics?|visualizer|remaster(ed)?|demo|live|extended|draft|special edition|deluxe|explicit|clean|4k|hd|hq|performance|clip|full album|slowed|reverb|sped up|sped-up|slowed\s*\+\s*reverb|color coded|eng sub|rom|han|eng|lyrics video).*?[)|\]]",
                r"(?i)\s*[(|\[](20\d\d|19\d\d)\s*(remaster|version|edition|mix|anniversary)?[)|\]]",
                r"(?i)\s*[(|\[](hd|hq|4k|upgrade|live|acoustic|instrumental)[)|\]]$",
            ]
            .into_iter()
            .map(compile)
            .collect(),
            leading_number: compile(r"^\d+[\s.-]+\s*"),
            brackets: compile(r"[(\[{][^)\]}]*[)\]}]"),
            dash_suffix: compile(r"(?i)\s*-\s*(official|audio|video|lyrics?|visualizer|remaster(ed)?|demo|live|extended|draft|special edition|deluxe).*$"),
            featuring: compile(r"(?i)\s+(feat|ft)\.?\s+.*$"),
            whitespace: compile(r"\s+"),
            timestamp: compile(r"^\[(\d+):(\d+)(?:[.,:](\d+))?\]"),
            tag: compile(r"^\[(\w+):\s*(.+?)\s*\]$"),
            word: compile(r"<(\d+):(\d+)(?:[.,:](\d+))?>\s*(\S+)"),
            section: compile(r"(?i)^\[(verse|chorus|bridge|intro|outro|hook|interlude)[^\]]*\]\s*"),
            artist_split: compile(r"(?i)(?:,|&|/|×|\s+x\s+|\s+и\s+|\s+feat\.?\s+|\s+ft\.?\s+|\s+при уч\.?\s+|\s+with\s+)"),
            credit_prefix: compile(r"(?i)^(?:作词|作曲|编曲|制作人|监制|混音|母带|录音|词|曲|和声|吉他|贝斯|鼓|键盘|Lyrics\s*by|Music\s*by|Composed\s*by|Arranged\s*by|Produced\s*by|Written\s*by|Mixed\s*by)\s*[:：]"),
        }
    })
}

fn is_credit_line(line: &str) -> bool {
    patterns().credit_prefix.is_match(line.trim())
}

fn cleanup_name(name: &str) -> String {
    let mut value = name.to_string();
    for suffix in &patterns().suffixes {
        value = suffix.replace_all(&value, "").into_owned();
    }
    value.trim().to_string()
}

pub fn clean_song_title(title: &str) -> String {
    let p = patterns();
    let value = p.leading_number.replace(title, "");
    let value = p.brackets.replace_all(&value, " ");
    let value = p.dash_suffix.replace_all(&value, " ");
    let value = p.featuring.replace_all(&value, " ");
    let value = p.whitespace.replace_all(&value, " ");
    cleanup_name(value.trim())
}

pub fn clean_artist(artist: &str) -> String {
    let p = patterns();
    let value = p.brackets.replace_all(artist, " ");
    let value = p.featuring.replace_all(&value, " ");
    let value = p.whitespace.replace_all(&value, " ");
    cleanup_name(value.trim())
}

fn split_artist_tokens(raw: &str) -> Vec<String> {
    let cleaned = clean_artist(raw).to_lowercase();
    let mut tokens: Vec<String> = patterns()
        .artist_split
        .split(&cleaned)
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect();
    if tokens.is_empty() && !cleaned.is_empty() {
        tokens.push(cleaned);
    }
    tokens
}

fn primary_artist_token(raw: &str) -> String {
    split_artist_tokens(raw)
        .into_iter()
        .next()
        .unwrap_or_else(|| clean_artist(raw))
}

fn strip_leading_artist_from_title(
    title_raw: &str,
    cand_artist: &str,
    query_artist: &str,
) -> String {
    if let Some((prefix, rest)) = title_raw.split_once(" - ") {
        let p_low = clean_artist(prefix).to_lowercase();
        let rest_clean = clean_song_title(rest);
        if !p_low.is_empty() && !rest_clean.is_empty() {
            let ca = split_artist_tokens(cand_artist);
            let qa = split_artist_tokens(query_artist);
            let prefix_matches_artist = ca
                .iter()
                .chain(qa.iter())
                .any(|tok| dice(&p_low, tok) >= 0.72 || jaro_winkler(&p_low, tok) >= 0.82);
            if prefix_matches_artist {
                return rest_clean;
            }
        }
    }
    clean_song_title(title_raw)
}

// bigram dice coefficient
fn dice(first: &str, second: &str) -> f64 {
    if first.is_empty() || second.is_empty() {
        return 0.0;
    }
    if first == second {
        return 1.0;
    }
    let first: Vec<char> = first.chars().collect();
    let second: Vec<char> = second.chars().collect();
    if first.len() < 2 || second.len() < 2 {
        return 0.0;
    }
    let mut bigrams: HashMap<(char, char), usize> = HashMap::new();
    for pair in first.windows(2) {
        *bigrams.entry((pair[0], pair[1])).or_default() += 1;
    }
    let mut intersection = 0;
    for pair in second.windows(2) {
        if let Some(count) = bigrams
            .get_mut(&(pair[0], pair[1]))
            .filter(|count| **count > 0)
        {
            *count -= 1;
            intersection += 1;
        }
    }
    (2.0 * intersection as f64) / (first.len() + second.len() - 2) as f64
}

fn jaro_winkler(left: &str, right: &str) -> f64 {
    if left == right {
        return 1.0;
    }
    let left: Vec<char> = left.chars().collect();
    let right: Vec<char> = right.chars().collect();
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let range = (left.len().max(right.len()) / 2).saturating_sub(1);
    let mut left_matches = vec![false; left.len()];
    let mut right_matches = vec![false; right.len()];
    let mut matches = 0usize;
    for (i, &ch) in left.iter().enumerate() {
        let start = i.saturating_sub(range);
        let end = (i + range + 1).min(right.len());
        for j in start..end {
            if right_matches[j] || ch != right[j] {
                continue;
            }
            left_matches[i] = true;
            right_matches[j] = true;
            matches += 1;
            break;
        }
    }
    if matches == 0 {
        return 0.0;
    }
    let left_chars = left.iter().zip(&left_matches).filter(|(_, m)| **m);
    let right_chars = right.iter().zip(&right_matches).filter(|(_, m)| **m);
    let transpositions = left_chars
        .zip(right_chars)
        .filter(|((a, _), (b, _))| a != b)
        .count() as f64
        / 2.0;
    let m = matches as f64;
    let jaro = (m / left.len() as f64 + m / right.len() as f64 + (m - transpositions) / m) / 3.0;
    let prefix = left
        .iter()
        .zip(&right)
        .take(4)
        .take_while(|(a, b)| a == b)
        .count() as f64;
    jaro + prefix * 0.1 * (1.0 - jaro)
}

fn similarity(
    target_raw: &str,
    target_clean: &str,
    candidate_raw: &str,
    candidate_clean: &str,
) -> f64 {
    [
        dice(target_clean, candidate_clean),
        dice(target_raw, candidate_clean),
        dice(target_clean, candidate_raw),
        jaro_winkler(target_clean, candidate_clean),
        jaro_winkler(target_raw, candidate_clean),
    ]
    .into_iter()
    .fold(0.0, f64::max)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn score_candidate(artist: &str, title: &str, duration: f64, query: &TrackQuery) -> f64 {
    score_candidate_mode(artist, title, duration, query, MatchMode::Strict)
}

fn score_candidate_mode(
    artist: &str,
    title: &str,
    duration: f64,
    query: &TrackQuery,
    mode: MatchMode,
) -> f64 {
    if artist.is_empty() || title.is_empty() {
        return -1.0;
    }
    let query_title_clean = strip_leading_artist_from_title(&query.title, artist, &query.artist);
    let cand_title_clean = strip_leading_artist_from_title(title, artist, &query.artist);

    let title_sim = similarity(
        &query.title.to_lowercase(),
        &query_title_clean.to_lowercase(),
        &title.to_lowercase(),
        &cand_title_clean.to_lowercase(),
    );

    let mut artist_sim = similarity(
        &query.artist.to_lowercase(),
        &clean_artist(&query.artist).to_lowercase(),
        &artist.to_lowercase(),
        &clean_artist(artist).to_lowercase(),
    );
    let q_tokens = split_artist_tokens(&query.artist);
    let c_tokens = split_artist_tokens(artist);
    for qt in &q_tokens {
        for ct in &c_tokens {
            let pair = dice(qt, ct).max(jaro_winkler(qt, ct));
            if pair > artist_sim {
                artist_sim = pair;
            }
        }
    }

    match mode {
        MatchMode::Strict => {
            if title_sim < 0.65 || artist_sim < 0.55 {
                return -1.0;
            }
        }
        MatchMode::Relaxed => {
            if title_sim < 0.84 || artist_sim < 0.78 {
                return -1.0;
            }
        }
    }

    let mut score = title_sim * 0.6 + artist_sim * 0.4;
    if query.duration > 0.0 && duration > 0.0 {
        let diff = (duration - query.duration).abs();
        match mode {
            MatchMode::Strict => {
                if diff > 4.5 {
                    return -1.0;
                }
                score += if diff <= 2.0 { 0.14 } else { 0.06 };
            }
            MatchMode::Relaxed => {
                // Allow slightly larger offset for video edits/remasters ONLY on high-confidence title+artist matches
                if diff > 18.0 {
                    return -1.0;
                }
                score += 0.02 - (diff / 100.0);
            }
        }
    }
    score
}

fn timestamp_ms(minutes: &str, seconds: &str, fraction: Option<&str>) -> i64 {
    let fraction_ms = fraction.map_or(0, |value| {
        let digits: String = value.chars().take(3).collect();
        format!("{digits:0<3}").parse::<i64>().unwrap_or(0)
    });
    minutes.parse::<i64>().unwrap_or(0) * 60_000
        + seconds.parse::<i64>().unwrap_or(0) * 1_000
        + fraction_ms
}

pub fn format_time(ms: i64) -> String {
    let ms = ms.max(0);
    format!(
        "{:02}:{:02}.{:02}",
        ms / 60_000,
        (ms % 60_000) / 1_000,
        (ms % 1_000) / 10
    )
}

pub fn parse_lrc(text: &str) -> Vec<LyricLine> {
    let p = patterns();
    let mut lines: Vec<LyricLine> = Vec::new();
    let mut offset = 0i64;

    for raw in text.lines() {
        let mut rest = raw.trim();
        if !rest.starts_with('[') {
            continue;
        }
        let mut stamps = Vec::new();
        while let Some(captures) = p.timestamp.captures(rest) {
            let minutes = &captures[1];
            let seconds = &captures[2];
            let fraction = captures.get(3).map(|m| m.as_str());
            stamps.push((
                format!("{minutes}:{seconds}.{}", fraction.unwrap_or("00")),
                timestamp_ms(minutes, seconds, fraction),
            ));
            rest = &rest[captures[0].len()..];
        }

        if stamps.is_empty() {
            if let Some(tag) = p.tag.captures(rest) {
                if &tag[1] == "offset" {
                    offset = tag[2].trim().parse().unwrap_or(0);
                }
            }
            continue;
        }

        let mut line_text = rest.trim().to_string();
        let words: Vec<LyricWord> = p
            .word
            .captures_iter(&line_text)
            .map(|captures| LyricWord {
                time_in_ms: timestamp_ms(
                    &captures[1],
                    &captures[2],
                    captures.get(3).map(|m| m.as_str()),
                ),
                word: captures[4].to_string(),
            })
            .collect();
        if !words.is_empty() {
            line_text = words
                .iter()
                .map(|word| word.word.as_str())
                .collect::<Vec<_>>()
                .join(" ");
        }
        let line_text = p.section.replace(&line_text, "").into_owned();
        if is_credit_line(&line_text) {
            continue;
        }

        for (time, time_in_ms) in stamps {
            lines.push(LyricLine {
                time,
                time_in_ms,
                duration: -1,
                text: line_text.clone(),
                words: words.clone(),
            });
        }
    }

    lines.sort_by_key(|line| line.time_in_ms);
    let starts: Vec<i64> = lines.iter().map(|line| line.time_in_ms).collect();
    for (index, line) in lines.iter_mut().enumerate() {
        line.time_in_ms += offset;
        line.duration = starts
            .get(index + 1)
            .map_or(MAX_TRAILING_DURATION_MS, |next| {
                (next - starts[index]).max(0)
            });
    }

    if let Some(first) = lines.first().map(|line| line.time_in_ms) {
        if first > LEADING_PAD_THRESHOLD_MS {
            lines.insert(
                0,
                LyricLine {
                    time: format_time(0),
                    time_in_ms: 0,
                    duration: first,
                    text: String::new(),
                    words: Vec::new(),
                },
            );
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(title: &str, artist: &str, duration: f64) -> TrackQuery {
        TrackQuery {
            title: title.to_string(),
            artist: artist.to_string(),
            album: None,
            duration,
            browse_id: None,
        }
    }

    #[test]
    fn parses_flexible_timestamps_and_strips_section_headers() {
        let lrc = "[re:LRCLIB]\n[00:10,50][Verse 1] Comma decimal\n[00:20.123][Chorus] Three digit millis\n[00:30] Integer seconds\n[00:40:50] Colon subseconds";
        let lines = parse_lrc(lrc);
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[0].time_in_ms, 0);
        assert_eq!(lines[0].duration, 10_500);
        assert_eq!(lines[1].time_in_ms, 10_500);
        assert_eq!(lines[1].text, "Comma decimal");
        assert_eq!(lines[2].time_in_ms, 20_123);
        assert_eq!(lines[2].text, "Three digit millis");
        assert_eq!(lines[3].time_in_ms, 30_000);
        assert_eq!(lines[3].text, "Integer seconds");
        assert_eq!(lines[4].time_in_ms, 40_500);
        assert_eq!(lines[4].duration, 3_000);
    }

    #[test]
    fn parses_repeated_stamps_offsets_and_word_timing() {
        let lrc =
            "[offset:+200]\n[00:01.00][00:05.00]Echo\n[00:03.00]<00:03.00> Hello <00:03.50> world";
        let lines = parse_lrc(lrc);
        let texts: Vec<_> = lines.iter().map(|line| line.text.as_str()).collect();
        assert_eq!(texts, ["", "Echo", "Hello world", "Echo"]);
        assert_eq!(lines[1].time_in_ms, 1_200);
        assert_eq!(lines[2].words.len(), 2);
        assert_eq!(lines[2].words[1].time_in_ms, 3_500);
    }

    #[test]
    fn strips_netease_and_kugou_credit_lines() {
        let lrc = "[00:00.000] 作词 : Steve Starkoff\n[00:01.000] 作曲 : Steve Starkoff\n[00:04.50]Lyrics by：Writer\n[00:10.00]Real lyric line\n[00:14.00]Second lyric line";
        let lines = parse_lrc(lrc);
        let texts: Vec<_> = lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(texts, ["", "Real lyric line", "Second lyric line"]);
    }

    #[test]
    fn cleans_titles_and_artists() {
        assert_eq!(
            clean_song_title("01. Song (Official Video) feat. Someone"),
            "Song"
        );
        assert_eq!(clean_song_title("Song - Remastered 2011"), "Song");
        assert_eq!(clean_artist("Artist - Topic"), "Artist");
        assert_eq!(clean_artist("ArtistVEVO"), "Artist");
    }

    #[test]
    fn matches_russian_collab_artists_and_artist_prefixed_titles() {
        let q1 = query("addiction", "LONOWN и Asenssia", 209.0);
        assert!(score_candidate("LONOWN, Asenssia", "addiction", 208.9, &q1) > 0.8);
        assert!(score_candidate("LONOWN", "addiction", 209.0, &q1) > 0.8);

        let q2 = query("Run Rabbit", "Mollie Elizabeth", 190.0);
        assert!(score_candidate("Mollie Elizabeth", "Mollie Elizabeth - Run Rabbit (Official Audio)", 190.0, &q2) > 0.9);
    }

    #[test]
    fn relaxed_fallback_recovers_video_duration_offset() {
        let q = query("ты похож на кота (remaster)", "zhanulka", 201.0);
        let items: Vec<LrcLibItem> = serde_json::from_str(
            r#"[{"trackName":"ты похож на кота","artistName":"zhanulka","duration":192.0,"syncedLyrics":"[00:05.00]Строчка 1\n[00:10.00]Строчка 2"}]"#,
        )
        .expect("items");
        assert_eq!(score_candidate("zhanulka", "ты похож на кота", 192.0, &q), -1.0);
        let best = best_candidate(items, &q).expect("relaxed fallback should match");
        assert!(best.synced_lyrics.is_some());
    }

    #[test]
    fn rejects_different_duration_or_artist() {
        let info = query("Song", "Artist", 180.0);
        assert!(score_candidate("Artist", "Song", 181.0, &info) > 0.0);
        assert_eq!(score_candidate("Artist", "Song", 190.0, &info), -1.0);
        assert_eq!(score_candidate("Somebody Else", "Song", 180.0, &info), -1.0);
    }

    #[test]
    fn similarity_handles_unicode() {
        assert_eq!(dice("夜に駆ける", "夜に駆ける"), 1.0);
        assert!(jaro_winkler("夜に駆ける", "夜に駆け") > 0.9);
    }

    #[test]
    fn picks_highest_scoring_candidate_and_prefers_synced() {
        let info = query("Song", "Artist", 0.0);
        let items: Vec<LrcLibItem> = serde_json::from_str(
            r#"[{"trackName":"Song","artistName":"Artist","plainLyrics":"x"},
                {"trackName":"Song","artistName":"Artist","syncedLyrics":"[00:01.00]Hi"},
                {"trackName":"Song","artistName":"Artist","instrumental":true}]"#,
        )
        .expect("items");
        let best = best_candidate(items, &info).expect("candidate");
        let result = into_result(best, &info, true);
        assert_eq!(result.artists, ["Artist"]);
        assert_eq!(result.lines.expect("lines")[1].text, "Hi");
    }

    #[test]
    fn parses_lyricsplus_word_and_line_payload() {
        let payload = serde_json::json!({
            "type": "Word",
            "selectedSongMetadata": {
                "title": "bad guy",
                "artist": "Billie Eilish",
                "duration": 194.0
            },
            "lyrics": [
                {
                    "time": 14157,
                    "duration": 805,
                    "text": "white shirt",
                    "syllabus": [
                        { "time": 14157, "duration": 352, "text": "white " },
                        { "time": 14515, "duration": 447, "text": "shirt" }
                    ]
                },
                {
                    "time": 15330,
                    "duration": 2358,
                    "text": "now red my bloody nose"
                }
            ]
        });
        let scored = parse_lyricsplus_json(&payload, &query("bad guy", "Billie Eilish", 194.0))
            .expect("parsed");
        let lines = scored.result.lines.expect("lines");
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[1].text, "white shirt");
        assert_eq!(lines[1].words.len(), 2);
        assert_eq!(lines[2].text, "now red my bloody nose");
    }

    #[test]
    fn parses_timed_ytm_lyrics() {
        let browse = serde_json::json!({
            "contents": { "elementRenderer": { "newElement": { "type": { "componentType": { "model": {
                "timedLyricsModel": { "lyricsData": { "timedLyricsData": [
                    { "lyricLine": "First  line", "cueRange": { "startTimeMilliseconds": "1000", "endTimeMilliseconds": "2500" } },
                    { "lyricLine": "\u{266a}", "cueRange": { "startTimeMilliseconds": "2500", "endTimeMilliseconds": "4000" } },
                    { "lyricLine": "Broken", "cueRange": { "startTimeMilliseconds": "9", "endTimeMilliseconds": "1" } }
                ] } }
            } } } } } }
        });
        let result = parse_ytm_browse(&browse, &query("Song", "Artist", 0.0)).expect("lyrics");
        let lines = result.lines.expect("lines");
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].duration, 1_000);
        assert_eq!(lines[1].text, "First line");
        assert_eq!(lines[1].duration, 1_500);
        assert_eq!(lines[2].text, "");
    }

    #[test]
    fn parses_plain_ytm_lyrics_and_skips_unavailable() {
        let plain = serde_json::json!({ "contents": { "sectionListRenderer": { "contents": [
            { "musicDescriptionShelfRenderer": { "description": { "runs": [{ "text": "Line one" }, { "text": "Line two" }] } } }
        ] } } });
        let result = parse_ytm_browse(&plain, &query("Song", "Artist", 0.0)).expect("plain");
        assert_eq!(result.lyrics.as_deref(), Some("Line one\nLine two"));

        let missing = serde_json::json!({ "contents": { "messageRenderer": { "text": { "runs": [{ "text": "Lyrics not available" }] } } } });
        assert!(parse_ytm_browse(&missing, &query("Song", "Artist", 0.0)).is_none());
    }

    #[test]
    fn browse_ids_are_validated() {
        assert!(is_valid_browse_id("MPLYt_abc-123"));
        assert!(!is_valid_browse_id("../etc"));
        assert!(!is_valid_browse_id(""));
    }

    #[test]
    fn retry_after_is_bounded() {
        assert_eq!(parse_retry_after("30"), 30);
        assert_eq!(parse_retry_after("999999"), MAX_RETRY_AFTER_SECONDS);
        assert_eq!(parse_retry_after("bogus"), DEFAULT_RETRY_AFTER_SECONDS);
    }

    #[test]
    fn unified_selector_upgrades_plain_ytm_to_synced_external() {
        let ytm_plain = LyricsResult {
            title: "addiction".to_string(),
            artists: vec!["LONOWN".to_string()],
            lines: None,
            lyrics: Some("Line 1\nLine 2".to_string()),
        };
        let netease_synced = ScoredLyrics {
            priority_bonus: 0.08,
            match_score: 1.35,
            result: LyricsResult {
                title: "addiction".to_string(),
                artists: vec!["LONOWN".to_string(), "Asenssia".to_string()],
                lines: Some(parse_lrc("[00:04.43]I'm shattered\n[00:07.51]In ruins shaped like you")),
                lyrics: None,
            },
        };
        let picked = select_best_unified(Some(ytm_plain.clone()), vec![netease_synced])
            .expect("should pick synced");
        assert!(has_usable_synced_lines(&picked));
        assert_eq!(picked.lines.expect("lines")[1].text, "I'm shattered");

        // When no synced lines exist anywhere, falls back to YTM plain
        let fallback = select_best_unified(Some(ytm_plain), vec![]).expect("should fallback plain");
        assert_eq!(fallback.lyrics.as_deref(), Some("Line 1\nLine 2"));
    }

    #[test]
    fn normalizes_target_language_and_cleans_translations() {
        assert_eq!(normalize_target_lang("ru-RU"), "ru");
        assert_eq!(normalize_target_lang("uk_UA"), "uk");
        assert_eq!(normalize_target_lang("zh-Hant-TW"), "zh-TW");
        assert_eq!(normalize_target_lang("invalid lang!"), "ru");

        assert_eq!(
            clean_translation_line("White shirt, now red", "Белая рубашка, теперь красная.").as_deref(),
            Some("Белая рубашка, теперь красная")
        );
        assert_eq!(
            clean_translation_line("My soul?", "Моя душа?").as_deref(),
            Some("Моя душа?")
        );
        assert_eq!(clean_translation_line("Hello!", "hello"), None);
        assert_eq!(clean_translation_line("Test", "  "), None);
    }

    #[test]
    fn parses_batch_translate_payload() {
        let json = serde_json::json!([
            [["Первая строка\n", "First line\n"], ["Вторая строка", "Second line"]],
            null,
            "en"
        ]);
        let (src, joined) = parse_translate_payload(&json);
        assert_eq!(src.as_deref(), Some("en"));
        assert_eq!(joined, "Первая строка\nВторая строка");
    }
}
