use semver::Version;
use serde::Serialize;

use crate::github::{GithubRelease, REPOSITORY};

pub const WORLD_ASSET_PREFIX: &str = "com.vrchat.worlds-";
pub const WORLD_ASSET_SUFFIX: &str = ".zip";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorldSdkVersion {
    pub version: String,
    pub tag: String,
    pub asset_name: String,
    pub download_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_url: Option<String>,
}

#[must_use]
pub fn github_releases_url(page: u32) -> String {
    format!("https://api.github.com/repos/{REPOSITORY}/releases?per_page=100&page={page}")
}

#[must_use]
pub fn version_from_asset_name(name: &str) -> Option<&str> {
    name.strip_prefix(WORLD_ASSET_PREFIX)
        .and_then(|rest| rest.strip_suffix(WORLD_ASSET_SUFFIX))
        .filter(|version| is_valid_version(version))
}

#[must_use]
pub fn is_valid_version(version: &str) -> bool {
    !version.is_empty()
        && version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'+' | b'-'))
}

#[must_use]
pub fn collect_world_sdk_versions(releases: &[GithubRelease]) -> Vec<WorldSdkVersion> {
    let mut versions = releases
        .iter()
        .flat_map(|release| {
            release.assets.iter().filter_map(|asset| {
                let version = version_from_asset_name(&asset.name)?;
                Some(WorldSdkVersion {
                    version: version.to_owned(),
                    tag: release.tag_name.clone(),
                    asset_name: asset.name.clone(),
                    download_url: asset.browser_download_url.clone(),
                    api_url: asset.url.clone(),
                })
            })
        })
        .collect::<Vec<_>>();

    sort_world_sdk_versions_desc(&mut versions);
    versions
}

#[must_use]
pub fn find_world_sdk_asset(releases: &[GithubRelease], version: &str) -> Option<WorldSdkVersion> {
    if !is_valid_version(version) {
        return None;
    }

    releases.iter().find_map(|release| {
        release.assets.iter().find_map(|asset| {
            (version_from_asset_name(&asset.name) == Some(version)).then(|| WorldSdkVersion {
                version: version.to_owned(),
                tag: release.tag_name.clone(),
                asset_name: asset.name.clone(),
                download_url: asset.browser_download_url.clone(),
                api_url: asset.url.clone(),
            })
        })
    })
}

pub fn sort_world_sdk_versions_desc(versions: &mut [WorldSdkVersion]) {
    versions.sort_by(|left, right| {
        let left_version = Version::parse(&left.version);
        let right_version = Version::parse(&right.version);

        match (left_version, right_version) {
            (Ok(left_version), Ok(right_version)) => right_version.cmp(&left_version),
            (Ok(_), Err(_)) => std::cmp::Ordering::Less,
            (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
            (Err(_), Err(_)) => right.version.cmp(&left.version),
        }
    });
}

#[cfg(test)]
mod tests {
    use crate::{GithubAsset, GithubRelease};

    use super::{collect_world_sdk_versions, find_world_sdk_asset, version_from_asset_name};

    fn release(tag_name: &str, assets: &[(&str, &str)]) -> GithubRelease {
        GithubRelease {
            tag_name: tag_name.to_owned(),
            assets: assets
                .iter()
                .map(|(name, url)| GithubAsset {
                    name: (*name).to_owned(),
                    browser_download_url: (*url).to_owned(),
                    url: Some(format!("{url}.api")),
                })
                .collect(),
        }
    }

    #[test]
    fn extracts_version_from_world_sdk_asset_name() {
        assert_eq!(
            version_from_asset_name("com.vrchat.worlds-3.10.4.zip"),
            Some("3.10.4")
        );
        assert_eq!(
            version_from_asset_name("com.vrchat.worlds-3.9.0-beta.2.zip"),
            Some("3.9.0-beta.2")
        );
    }

    #[test]
    fn rejects_non_world_sdk_asset_names() {
        assert_eq!(
            version_from_asset_name("com.vrchat.avatars-3.10.4.zip"),
            None
        );
        assert_eq!(
            version_from_asset_name("com.vrchat.worlds-3.10.4.tgz"),
            None
        );
        assert_eq!(
            version_from_asset_name("com.vrchat.worlds-../3.10.4.zip"),
            None
        );
    }

    #[test]
    fn collects_only_world_sdk_versions_and_sorts_semver_desc() {
        let releases = vec![
            release(
                "release-3",
                &[
                    (
                        "com.vrchat.worlds-3.9.1.zip",
                        "https://example.test/3.9.1.zip",
                    ),
                    (
                        "com.vrchat.avatars-3.10.4.zip",
                        "https://example.test/avatar.zip",
                    ),
                ],
            ),
            release(
                "release-4",
                &[
                    (
                        "com.vrchat.worlds-3.10.4.zip",
                        "https://example.test/3.10.4.zip",
                    ),
                    (
                        "com.vrchat.worlds-3.10.4-beta.1.zip",
                        "https://example.test/3.10.4-beta.1.zip",
                    ),
                    (
                        "com.vrchat.worlds-3.10.0.zip",
                        "https://example.test/3.10.0.zip",
                    ),
                    (
                        "com.vrchat.worlds-3.0.0.zip",
                        "https://example.test/3.0.0.zip",
                    ),
                ],
            ),
        ];

        let versions = collect_world_sdk_versions(&releases)
            .into_iter()
            .map(|version| version.version)
            .collect::<Vec<_>>();

        assert_eq!(
            versions,
            vec!["3.10.4", "3.10.4-beta.1", "3.10.0", "3.9.1", "3.0.0"]
        );
    }

    #[test]
    fn finds_asset_for_requested_version() {
        let releases = vec![release(
            "release-4",
            &[(
                "com.vrchat.worlds-3.10.4.zip",
                "https://example.test/3.10.4.zip",
            )],
        )];

        let asset = find_world_sdk_asset(&releases, "3.10.4").unwrap();

        assert_eq!(asset.version, "3.10.4");
        assert_eq!(asset.tag, "release-4");
        assert_eq!(asset.asset_name, "com.vrchat.worlds-3.10.4.zip");
        assert_eq!(asset.download_url, "https://example.test/3.10.4.zip");
        assert_eq!(
            asset.api_url,
            Some("https://example.test/3.10.4.zip.api".to_owned())
        );
        assert_eq!(find_world_sdk_asset(&releases, "../3.10.4"), None);
        assert_eq!(find_world_sdk_asset(&releases, "3.10.3"), None);
    }
}
