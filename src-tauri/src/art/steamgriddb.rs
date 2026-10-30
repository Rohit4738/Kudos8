//! SteamGridDB API v2 client (Session M).
//!
//! Endpoints used (all authenticated with `Authorization: Bearer <key>`):
//! - `GET /api/v2/search/autocomplete/<term>` — find the SteamGridDB game id
//! - `GET /api/v2/grids/game/<id>` — 600x900 portrait grids
//! - `GET /api/v2/heroes/game/<id>` — 1920x620 heroes
//! - `GET /api/v2/logos/game/<id>` and `/icons/game/<id>`
//!
//! The client never logs the key. Results are cached per process so switching
//! tabs in the art picker does not spend the request budget twice.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use serde::Deserialize;

use crate::art::{get_with_retry, ArtKind, ArtResult, RequestBudget};
use crate::secrets::{self, PROVIDER_STEAMGRIDDB};

const API_HOST: &str = "api.steamgriddb.com";
const API_ROOT: &str = "/api/v2";
const MAX_JSON_BYTES: usize = 2 * 1024 * 1024;
const MAX_CACHE_ENTRIES: usize = 200;

static GAME_CACHE: LazyLock<Mutex<HashMap<String, SgdbGame>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static ART_CACHE: LazyLock<Mutex<HashMap<String, Vec<ArtResult>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct SgdbGame {
    pub id: u64,
    pub name: String,
}

#[derive(Debug, Deserialize)]
struct Envelope<T> {
    #[serde(default)]
    success: bool,
    #[serde(default)]
    data: Option<T>,
    #[serde(default)]
    errors: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct SgdbAuthor {
    #[serde(default)]
    name: String,
}

#[derive(Debug, Deserialize)]
struct SgdbAsset {
    id: u64,
    url: String,
    #[serde(default)]
    thumb: String,
    #[serde(default)]
    width: u32,
    #[serde(default)]
    height: u32,
    #[serde(default)]
    author: Option<SgdbAuthor>,
}

impl SgdbAsset {
    fn into_result(self, kind: ArtKind) -> ArtResult {
        let thumb_url = if self.thumb.trim().is_empty() {
            self.url.clone()
        } else {
            self.thumb
        };
        ArtResult {
            id: self.id.to_string(),
            kind,
            url: self.url,
            thumb_url,
            width: self.width,
            height: self.height,
            author: self
                .author
                .map(|author| author.name.trim().to_string())
                .filter(|name| !name.is_empty()),
        }
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn search(title: &str, kind: ArtKind, budget: &RequestBudget) -> Result<Vec<ArtResult>, String> {
    let key = secrets::get(PROVIDER_STEAMGRIDDB).ok_or_else(|| {
        "No SteamGridDB API key is saved. Add one in Settings → Artwork.".to_string()
    })?;

    let game = find_game(&key, title, budget)?;
    let cache_key = format!("{}:{}", game.id, kind.as_str());
    if let Some(cached) = ART_CACHE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(&cache_key)
        .cloned()
    {
        return Ok(cached);
    }

    let path = format!(
        "{API_ROOT}/{}/game/{}?{}",
        kind_segment(kind),
        game.id,
        kind_query(kind)
    );
    let body = api_get(&key, &path, budget)?;
    let assets = parse_assets(&body, kind)?;

    let mut cache = ART_CACHE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if cache.len() >= MAX_CACHE_ENTRIES {
        cache.clear();
    }
    cache.insert(cache_key, assets.clone());
    Ok(assets)
}

fn find_game(key: &str, title: &str, budget: &RequestBudget) -> Result<SgdbGame, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("This game has no title to search for.".to_string());
    }
    let cache_key = title.to_ascii_lowercase();
    if let Some(cached) = GAME_CACHE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(&cache_key)
        .cloned()
    {
        return Ok(cached);
    }

    let path = format!(
        "{API_ROOT}/search/autocomplete/{}",
        percent_encode(title)
    );
    let body = api_get(key, &path, budget)?;
    let games = parse_search(&body)?;
    let game = pick_game(title, games).ok_or_else(|| {
        format!("SteamGridDB has no game matching \"{title}\". Try a shorter title.")
    })?;

    let mut cache = GAME_CACHE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if cache.len() >= MAX_CACHE_ENTRIES {
        cache.clear();
    }
    cache.insert(cache_key, game.clone());
    Ok(game)
}

fn api_get(key: &str, path: &str, budget: &RequestBudget) -> Result<Vec<u8>, String> {
    let headers = format!("Authorization: Bearer {key}\r\nAccept: application/json\r\n");
    let response = get_with_retry(API_HOST, path, Some(&headers), MAX_JSON_BYTES, budget)
        .map_err(|err| format!("SteamGridDB request failed: {err}"))?;
    let content_type = response
        .content_type
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !content_type.contains("application/json") && !content_type.contains("text/json") {
        return Err(format!(
            "SteamGridDB returned \"{}\" instead of JSON.",
            if content_type.is_empty() {
                "no content type"
            } else {
                &content_type
            }
        ));
    }
    Ok(response.body)
}

// ---------------------------------------------------------------------------
// Endpoint helpers
// ---------------------------------------------------------------------------

fn kind_segment(kind: ArtKind) -> &'static str {
    match kind {
        ArtKind::Grid => "grids",
        ArtKind::Hero => "heroes",
        ArtKind::Logo => "logos",
        ArtKind::Icon => "icons",
    }
}

fn kind_query(kind: ArtKind) -> &'static str {
    match kind {
        ArtKind::Grid => {
            "dimensions=600x900&types=static&nsfw=false&humor=false&epilepsy=false"
        }
        ArtKind::Hero => {
            "dimensions=1920x620&types=static&nsfw=false&humor=false&epilepsy=false"
        }
        ArtKind::Logo | ArtKind::Icon => "types=static&nsfw=false&humor=false&epilepsy=false",
    }
}

/// Percent-encode a UTF-8 search term. Keeps RFC 3986 unreserved characters and
/// encodes everything else, including spaces, quotes, `&`, `:` and non-ASCII.
fn percent_encode(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(value.len() * 3);
    for byte in value.as_bytes() {
        let unreserved = byte.is_ascii_alphanumeric()
            || matches!(*byte, b'-' | b'.' | b'_' | b'~');
        if unreserved {
            out.push(*byte as char);
        } else {
            out.push('%');
            out.push(HEX[(byte >> 4) as usize] as char);
            out.push(HEX[(byte & 0x0f) as usize] as char);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Response parsing (unit-tested with committed fixtures)
// ---------------------------------------------------------------------------

fn parse_search(body: &[u8]) -> Result<Vec<SgdbGame>, String> {
    let envelope: Envelope<Vec<SgdbGame>> = serde_json::from_slice(body)
        .map_err(|err| format!("SteamGridDB returned an unexpected search response: {err}"))?;
    if !envelope.success {
        return Err(envelope_error(envelope.errors));
    }
    Ok(envelope.data.unwrap_or_default())
}

fn parse_assets(body: &[u8], kind: ArtKind) -> Result<Vec<ArtResult>, String> {
    let envelope: Envelope<Vec<SgdbAsset>> = serde_json::from_slice(body)
        .map_err(|err| format!("SteamGridDB returned an unexpected art response: {err}"))?;
    if !envelope.success {
        return Err(envelope_error(envelope.errors));
    }
    Ok(envelope
        .data
        .unwrap_or_default()
        .into_iter()
        .filter(|asset| !asset.url.trim().is_empty())
        .map(|asset| asset.into_result(kind))
        .collect())
}

fn envelope_error(errors: Option<Vec<String>>) -> String {
    let message = errors
        .unwrap_or_default()
        .into_iter()
        .map(|error| error.trim().to_string())
        .filter(|error| !error.is_empty())
        .collect::<Vec<_>>()
        .join("; ");
    if message.is_empty() {
        "SteamGridDB rejected the request.".to_string()
    } else {
        format!("SteamGridDB: {message}")
    }
}

fn pick_game(title: &str, games: Vec<SgdbGame>) -> Option<SgdbGame> {
    let wanted = title.trim().to_ascii_lowercase();
    if let Some(exact) = games
        .iter()
        .find(|game| game.name.trim().to_ascii_lowercase() == wanted)
    {
        return Some(exact.clone());
    }
    games.into_iter().next()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("art")
            .join(name);
        std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("fixture {}: {err}", path.display()))
    }

    #[test]
    fn parses_autocomplete_and_prefers_an_exact_title() {
        let games = parse_search(fixture("steamgriddb-search.json").as_bytes()).unwrap();
        assert_eq!(games.len(), 3);
        assert_eq!(games[0].id, 4106);
        assert_eq!(games[0].name, "The Witcher 3: Wild Hunt - Blood and Wine");

        let picked = pick_game("the witcher 3: wild hunt", games.clone()).unwrap();
        assert_eq!(picked.id, 4105);
        assert_eq!(picked.name, "The Witcher 3: Wild Hunt");

        let fallback = pick_game("Witcher", games).unwrap();
        assert_eq!(fallback.id, 4106);
    }

    #[test]
    fn parses_grid_assets_into_the_art_contract() {
        let assets = parse_assets(
            fixture("steamgriddb-grids.json").as_bytes(),
            ArtKind::Grid,
        )
        .unwrap();
        assert_eq!(assets.len(), 2);
        assert_eq!(assets[0].id, "9001");
        assert_eq!(assets[0].kind, ArtKind::Grid);
        assert_eq!(assets[0].width, 600);
        assert_eq!(assets[0].height, 900);
        assert_eq!(assets[0].author.as_deref(), Some("Necro"));
        assert_eq!(
            assets[0].thumb_url,
            "https://cdn2.steamgriddb.com/thumb/9001.jpg"
        );

        let json = serde_json::to_string(&assets[0]).unwrap();
        assert!(json.contains("\"thumbUrl\""));
        assert!(json.contains("\"kind\":\"grid\""));
        assert!(!json.contains("thumb_url"));
    }

    #[test]
    fn missing_author_and_thumb_fall_back_cleanly() {
        let body = br#"{"success":true,"data":[{"id":7,"url":"https://cdn2.steamgriddb.com/a.png","width":512,"height":512}]}"#;
        let assets = parse_assets(body, ArtKind::Icon).unwrap();
        assert_eq!(assets[0].thumb_url, assets[0].url);
        assert_eq!(assets[0].author, None);
    }

    #[test]
    fn error_envelope_becomes_a_readable_message() {
        let error = parse_assets(
            fixture("steamgriddb-error.json").as_bytes(),
            ArtKind::Hero,
        )
        .unwrap_err();
        assert!(error.contains("SteamGridDB"));
        assert!(error.contains("Invalid API key"));
    }

    #[test]
    fn empty_data_is_not_an_error() {
        let body = br#"{"success":true,"data":[]}"#;
        assert!(parse_assets(body, ArtKind::Logo).unwrap().is_empty());
    }

    #[test]
    fn percent_encoding_handles_spaces_apostrophes_and_unicode() {
        assert_eq!(percent_encode("Portal 2"), "Portal%202");
        assert_eq!(
            percent_encode("Assassin's Creed: Origins"),
            "Assassin%27s%20Creed%3A%20Origins"
        );
        assert_eq!(percent_encode("Pokémon"), "Pok%C3%A9mon");
        assert_eq!(percent_encode("a-b_c.d~e"), "a-b_c.d~e");
    }

    #[test]
    fn endpoint_queries_use_the_documented_dimensions() {
        assert!(kind_query(ArtKind::Grid).contains("dimensions=600x900"));
        assert!(kind_query(ArtKind::Hero).contains("dimensions=1920x620"));
        assert!(kind_query(ArtKind::Logo).contains("types=static"));
        assert_eq!(kind_segment(ArtKind::Grid), "grids");
        assert_eq!(kind_segment(ArtKind::Hero), "heroes");
        assert_eq!(kind_segment(ArtKind::Logo), "logos");
        assert_eq!(kind_segment(ArtKind::Icon), "icons");
    }
}
