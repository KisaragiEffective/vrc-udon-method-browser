use gloo_net::http::Request;
use semver::Version;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteSdk {
    pub id: String,
    pub label: String,
    pub version: String,
}

#[derive(Debug, Clone, Deserialize)]
struct GithubRelease {
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Clone, Deserialize)]
struct GithubAsset {
    name: String,
}

pub async fn load_remote_sdks() -> Result<Vec<RemoteSdk>, String> {
    let mut releases = Vec::new();
    for page in 1..=10 {
        let page_releases: Vec<GithubRelease> = Request::get(&format!(
            "https://api.github.com/repos/vrchat/packages/releases?per_page=100&page={page}"
        ))
        .send()
        .await
        .map_err(|error| error.to_string())?
        .json()
        .await
        .map_err(|error| error.to_string())?;

        if page_releases.is_empty() {
            break;
        }
        releases.extend(page_releases);
    }

    let mut release_sdks = Vec::new();
    for release in releases {
        for asset in release.assets {
            let Some(version) = world_sdk_version_from_asset_name(&asset.name) else {
                continue;
            };
            let Ok(parsed_version) = Version::parse(&version) else {
                continue;
            };

            release_sdks.push((
                parsed_version,
                RemoteSdk {
                    id: format!("release:{version}"),
                    label: version.clone(),
                    version,
                },
            ));
        }
    }

    release_sdks.sort_by(|(left, _), (right, _)| right.cmp(left));
    Ok(release_sdks.into_iter().map(|(_, sdk)| sdk).collect())
}

fn world_sdk_version_from_asset_name(name: &str) -> Option<String> {
    name.strip_prefix("com.vrchat.worlds-")
        .and_then(|version| version.strip_suffix(".zip"))
        .map(str::to_owned)
}

#[must_use]
pub fn sdk_proxy_base() -> &'static str {
    option_env!("VRC_UDON_METHODS_SDK_PROXY_BASE").unwrap_or("")
}
