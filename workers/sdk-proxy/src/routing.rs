use vrc_udon_methods_worker_core::{
    GITHUB_ASSET_ACCEPT, GITHUB_JSON_ACCEPT, GithubRelease, MAX_RELEASE_PAGES,
    collect_world_sdk_versions, find_world_sdk_asset, github_releases_url, is_valid_version,
};
use worker::{Env, Method, Request, Response, Result};

use crate::config::runtime_config;
use crate::github_client::GithubClient;
use crate::responses::{
    ErrorBody, configuration_error_response, empty_response, json_response, set_cors_headers,
};

pub async fn fetch(req: Request, env: Env) -> Result<Response> {
    if req.method() == Method::Options {
        return empty_response(204, &env);
    }

    let config = match runtime_config(&env) {
        Ok(config) => config,
        Err(error) => return configuration_error_response(error, &env),
    };

    let url = req.url()?;
    let path = url.path();
    let client = GithubClient::new(env.clone(), config);

    let response = match (req.method(), path) {
        (Method::Get, "/versions") => json_response(&list_versions(&client).await?, 200, &env)?,
        (Method::Get, path) => {
            if let Some(version) = path
                .strip_prefix("/sdk/")
                .and_then(|path| path.strip_suffix(".zip"))
            {
                proxy_sdk_zip(version, &client, &env).await?
            } else {
                json_response(&ErrorBody::new("not found"), 404, &env)?
            }
        }
        _ => json_response(&ErrorBody::new("method not allowed"), 405, &env)?,
    };

    Ok(response)
}

async fn list_versions(
    client: &GithubClient,
) -> Result<Vec<vrc_udon_methods_worker_core::WorldSdkVersion>> {
    let releases = fetch_all_releases(client).await?;
    Ok(collect_world_sdk_versions(&releases))
}

async fn proxy_sdk_zip(version: &str, client: &GithubClient, env: &Env) -> Result<Response> {
    if !is_valid_version(version) {
        return json_response(&ErrorBody::new("invalid version"), 400, env);
    }

    let releases = fetch_all_releases(client).await?;
    let Some(asset) = find_world_sdk_asset(&releases, version) else {
        return json_response(
            &ErrorBody::new(format!("World SDK {version} was not found")),
            404,
            env,
        );
    };

    let download_url = asset.api_url.as_deref().unwrap_or(&asset.download_url);
    let github_stream = client
        .fetch_stream(download_url, GITHUB_ASSET_ACCEPT)
        .await?;
    let mut response = Response::from_stream(github_stream.stream)?.with_status(200);
    set_cors_headers(response.headers_mut(), env)?;
    response
        .headers_mut()
        .set("Content-Type", "application/zip")?;
    if let Some(etag) = github_stream.etag {
        response.headers_mut().set("ETag", &etag)?;
    }
    response.headers_mut().set(
        "Content-Disposition",
        &format!("attachment; filename=\"{}\"", asset.asset_name),
    )?;
    response
        .headers_mut()
        .set("Cache-Control", "public, max-age=86400")?;
    Ok(response)
}

async fn fetch_all_releases(client: &GithubClient) -> Result<Vec<GithubRelease>> {
    let mut releases = Vec::new();

    for page in 1..=MAX_RELEASE_PAGES {
        let body = client
            .fetch_text(&github_releases_url(page), GITHUB_JSON_ACCEPT, true)
            .await?;
        let page_releases = serde_json::from_str::<Vec<GithubRelease>>(&body)
            .map_err(|error| format!("GitHub releases JSON decode failed: {error}"))?;
        if page_releases.is_empty() {
            break;
        }
        releases.extend(page_releases);
    }

    Ok(releases)
}
