use faststr::FastStr;
use rand::Rng;
use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};
use sonic_rs::{json, JsonValueTrait, Value};
use std::{
    net::{Ipv6Addr, SocketAddr},
    sync::LazyLock,
};
use volo_http::{
    server::{route::post, IntoResponse, Router},
    Address, Json, Server,
};

static ENDPOINT: &str = "https://api.deepl.com/jsonrpc?client=chrome-extension,1.28.0";
static DL_SESSION: LazyLock<FastStr> = LazyLock::new(|| {
    std::env::var("DL_SESSION")
        .unwrap_or_else(|_| "".to_string())
        .into()
});

#[derive(Deserialize, Serialize)]
struct Request {
    jsonrpc: FastStr,
    method: FastStr,
    params: Params,
    id: i64,
}

impl Request {
    fn new_translate_request(params: Params) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            method: "LMT_handle_jobs".into(),
            params,
            id: rand::thread_rng().gen_range(60000000..=80000000),
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Params {
    jobs: Vec<Job>,
    lang: Lang,
    priority: i32,
    common_job_params: CommonJobParams,
    timestamp: i64,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            jobs: vec![],
            lang: Lang::default(),
            priority: 1,
            common_job_params: CommonJobParams::default(),
            timestamp: chrono::Utc::now().timestamp(),
        }
    }
}

impl Params {
    fn new(text: &str, jobs: Vec<Job>) -> Self {
        Self {
            jobs,
            timestamp: get_timestamp(count_i_in_text(text)),
            ..Default::default()
        }
    }
}

#[derive(Default, Deserialize, Serialize)]
struct Job {
    kind: FastStr,
    sentences: Vec<Sentence>,
    raw_en_context_before: Vec<FastStr>,
    raw_en_context_after: Vec<FastStr>,
    preferred_num_beams: i32,
}

#[derive(Clone, Default, Deserialize, Serialize)]
struct Sentence {
    text: FastStr,
    id: Option<i32>,
    prefix: Option<FastStr>,
}

#[derive(Deserialize, Serialize)]
struct Lang {
    target_lang: FastStr,
    preference: Preference,
    source_lang_computed: FastStr,
}

impl Default for Lang {
    fn default() -> Self {
        Self {
            target_lang: "ZH".into(),
            source_lang_computed: "auto".into(),
            preference: Preference::default(),
        }
    }
}

#[derive(Deserialize, Serialize)]
struct Preference {
    weight: Value,
    default: FastStr,
}

impl Default for Preference {
    fn default() -> Self {
        Self {
            weight: json!({}),
            default: "default".into(),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommonJobParams {
    quality: FastStr,
    regional_variant: FastStr,
    mode: FastStr,
    browser_type: i32,
    text_type: FastStr,
    advanced_mode: bool,
}

impl Default for CommonJobParams {
    fn default() -> Self {
        Self {
            quality: "normal".into(),
            regional_variant: "zh-Hans".into(),
            mode: "translate".into(),
            browser_type: 1,
            text_type: "richtext".into(),
            advanced_mode: false,
        }
    }
}

#[derive(Deserialize)]
struct LMTResponse {
    translations: Vec<Translation>,
    target_lang: FastStr,
    source_lang: FastStr,
}

#[derive(Deserialize)]
struct Translation {
    beams: Vec<Beam>,
}

#[derive(Deserialize)]
struct Beam {
    sentences: Vec<Sentence>,
}

#[derive(Deserialize)]
struct DeeplResponse {
    id: i64,
    result: Value,
}

#[derive(Deserialize)]
struct SplitTextResponse {
    lang: Value,
    texts: Vec<Text>,
}

#[derive(Deserialize)]
struct Text {
    chunks: Vec<Chunk>,
}

#[derive(Clone, Deserialize)]
struct Chunk {
    sentences: Vec<Sentence>,
}

#[derive(Debug, Serialize)]
struct TranslateResponse {
    alternatives: Vec<FastStr>,
    code: i32,
    data: FastStr,
    id: i64,
    method: FastStr,
    source_lang: FastStr,
    target_lang: FastStr,
}

#[derive(Debug, Deserialize)]
struct TranslateRequest {
    // #[serde(deserialize_with = "deserialize_text_field")]
    text: FastStr,
    source_lang: Option<FastStr>,
    target_lang: Option<FastStr>,
}

// #[derive(Debug)]
// enum TextInput {
//     Single(FastStr),
//     Multiple(Vec<FastStr>),
// }

// fn deserialize_text_field<'de, D>(deserializer: D) -> Result<TextInput, D::Error>
// where
//     D: serde::Deserializer<'de>,
// {
//     let value = Value::deserialize(deserializer)?;
//     match value {
//         Value::String(s) => Ok(TextInput::Single(s.into())),
//         Value::Array(arr) => {
//             let texts = arr
//                 .into_iter()
//                 .map(|v| match v {
//                     Value::String(s) => Ok(s.into()),
//                     _ => Err(serde::de::Error::custom("Array elements must be strings")),
//                 })
//                 .collect::<Result<Vec<FastStr>, _>>()?;
//             Ok(TextInput::Multiple(texts))
//         }
//         _ => Err(serde::de::Error::custom(
//             "Text must be either a string or an array of strings",
//         )),
//     }
// }

fn count_i_in_text(text: &str) -> usize {
    text.chars().filter(|c| *c == 'i').count()
}

fn get_timestamp(i_count: usize) -> i64 {
    let ts = chrono::Utc::now().timestamp_millis();
    if i_count != 0 {
        ts - ts % i_count as i64 + i_count as i64
    } else {
        ts
    }
}

fn is_rich_text(text: &str) -> bool {
    text.chars()
        .any(|c| c == '<' && text[c.len_utf8()..].contains('>'))
}

async fn request(request: Value) -> anyhow::Result<String> {
    let client = reqwest::Client::new();
    let mut headers = HeaderMap::new();
    headers.insert("accept", "*/*".parse()?);
    headers.insert("accept-language", "zh-CN,zh;q=0.9".parse()?);
    headers.insert("cache-control", "no-cache".parse()?);
    headers.insert("content-type", "application/json".parse()?);
    headers.insert(
        "cookie",
        format!("dl_session={}; ", DL_SESSION.as_str()).parse()?,
    );
    headers.insert("dnt", "1".parse()?);
    headers.insert("origin", "https://www.deepl.com".parse()?);
    headers.insert("pragma", "no-cache".parse()?);
    headers.insert("priority", "u=1, i".parse()?);
    headers.insert("referer", "https://www.deepl.com/".parse()?);
    headers.insert("user-agent", "DeepLBrowserExtension/1.28.0 Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/127.0.0.0 Safari/537.36".parse()?);
    let body = request.to_string();
    tracing::info!("body: {}", body);
    let response = client
        .post(ENDPOINT)
        .body(body)
        .headers(headers)
        .send()
        .await?;
    let text = response.text().await?;
    tracing::info!("response: {}", text);
    Ok(text)
}

async fn split_text(req: &TranslateRequest) -> anyhow::Result<SplitTextResponse> {
    let req = json!(
        {
            "jsonrpc": "2.0",
            "method": "LMT_split_text",
            "params": {
                "texts": [
                    req.text
                ],
                "commonJobParams": {
                    "mode": "translate",
                    "textType": "plaintext"
                },
                "lang": {
                    "lang_user_selected": "auto"
                }
            },
            "id": rand::thread_rng().gen_range(60000000..=80000000),
        }
    );
    let res_str = request(req).await?;
    let deepl_res: DeeplResponse = sonic_rs::from_str(&res_str)?;
    let split_text_res: SplitTextResponse = sonic_rs::from_str(&deepl_res.result.to_string())?;
    Ok(split_text_res)
}

async fn translate_text(req: &TranslateRequest) -> anyhow::Result<TranslateResponse> {
    let split_text_res = split_text(req).await?;
    let chunks = split_text_res.texts.first().unwrap().chunks.clone();
    let jobs = chunks
        .iter()
        .enumerate()
        .map(|(i, chunk)| {
            let sentence = chunk.sentences.first().unwrap();
            let mut context_before = vec![];
            let mut context_after = vec![];
            if i > 0_usize {
                context_before.push(chunks[i - 1].sentences.last().unwrap().text.clone());
            }
            if i < chunks.len() - 1 {
                context_after.push(chunks[i + 1].sentences.first().unwrap().text.clone());
            }
            Job {
                kind: "default".into(),
                sentences: vec![Sentence {
                    text: sentence.text.clone(),
                    id: Some(i as i32 + 1),
                    prefix: sentence.prefix.clone(),
                }],
                raw_en_context_before: context_before,
                raw_en_context_after: context_after,
                preferred_num_beams: 4,
            }
        })
        .collect();

    let mut translate_req = Request::new_translate_request(Params::new(&req.text, jobs));

    if translate_req.params.lang.source_lang_computed == "auto" {
        translate_req.params.lang.source_lang_computed = match split_text_res.lang.get("detected") {
            Some(value) => FastStr::new(value.as_str().unwrap_or("auto")),
            None => match &req.source_lang {
                Some(lang) => lang.clone(),
                None => "auto".into(),
            },
        };
    }
    if let Some(lang) = &req.target_lang {
        translate_req.params.lang.target_lang = lang.clone();
    }
    translate_req.params.common_job_params.advanced_mode = true;
    translate_req.params.common_job_params.text_type = if is_rich_text(&req.text) {
        "richtext".into()
    } else {
        "plaintext".into()
    };
    let res_str = request(json!(translate_req)).await?;
    let deepl_res: DeeplResponse = sonic_rs::from_str(&res_str)?;
    let lmt_res: LMTResponse = sonic_rs::from_str(&deepl_res.result.to_string())?;

    let mut alternatives: Vec<FastStr> = vec![];
    let num_beams = lmt_res.translations.first().unwrap().beams.len();
    for i in 1..num_beams {
        let mut alternative_str = String::new();
        for translation in &lmt_res.translations {
            if i < translation.beams.len() {
                alternative_str.push_str(
                    translation
                        .beams
                        .get(i)
                        .unwrap()
                        .sentences
                        .first()
                        .unwrap()
                        .text
                        .as_str(),
                );
            }
        }
        alternatives.push(alternative_str.into());
    }
    let data = lmt_res
        .translations
        .iter()
        .map(|translation| {
            translation
                .beams
                .first()
                .unwrap()
                .sentences
                .first()
                .unwrap()
                .text
                .as_str()
        })
        .collect::<Vec<&str>>()
        .join(" ");
    Ok(TranslateResponse {
        alternatives,
        code: 200,
        data: data.into(),
        id: deepl_res.id,
        method: "free".into(),
        source_lang: lmt_res.source_lang,
        target_lang: lmt_res.target_lang,
    })
}

async fn handler(Json(payload): Json<TranslateRequest>) -> impl IntoResponse {
    let start_time = std::time::Instant::now();
    let translate_res = translate_text(&payload).await.unwrap();
    let elapsed = start_time.elapsed();
    tracing::info!("costs {:.2?}", elapsed);
    Json(translate_res).into_response()
}
#[volo::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_file(true)
        .with_line_number(true)
        .with_target(false)
        .init();
    let app = Router::new().route("/translate", post(handler));
    let port = std::env::var("PRIMARY_PORT")
        .unwrap_or_else(|_| "59000".to_string())
        .parse::<u16>()
        .unwrap();
    let addr = SocketAddr::new(Ipv6Addr::UNSPECIFIED.into(), port);
    tracing::info!("listening on {}", addr);

    let addr = Address::from(addr);
    let server = Server::new(app);
    server.run(addr).await.unwrap();
}
