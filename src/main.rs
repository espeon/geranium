use std::convert::Infallible;
use std::env;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use foyer::{DirectFsDeviceOptions, Engine, HybridCache, HybridCacheBuilder};
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util;
use hyper_util::server::conn::auto::Builder as AutoBuilder;
use lazy_static::lazy_static;
use multihash_codetable::{Code, MultihashDigest};
use tokio::net::TcpListener;

mod err;
mod image;
mod resolve;
use image::process_image;
use resolve::{resolve_identity, ResolvedIdentity};

type Body = Full<Bytes>;
lazy_static! {
    static ref MAX_BLOB_SIZE: u64 = env::var("MAX_BLOB_SIZE")
        .map(|h| h.parse().unwrap_or(64 * 1024 * 1024))
        .unwrap_or(64 * 1024 * 1024);
}

#[derive(Clone)]
// An Executor that uses the tokio runtime.
pub struct TokioExecutor;

// Implement the `hyper::rt::Executor` trait for `TokioExecutor` so that it can be used to spawn
// tasks in the hyper runtime.
// An Executor allows us to manage execution of tasks which can help us improve the efficiency and
// scalability of the server.
impl<F> hyper::rt::Executor<F> for TokioExecutor
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    fn execute(&self, fut: F) {
        tokio::task::spawn(fut);
    }
}

async fn handle_request(
    req: hyper::Request<hyper::body::Incoming>,
    cache: Arc<HybridCache<String, Vec<u8>>>,
) -> anyhow::Result<Response<Body>> {
    let path = req.uri().path().to_string();

    if path == "/health" {
        return Ok(Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json")
            .header("Access-Control-Allow-Origin", "*")
            .body(Body::from(r#"{"status":"ok"}"#))
            .unwrap());
    }
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    dbg!(&segments);

    if segments.len() != 2 {
        return Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header("Access-Control-Allow-Origin", "*")
            .body(Body::from("Invalid path. Use /<DID>/<CID>"))
            .unwrap());
    }

    let (did, cid) = (segments[0].to_string(), segments[1].to_string());

    // if @ at end
    let (format, max_len, prefix) = if let Some((prefix, suffix)) = cid.split_once('@') {
        let mut parts = suffix.split(',');
        let format = parts.next().unwrap_or("jpeg");
        let max_len = parts.next().and_then(|s| s.parse::<u32>().ok());
        (format.to_string(), max_len, prefix.to_string())
    } else {
        ("jpeg".to_string(), None, cid)
    };
    let did_clone = did.clone();
    let cid_clone = prefix.clone();

    //dbg!(&format, &max_len);

    match cache
        .clone()
        .fetch(did + &cid_clone, || async move {
            // Resolve DID to PDS endpoint
            let did_cc = did_clone.clone();
            match cache
                .clone()
                .fetch(did_clone.clone(), || async move {
                    resolve_identity(&did_cc, "https://public.api.bsky.app")
                        .await
                        .map(|doc| {
                            // Serialize to bytes
                            serde_json::to_vec(&doc).unwrap_or_default()
                        })
                        .map_err(|e| anyhow!(e))
                })
                .await
            {
                // TODO: put this elsewhere
                Ok(did_doc_bytes) => {
                    let did_doc: ResolvedIdentity = match serde_json::from_slice(&did_doc_bytes) {
                        Ok(doc) => doc,
                        Err(_) => {
                            return Err(anyhow::anyhow!("Failed to deserialize DID doc"));
                        }
                    };
                    let blob = match fetch_blob(&did_doc.pds, &did_clone, &cid_clone).await {
                        Ok(data) => data,
                        Err(e) => return Err(anyhow::anyhow!("Blob fetch failed: {}", e)),
                    };
                    if let Ok(blob_ref) = Ok::<_, anyhow::Error>(&blob) {
                        let hash = Code::Sha2_256.digest(blob_ref);
                        let og_cid = cid::Cid::from_str(&cid_clone)?;
                        let codec = cid::Cid::from_str(&cid_clone).unwrap().codec();
                        //dbg!(codec);
                        let c = cid::Cid::new_v1(codec, hash);
                        if og_cid != c {
                            return Err(anyhow::anyhow!("CIDs do not match, invalid cid!"));
                        }
                    };
                    Ok(blob)
                }
                Err(e) => Err(anyhow::anyhow!(e)),
            }
        })
        .await
    {
        Ok(hit) => {
            let (image, fmt) = match process_image(&hit, &format, max_len) {
                Ok(data) => data,
                Err(e) => return server_error(format!("Image processing failed: {}", e)),
            };
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "image/".to_owned() + &fmt)
                .header("Access-Control-Allow-Origin", "*")
                .body(Body::from(image))
                .unwrap())
        }
        Err(e) => return server_error(e.to_string()),
    }
}

async fn fetch_blob(
    pds_endpoint: &str,
    did: &str,
    cid: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    const MAX_BLOB_SIZE: u64 = 64 * 1024 * 1024; // 64MB
    const MAX_RETRIES: u32 = 3;

    let client = reqwest::ClientBuilder::new()
        .timeout(Duration::from_secs(15))
        .connect_timeout(Duration::from_secs(1))
        .build()?;

    let url = format!(
        "{}/xrpc/com.atproto.sync.getBlob?did={}&cid={}",
        pds_endpoint.trim_end_matches('/'),
        did,
        cid
    );

    let mut attempts = 0;
    while attempts < MAX_RETRIES {
        match client.get(&url).send().await {
            Ok(response) => {
                if !response.status().is_success() {
                    attempts += 1;
                    continue;
                }

                if let Some(len) = response.content_length() {
                    if len > MAX_BLOB_SIZE {
                        return Err("Blob too large".into());
                    }
                }

                let bytes = response.bytes().await?;
                if bytes.len() as u64 > MAX_BLOB_SIZE {
                    return Err("Blob too large".into());
                }

                return Ok(bytes.to_vec());
            }
            Err(e) => {
                attempts += 1;
                if attempts == MAX_RETRIES {
                    return Err(e.into());
                }
            }
        }
    }

    Err("Max retries exceeded".into())
}

fn server_error(message: String) -> anyhow::Result<Response<Body>> {
    Ok(Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header("Access-Control-Allow-Origin", "*")
        .body(Body::from(message))
        .unwrap())
}

async fn build_cache() -> HybridCache<String, Vec<u8>> {
    let cache_location = env::var("CACHE_DIR").unwrap_or("./data/foyer".into());
    let memory: usize = env::var("CACHE_SIZE")
        .map(|str| str.parse().unwrap_or(1024))
        .unwrap_or(1024);

    let mut hybrid = HybridCacheBuilder::new()
        .with_name("ger")
        .memory(memory)
        .storage(Engine::Small);

    if cache_location.is_empty() {
        hybrid = hybrid.with_device_options(DirectFsDeviceOptions::new(cache_location))
    }

    let cache: HybridCache<String, Vec<u8>> = hybrid.build().await.unwrap();

    cache
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut port = env::var("PORT").unwrap_or("0.0.0.0:3000".into());
    // if the user just put the port, or colon then the port
    if port.starts_with(':') || !port.contains(':') {
        // add the ip to start, and negate duplicate colons
        port = ("0.0.0.0:".to_string() + &port).replace("::", ":");
    }

    let listener = TcpListener::bind(port).await?;

    // init cache
    // please god i hope cache is arced :pelading:
    let cache = Arc::new(build_cache().await);

    loop {
        let (stream, _) = listener.accept().await?;
        let io = hyper_util::rt::tokio::TokioIo::new(stream);

        let cache = cache.clone();
        tokio::task::spawn(async move {
            if let Err(err) = AutoBuilder::new(TokioExecutor)
                .serve_connection(
                    io,
                    service_fn(move |req: Request<hyper::body::Incoming>| {
                        let cache = cache.clone();
                        async move {
                            Ok::<_, Infallible>(
                                handle_request(req, cache)
                                    .await
                                    .unwrap_or_else(|e| server_error(e.to_string()).unwrap()),
                            )
                        }
                    }),
                )
                .await
            {
                eprintln!("Error serving connection: {}", err);
            }
        });
    }
}
