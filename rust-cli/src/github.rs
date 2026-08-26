use std::thread;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use reqwest::blocking::{Client, RequestBuilder, Response};
use reqwest::header::{ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::Deserialize;
use serde_json::json;

use crate::crypto::device_hash;
use crate::model::{EncryptedLedger, PublicLedger};

const API: &str = "https://api.github.com";
const API_VERSION: &str = "2022-11-28";
pub const UPSTREAM_REPO: &str = "Atingaii/token-monitor";

pub struct GithubClient {
    http: Client,
    repo: String,
    token: String,
}

#[derive(Deserialize)]
struct ShaResponse { sha: String }
#[derive(Deserialize)]
struct GitObject { sha: String }
#[derive(Deserialize)]
struct RefResponse { object: GitObject }
#[derive(Deserialize)]
struct UserResponse { login: String }
#[derive(Deserialize)]
struct ParentResponse { full_name: String }
#[derive(Deserialize)]
struct RepositoryResponse {
    private: bool,
    full_name: String,
    #[serde(default)]
    fork: bool,
    parent: Option<ParentResponse>,
}

fn http_client() -> Result<Client> {
    Ok(Client::builder().timeout(Duration::from_secs(30)).build()?)
}

fn request(http: &Client, token: &str, method: reqwest::Method, url: String) -> RequestBuilder {
    http.request(method, url)
        .header(USER_AGENT, "token-monitor/1.0")
        .header(ACCEPT, "application/vnd.github+json")
        .header("X-GitHub-Api-Version", API_VERSION)
        .header(AUTHORIZATION, format!("Bearer {token}"))
}

fn checked(response: Response, action: &str) -> Result<Response> {
    if response.status().is_success() { return Ok(response); }
    let status = response.status();
    let body = response.text().unwrap_or_default();
    bail!("GitHub {action} failed ({status}): {}", body.chars().take(500).collect::<String>())
}

fn repo_is_upstream_fork(repository: &RepositoryResponse) -> bool {
    repository.full_name.eq_ignore_ascii_case(UPSTREAM_REPO)
        || (repository.fork
            && repository.parent.as_ref().is_some_and(|parent| {
                parent.full_name.eq_ignore_ascii_case(UPSTREAM_REPO)
            }))
}

/// Resolve the user's own fork of this project. The common path is completely
/// automatic: `<login>/token-monitor` is reused when it is already a fork; if it
/// does not exist, GitHub's fork API creates it. `--repo` remains available for
/// renamed forks or organization-owned forks.
pub fn ensure_user_fork(token: &str) -> Result<String> {
    let http = http_client()?;
    let user: UserResponse = checked(
        request(&http, token, reqwest::Method::GET, format!("{API}/user")).send()?,
        "account discovery",
    )?.json()?;
    let expected = format!("{}/token-monitor", user.login);
    let repo_url = format!("{API}/repos/{expected}");
    let existing = request(&http, token, reqwest::Method::GET, repo_url.clone()).send()?;
    if existing.status().is_success() {
        let repository: RepositoryResponse = existing.json()?;
        if !repo_is_upstream_fork(&repository) {
            bail!("{expected} already exists but is not a fork of {UPSTREAM_REPO}; rerun setup with --repo OWNER/RENAMED_FORK")
        }
        if repository.private {
            bail!("{expected} is private. The zero-server dashboard requires publicly readable aggregate snapshots")
        }
        return Ok(repository.full_name);
    }
    if existing.status() != reqwest::StatusCode::NOT_FOUND {
        checked(existing, "fork discovery")?;
    }

    let create = request(
        &http,
        token,
        reqwest::Method::POST,
        format!("{API}/repos/{UPSTREAM_REPO}/forks"),
    )
    .json(&json!({ "default_branch_only": true }))
    .send()?;
    if !create.status().is_success() && create.status() != reqwest::StatusCode::ACCEPTED {
        checked(create, "automatic fork creation")?;
    }

    for _ in 0..20 {
        let response = request(&http, token, reqwest::Method::GET, repo_url.clone()).send()?;
        if response.status().is_success() {
            let repository: RepositoryResponse = response.json()?;
            if repo_is_upstream_fork(&repository) && !repository.private {
                return Ok(repository.full_name);
            }
        }
        thread::sleep(Duration::from_millis(750));
    }
    bail!("GitHub accepted the fork request but the fork is not ready yet; rerun `token-monitor setup` once GitHub finishes creating it")
}

impl GithubClient {
    pub fn new(repo: String, token: String) -> Result<Self> {
        Ok(Self { http: http_client()?, repo, token })
    }

    fn request(&self, method: reqwest::Method, url: String) -> RequestBuilder {
        request(&self.http, &self.token, method, url)
    }

    pub fn validate(&self) -> Result<()> {
        let response = checked(
            self.request(reqwest::Method::GET, format!("{API}/repos/{}", self.repo)).send()?,
            "repository access",
        )?;
        let repository: RepositoryResponse = response.json()?;
        if repository.private {
            bail!("the fork must be public for the zero-server dashboard. Only de-identified aggregate snapshots are publicly readable; the compatibility ledger remains AES-GCM encrypted")
        }
        if !repo_is_upstream_fork(&repository) {
            bail!("{} is not a fork of {}; use the project fork itself as the ledger repository", repository.full_name, UPSTREAM_REPO)
        }
        Ok(())
    }

    pub fn snapshot_branch(device_id: &str) -> String {
        format!("tm-ledger-{}", device_hash(device_id))
    }

    /// Replace one device branch with a history-free root snapshot containing:
    /// - `ledger.json`: AES-256-GCM compatibility/private aggregate envelope.
    /// - `public.json`: de-identified aggregate-only payload for the zero-login
    ///   static dashboard. No hostname, raw device id, session id or content is
    ///   serialized into the public payload.
    pub fn replace_snapshot(
        &self,
        device_id: &str,
        envelope: &EncryptedLedger,
        public_ledger: &PublicLedger,
    ) -> Result<String> {
        let branch = Self::snapshot_branch(device_id);
        let private_serialized = serde_json::to_string(envelope)?;
        let public_serialized = serde_json::to_string(public_ledger)?;

        let private_blob: ShaResponse = checked(
            self.request(reqwest::Method::POST, format!("{API}/repos/{}/git/blobs", self.repo))
                .json(&json!({ "content": private_serialized, "encoding": "utf-8" }))
                .send()?,
            "encrypted blob creation",
        )?.json()?;
        let public_blob: ShaResponse = checked(
            self.request(reqwest::Method::POST, format!("{API}/repos/{}/git/blobs", self.repo))
                .json(&json!({ "content": public_serialized, "encoding": "utf-8" }))
                .send()?,
            "public aggregate blob creation",
        )?.json()?;
        let tree: ShaResponse = checked(
            self.request(reqwest::Method::POST, format!("{API}/repos/{}/git/trees", self.repo))
                .json(&json!({ "tree": [
                    { "path": "ledger.json", "mode": "100644", "type": "blob", "sha": private_blob.sha },
                    { "path": "public.json", "mode": "100644", "type": "blob", "sha": public_blob.sha }
                ] }))
                .send()?,
            "snapshot tree creation",
        )?.json()?;
        let commit: ShaResponse = checked(
            self.request(reqwest::Method::POST, format!("{API}/repos/{}/git/commits", self.repo))
                .json(&json!({ "message": format!("token-monitor snapshot {}", envelope.device_hash), "tree": tree.sha, "parents": [] }))
                .send()?,
            "snapshot commit creation",
        )?.json()?;

        let ref_url = format!("{API}/repos/{}/git/ref/heads/{branch}", self.repo);
        let ref_response = self.request(reqwest::Method::GET, ref_url).send()?;
        if ref_response.status() == reqwest::StatusCode::NOT_FOUND {
            checked(
                self.request(reqwest::Method::POST, format!("{API}/repos/{}/git/refs", self.repo))
                    .json(&json!({ "ref": format!("refs/heads/{branch}"), "sha": commit.sha }))
                    .send()?,
                "snapshot branch creation",
            )?;
        } else {
            let current: RefResponse = checked(ref_response, "snapshot branch read")?.json()?;
            if current.object.sha != commit.sha {
                checked(
                    self.request(reqwest::Method::PATCH, format!("{API}/repos/{}/git/refs/heads/{branch}", self.repo))
                        .json(&json!({ "sha": commit.sha, "force": true }))
                        .send()?,
                    "snapshot branch update",
                )?;
            }
        }
        Ok(branch)
    }

    pub fn remove_snapshot_branch(&self, device_id: &str) -> Result<()> {
        let branch = Self::snapshot_branch(device_id);
        let response = self.request(
            reqwest::Method::DELETE,
            format!("{API}/repos/{}/git/refs/heads/{branch}", self.repo),
        ).send().context("failed to contact GitHub")?;
        if response.status() == reqwest::StatusCode::NOT_FOUND { return Ok(()); }
        checked(response, "snapshot branch deletion")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifies_the_expected_upstream_fork() {
        let repository = RepositoryResponse {
            private: false,
            full_name: "alice/token-monitor".into(),
            fork: true,
            parent: Some(ParentResponse { full_name: UPSTREAM_REPO.into() }),
        };
        assert!(repo_is_upstream_fork(&repository));
    }
}
