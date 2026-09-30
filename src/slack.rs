use crate::auth::{self, Credentials};
use anyhow::{bail, Context, Result};
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::{collections::HashSet, fs::File, path::Path, time::Duration};

pub struct Slack {
    client: Client,
    creds: Credentials,
    #[cfg(test)]
    endpoint: Option<String>,
}
const DEFAULT_WORKSPACE: &str = "Grail Computer";
pub fn form(params: &Value) -> Vec<(String, String)> {
    params
        .as_object()
        .into_iter()
        .flatten()
        .map(|(k, v)| {
            (
                k.clone(),
                v.as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| v.to_string()),
            )
        })
        .collect()
}
fn validate_download_url(url: &str) -> Result<()> {
    let parsed = reqwest::Url::parse(url).map_err(|_| anyhow::anyhow!("Invalid download URL"))?;
    if parsed.scheme() != "https"
        || parsed.host_str() != Some("files.slack.com")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port_or_known_default() != Some(443)
    {
        bail!("Unexpected download host");
    }
    Ok(())
}

fn save_download(
    mut response: reqwest::blocking::Response,
    output: &Path,
    expected: Option<u64>,
) -> Result<u64> {
    use std::io::Write;
    if !response.status().is_success() {
        bail!("Slack download HTTP {}", response.status());
    }
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    // Tempfiles have owner-only permissions. Persist only complete downloads,
    // without replacing existing files (including symlinks).
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    let bytes = std::io::copy(&mut response, &mut file)
        .map_err(|_| anyhow::anyhow!("Slack download body read failed"))?;
    if expected.is_some_and(|size| bytes != size) {
        bail!("Download size mismatch");
    }
    file.flush()?;
    file.as_file().sync_all()?;
    file.persist_noclobber(output)
        .map_err(|e| anyhow::anyhow!("Cannot save download: {}", e.error))?;
    Ok(bytes)
}

impl Slack {
    pub fn download(&self, file_id: &str, output: &Path) -> Result<Value> {
        if output.try_exists()? {
            bail!("Output already exists; choose a new path");
        }
        let info = self.api("files.info", json!({"file":file_id}))?;
        let url = info["file"]["url_private_download"]
            .as_str()
            .or_else(|| info["file"]["url_private"].as_str())
            .context("No download URL")?;
        validate_download_url(url)?;
        let response = self
            .client
            .get(url)
            .bearer_auth(&self.creds.token)
            .header("Cookie", format!("d={}", self.creds.cookie))
            .send()
            .map_err(|_| anyhow::anyhow!("Slack download network error"))?;
        let bytes = save_download(response, output, info["file"]["size"].as_u64())?;
        Ok(json!({"ok":true,"file":file_id,"bytes":bytes,"output":output}))
    }
    pub fn new(refresh: bool, workspace: Option<&str>) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let wanted = workspace.unwrap_or(DEFAULT_WORKSPACE);
        // The cache intentionally stores only credentials, so validate its
        // workspace before reusing it. This keeps Grail Computer as the
        // default even if an older invocation cached another workspace.
        if !refresh {
            if let Some(creds) = auth::cached() {
                let s = Self {
                    client: client.clone(),
                    creds,
                    #[cfg(test)]
                    endpoint: None,
                };
                let auth = s.raw("auth.test", &json!({}))?;
                if auth["ok"] == true
                    && (auth["team"]
                        .as_str()
                        .is_some_and(|name| name.eq_ignore_ascii_case(wanted))
                        || auth["team_id"]
                            .as_str()
                            .is_some_and(|id| id.eq_ignore_ascii_case(wanted)))
                {
                    return Ok(s);
                }
            }
        }
        let (cookie, tokens) = auth::fresh()?;
        let mut discovered = Vec::new();
        for token in tokens {
            let s = Self {
                client: client.clone(),
                #[cfg(test)]
                endpoint: None,
                creds: Credentials {
                    token,
                    cookie: cookie.clone(),
                },
            };
            let auth = s.raw("auth.test", &json!({}))?;
            if auth["ok"] == true {
                let team = auth["team"].as_str().unwrap_or("unknown");
                let team_id = auth["team_id"].as_str().unwrap_or("");
                discovered.push(format!("{team} ({team_id})"));
                if !auth["team"]
                    .as_str()
                    .is_some_and(|name| name.eq_ignore_ascii_case(wanted))
                    && !auth["team_id"]
                        .as_str()
                        .is_some_and(|id| id.eq_ignore_ascii_case(wanted))
                {
                    continue;
                }
                auth::save(&s.creds)?;
                return Ok(s);
            }
        }
        if discovered.is_empty() {
            bail!("No valid Slack desktop session found while selecting workspace {wanted}");
        }
        bail!(
            "Workspace {wanted:?} was not found. Available workspaces: {}",
            discovered.join(", ")
        )
    }
    fn raw(&self, method: &str, params: &Value) -> Result<Value> {
        if !method
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
        {
            bail!("Invalid Slack method");
        }
        let endpoint = "https://slack.com/api/";
        #[cfg(test)]
        let endpoint = self.endpoint.as_deref().unwrap_or(endpoint);
        let response = self
            .client
            .post(format!("{endpoint}{method}"))
            .bearer_auth(&self.creds.token)
            .header("Cookie", format!("d={}", self.creds.cookie))
            .form(&form(params))
            .send()
            .map_err(|_| anyhow::anyhow!("Slack network request failed"))?;
        if response.status().as_u16() == 429 {
            bail!(
                "Slack rate limit reached; retry after {} seconds",
                response
                    .headers()
                    .get("retry-after")
                    .and_then(|h| h.to_str().ok())
                    .unwrap_or("the requested interval")
            );
        }
        if !response.status().is_success() {
            bail!("Slack HTTP {}", response.status());
        }
        response.json().context("Slack returned invalid JSON")
    }
    pub fn api(&self, method: &str, params: Value) -> Result<Value> {
        let v = self.raw(method, &params)?;
        if v["ok"] != true {
            bail!(
                "Slack {}: {}",
                method,
                v["error"].as_str().unwrap_or("unknown error")
            );
        }
        Ok(v)
    }
    pub fn list(&self, method: &str, mut params: Value, key: &str, max: usize) -> Result<Value> {
        let mut all = Vec::new();
        let mut seen = HashSet::new();
        let mut cursor = String::new();
        loop {
            params["limit"] = json!((max - all.len()).min(200));
            if !cursor.is_empty() {
                params["cursor"] = json!(cursor);
            }
            let page = self.api(method, params.clone())?;
            all.extend(page[key].as_array().cloned().unwrap_or_default());
            cursor = page["response_metadata"]["next_cursor"]
                .as_str()
                .unwrap_or("")
                .to_string();
            if all.len() >= max || cursor.is_empty() {
                all.truncate(max);
                return Ok(
                    json!({"ok":true,key:all,"has_more":!cursor.is_empty()||page["has_more"]==true,"next_cursor":cursor}),
                );
            }
            if !seen.insert(cursor.clone()) {
                bail!("Slack repeated a pagination cursor");
            }
        }
    }
    pub fn resolve(&self, name: &str, write: bool) -> Result<String> {
        let is_id = |s: &str| {
            s.len() > 8
                && s.chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        };
        if matches!(name.as_bytes().first(), Some(b'C' | b'D' | b'G')) && is_id(name) {
            return Ok(name.into());
        }
        let channels = self.list(
            "conversations.list",
            json!({"types":"public_channel,private_channel,mpim,im","exclude_archived":true}),
            "channels",
            100_000,
        )?;
        if !name.starts_with('@') && !name.starts_with('U') {
            let matches: Vec<_> = channels["channels"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|c| c["name"].as_str() == Some(name.trim_start_matches('#')))
                .collect();
            if matches.len() == 1 {
                return Ok(matches[0]["id"]
                    .as_str()
                    .context("Missing channel ID")?
                    .into());
            }
        }
        let user = if name.starts_with('U') && is_id(name) {
            name.to_string()
        } else {
            let users = self.list("users.list", json!({}), "members", 100_000)?;
            let query = name.trim_start_matches('@');
            let matches: Vec<_> = users["members"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|u| {
                    [
                        u["name"].as_str(),
                        u["real_name"].as_str(),
                        u["profile"]["display_name"].as_str(),
                    ]
                    .into_iter()
                    .flatten()
                    .any(|s| s.eq_ignore_ascii_case(query))
                })
                .collect();
            if matches.len() != 1 {
                bail!("No unique channel/user match for {name}; use an exact ID");
            }
            matches[0]["id"]
                .as_str()
                .context("Missing user ID")?
                .to_string()
        };
        if let Some(c) = channels["channels"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["user"].as_str() == Some(&user))
        {
            return Ok(c["id"].as_str().context("Missing DM ID")?.into());
        }
        if !write {
            bail!("No existing DM found; a read command will not open a new DM");
        }
        Ok(
            self.api("conversations.open", json!({"users":user}))?["channel"]["id"]
                .as_str()
                .context("Missing DM ID")?
                .into(),
        )
    }
    pub fn upload(
        &self,
        channel: &str,
        path: &Path,
        caption: Option<String>,
        thread: Option<String>,
    ) -> Result<Value> {
        let file = File::open(path).context("Cannot open upload file")?;
        let len = file.metadata()?.len();
        if len == 0 {
            bail!("Cannot upload an empty file");
        }
        let filename = path
            .file_name()
            .context("Missing filename")?
            .to_string_lossy();
        let ticket = self.api(
            "files.getUploadURLExternal",
            json!({"filename":filename,"length":len}),
        )?;
        let url = ticket["upload_url"]
            .as_str()
            .context("Missing upload URL")?;
        let parsed = reqwest::Url::parse(url)?;
        if parsed.scheme() != "https"
            || !parsed
                .host_str()
                .is_some_and(|h| h == "files.slack.com" || h.ends_with(".slack.com"))
        {
            bail!("Unexpected Slack upload destination");
        }
        let res = self
            .client
            .post(url)
            .header("Content-Type", "application/octet-stream")
            .body(reqwest::blocking::Body::sized(file, len))
            .send()
            .map_err(|_| anyhow::anyhow!("File transfer failed; upload was not finalized"))?;
        if !res.status().is_success() {
            bail!("File transfer HTTP {}; upload not finalized", res.status());
        }
        let mut params =
            json!({"files":[{"id":ticket["file_id"],"title":filename}],"channel_id":channel});
        if let Some(s) = caption {
            params["initial_comment"] = json!(s);
        }
        if let Some(t) = thread {
            params["thread_ts"] = json!(t);
        }
        self.api("files.completeUploadExternal", params)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn mock(responses: Vec<String>) -> (Slack, std::thread::JoinHandle<Vec<String>>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/", listener.local_addr().unwrap());
        let worker = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for response in responses {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut byte = [0];
                while !bytes.ends_with(b"\r\n\r\n") {
                    stream.read_exact(&mut byte).unwrap();
                    bytes.push(byte[0]);
                }
                let header = String::from_utf8(bytes.clone()).unwrap();
                let len = header
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .and_then(|n| n.parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                let mut body = vec![0; len];
                stream.read_exact(&mut body).unwrap();
                bytes.extend(body);
                requests.push(String::from_utf8(bytes).unwrap());
                write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",response.len(),response).unwrap();
            }
            requests
        });
        (
            Slack {
                client: Client::builder()
                    .timeout(Duration::from_secs(5))
                    .build()
                    .unwrap(),
                creds: Credentials {
                    token: "fixture-token".into(),
                    cookie: "fixture-cookie".into(),
                },
                endpoint: Some(endpoint),
            },
            worker,
        )
    }
    #[test]
    fn downloads_reject_untrusted_credential_destinations() {
        assert!(validate_download_url("https://files.slack.com/files-pri/T1-F1/image.png").is_ok());
        for url in [
            "http://files.slack.com/file",
            "https://files.slack.com.evil.test/file",
            "https://evil.test/file",
            "https://user@files.slack.com/file",
            "https://files.slack.com:444/file",
        ] {
            assert!(validate_download_url(url).is_err());
        }
    }

    #[test]
    fn download_is_complete_private_and_never_overwrites() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("image.png");
        let payload = "synthetic-image-bytes";
        let (s, worker) = mock(vec![payload.into()]);
        let response = s.client.get(s.endpoint.as_ref().unwrap()).send().unwrap();
        assert_eq!(
            save_download(response, &output, Some(payload.len() as u64)).unwrap(),
            payload.len() as u64
        );
        worker.join().unwrap();
        assert_eq!(std::fs::read(&output).unwrap(), payload.as_bytes());
        assert_eq!(
            std::fs::metadata(&output).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let (s, worker) = mock(vec!["replacement".into()]);
        let response = s.client.get(s.endpoint.as_ref().unwrap()).send().unwrap();
        assert!(save_download(response, &output, None).is_err());
        worker.join().unwrap();
        assert_eq!(std::fs::read(&output).unwrap(), payload.as_bytes());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn truncated_download_leaves_no_output_or_tempfile() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("image.png");
        let (s, worker) = mock(vec!["short".into()]);
        let response = s.client.get(s.endpoint.as_ref().unwrap()).send().unwrap();
        assert!(save_download(response, &output, Some(100))
            .unwrap_err()
            .to_string()
            .contains("size mismatch"));
        worker.join().unwrap();
        assert!(!output.exists());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn cursor_paging_keeps_dynamic_result_key_and_limits() {
        let (s,worker)=mock(vec![json!({"ok":true,"messages":[{"ts":"1"}],"response_metadata":{"next_cursor":"next-page"}}).to_string(),json!({"ok":true,"messages":[{"ts":"2"}],"response_metadata":{"next_cursor":""}}).to_string()]);
        let result = s
            .list(
                "conversations.replies",
                json!({"channel":"C1","ts":"1"}),
                "messages",
                2,
            )
            .unwrap();
        assert_eq!(result["messages"].as_array().unwrap().len(), 2);
        assert_eq!(result["has_more"], false);
        let requests = worker.join().unwrap();
        assert!(requests[0].contains("limit=2"));
        assert!(requests[1].contains("cursor=next-page"));
        assert!(requests[1].contains("limit=1"));
    }
    #[test]
    fn wire_text_and_thread_are_form_encoded_once() {
        let (s, worker) = mock(vec![json!({"ok":true,"ts":"2"}).to_string()]);
        s.api(
            "chat.postMessage",
            json!({"channel":"C1","thread_ts":"1.0","text":"one\ntwo & three"}),
        )
        .unwrap();
        let request = &worker.join().unwrap()[0];
        assert!(request.contains("text=one%0Atwo+%26+three"));
        assert!(request.contains("thread_ts=1.0"));
    }
    #[test]
    fn slack_failure_is_an_error_not_empty_success() {
        let (s, worker) = mock(vec![json!({"ok":false,"error":"missing_scope"}).to_string()]);
        assert!(s
            .api("conversations.history", json!({}))
            .unwrap_err()
            .to_string()
            .contains("missing_scope"));
        worker.join().unwrap();
    }
    #[test]
    fn form_encodes_structured_upload() {
        let fields = form(&json!({"files":[{"id":"F1"}],"text":"one\ntwo","length":2}));
        assert!(fields.contains(&("text".into(), "one\ntwo".into())));
        assert!(fields.contains(&("files".into(), "[{\"id\":\"F1\"}]".into())));
    }
}
