use anyhow::{bail, Context, Result};
use reqwest::blocking::{Client, Response};
use reqwest::header::{ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::Deserialize;
use serde_json::json;

use crate::crypto::device_hash;
use crate::model::EncryptedLedger;

const API: &str = "https://api.github.com";
const API_VERSION: &str = "2022-11-28";

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

impl GithubClient {
    pub fn new(repo: String, token: String) -> Result<Self> {
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        Ok(Self { http, repo, token })
    }

    fn request(&self, method: reqwest::Method, url: String) -> reqwest::blocking::RequestBuilder {
        self.http
            .request(method, url)
            .header(USER_AGENT, "token-monitor/1.0")
            .header(ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", API_VERSION)
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
    }

    fn checked(response: Response, action: &str) -> Result<Response> {
        if response.status().is_success() { return Ok(response); }
        let status = response.status();
        let body = response.text().unwrap_or_default();
        bail!("GitHub {action} failed ({status}): {}", body.chars().take(500).collect::<String>())
    }

    pub fn validate(&self) -> Result<()> {
        let url = format!("{API}/repos/{}", self.repo);
        let response = self.request(reqwest::Method::GET, url).send()?;
        Self::checked(response, "repository access")?;
        Ok(())
    }

    pub fn snapshot_branch(device_id: &str) -> String {
        format!("tm-ledger-{}", device_hash(device_id))
    }

    pub fn replace_snapshot(&self, device_id: &str, envelope: &EncryptedLedger) -> Result<String> {
        let branch = Self::snapshot_branch(device_id);
        let serialized = serde_json::to_string(envelope)?;

        let blob: ShaResponse = Self::checked(
            self.request(reqwest::Method::POST, format!("{API}/repos/{}/git/blobs", self.repo))
                .json(&json!({ "content": serialized, "encoding": "utf-8" }))
                .send()?,
            "blob creation",
        )?.json()?;

        let tree: ShaResponse = Self::checked(
            self.request(reqwest::Method::POST, format!("{API}/repos/{}/git/trees", self.repo))
                .json(&json!({
                    "tree": [{
                        "path": "ledger.json",
                        "mode": "100644",
                        "type": "blob",
                        "sha": blob.sha
                    }]
                }))
                .send()?,
            "tree creation",
        )?.json()?;

        // Intentionally create a root commit every sync. The device ref is then
        // force-moved to it, so snapshots do not build an unbounded visible commit
        // chain. GitHub eventually garbage-collects the unreachable old snapshot.
        let commit: ShaResponse = Self::checked(
            self.request(reqwest::Method::POST, format!("{API}/repos/{}/git/commits", self.repo))
                .json(&json!({
                    "message": format!("token-monitor snapshot {}", envelope.device_hash),
                    "tree": tree.sha,
                    "parents": []
                }))
                .send()?,
            "snapshot commit creation",
        )?.json()?;

        let ref_url = format!("{API}/repos/{}/git/ref/heads/{branch}", self.repo);
        let ref_response = self.request(reqwest::Method::GET, ref_url).send()?;
        if ref_response.status() == reqwest::StatusCode::NOT_FOUND {
            Self::checked(
                self.request(reqwest::Method::POST, format!("{API}/repos/{}/git/refs", self.repo))
                    .json(&json!({ "ref": format!("refs/heads/{branch}"), "sha": commit.sha }))
                    .send()?,
                "snapshot branch creation",
            )?;
        } else {
            let _current: RefResponse = Self::checked(ref_response, "snapshot branch read")?.json()?;
            Self::checked(
                self.request(reqwest::Method::PATCH, format!("{API}/repos/{}/git/refs/heads/{branch}", self.repo))
                    .json(&json!({ "sha": commit.sha, "force": true }))
                    .send()?,
                "snapshot branch update",
            )?;
        }

        Ok(branch)
    }

    pub fn remove_snapshot_branch(&self, device_id: &str) -> Result<()> {
        let branch = Self::snapshot_branch(device_id);
        let response = self
            .request(reqwest::Method::DELETE, format!("{API}/repos/{}/git/refs/heads/{branch}", self.repo))
            .send()
            .context("failed to contact GitHub")?;
        if response.status() == reqwest::StatusCode::NOT_FOUND { return Ok(()); }
        Self::checked(response, "snapshot branch deletion")?;
        Ok(())
    }
}
