//! A small client for a local [Ollama](https://ollama.com) server, which runs AI models on this PC.
//!
//! It only ever talks to `127.0.0.1`, so nothing written in the diary leaves the computer. The
//! calls are blocking; the app runs them on background threads.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::time::Duration;

use serde_json::{Value, json};

/// Ollama's address on this PC. (Fixed on purpose: the AI helpers never use another machine.)
pub const HOST: &str = "http://127.0.0.1:11434";

/// Models worth suggesting: (tag, description, rough download size).
pub const RECOMMENDED: [(&str, &str, &str); 4] = [
    ("llama3.2:3b", "Llama 3.2 (3B): a good all-rounder", "about 2 GB"),
    ("qwen2.5:3b", "Qwen 2.5 (3B): clear writing", "about 1.9 GB"),
    ("gemma2:2b", "Gemma 2 (2B): smaller and faster", "about 1.6 GB"),
    ("phi3:mini", "Phi-3 mini: compact", "about 2.2 GB"),
];

#[derive(Debug, Clone, PartialEq)]
pub struct ModelInfo {
    pub name: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Progress {
    pub status: String,
    pub completed: u64,
    pub total: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// Nothing is listening: Ollama isn't installed or isn't running.
    NotRunning,
    /// The model isn't installed.
    NoSuchModel(String),
    Other(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotRunning => write!(f, "Ollama isn't running. Open Settings → AI helpers to start it."),
            Error::NoSuchModel(m) => write!(f, "The model \"{m}\" isn't installed. Choose or download one under Settings → AI helpers."),
            Error::Other(e) => write!(f, "{e}"),
        }
    }
}

fn io_error(e: ureq::Error) -> Error {
    match e {
        ureq::Error::Io(_) | ureq::Error::ConnectionFailed | ureq::Error::HostNotFound | ureq::Error::Timeout(_) => Error::NotRunning,
        other => Error::Other(format!("Couldn't talk to Ollama: {other}")),
    }
}

/// While tests run, `Client::new()` can be pointed at a fake server on this PC.
#[cfg(test)]
pub(crate) static TEST_BASE: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

pub struct Client {
    agent: ureq::Agent,
    base: String,
}

impl Client {
    pub fn new() -> Client {
        #[cfg(test)]
        if let Some(base) = TEST_BASE.lock().unwrap_or_else(|e| e.into_inner()).clone() {
            return Client::at(&base);
        }
        Client::at(HOST)
    }

    /// Only the tests point the client anywhere else (at a fake server on this PC).
    #[cfg(test)]
    fn with_base(base: &str) -> Client {
        Client::at(base)
    }

    fn at(base: &str) -> Client {
        let config = ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(2)))
            // The first answer can take a while: the model has to be loaded into memory.
            .timeout_global(None)
            .http_status_as_error(false)
            .proxy(None)
            .build();
        Client { agent: ureq::Agent::new_with_config(config), base: base.trim_end_matches('/').to_string() }
    }

    /// Ollama's version, if it is running.
    pub fn version(&self) -> Result<String, Error> {
        let mut resp = self.agent.get(format!("{}/api/version", self.base)).call().map_err(io_error)?;
        let body = resp.body_mut().read_to_string().map_err(io_error)?;
        serde_json::from_str::<Value>(&body)
            .ok()
            .and_then(|v| v["version"].as_str().map(str::to_string))
            .ok_or_else(|| Error::Other("That doesn't look like Ollama.".into()))
    }

    /// The models that are installed.
    pub fn models(&self) -> Result<Vec<ModelInfo>, Error> {
        let mut resp = self.agent.get(format!("{}/api/tags", self.base)).call().map_err(io_error)?;
        let body = resp.body_mut().read_to_string().map_err(io_error)?;
        let v: Value = serde_json::from_str(&body).map_err(|e| Error::Other(format!("Unexpected answer from Ollama: {e}")))?;
        Ok(v["models"]
            .as_array()
            .map(|list| {
                list.iter()
                    .filter_map(|m| Some(ModelInfo { name: m["name"].as_str()?.to_string(), size: m["size"].as_u64().unwrap_or(0) }))
                    .collect()
            })
            .unwrap_or_default())
    }

    /// POST `body` to `path` and call `on_line` with every JSON line of the streamed answer.
    /// `on_line` returns `false` to stop early. `model` is only used to word errors.
    fn stream(&self, path: &str, body: Value, model: &str, on_line: &mut dyn FnMut(&Value) -> bool) -> Result<(), Error> {
        let resp = self
            .agent
            .post(format!("{}{path}", self.base))
            .header("Content-Type", "application/json")
            .send(body.to_string())
            .map_err(io_error)?;
        let status = resp.status().as_u16();
        let mut reader = BufReader::new(resp.into_body().into_reader());

        if status != 200 {
            let mut text = String::new();
            std::io::Read::read_to_string(&mut reader, &mut text).ok();
            let message = serde_json::from_str::<Value>(&text).ok().and_then(|v| v["error"].as_str().map(str::to_string)).unwrap_or(text);
            return Err(if status == 404 { Error::NoSuchModel(model.to_string()) } else { Error::Other(format!("Ollama said: {}", message.trim())) });
        }
        for line in reader.lines() {
            let line = line.map_err(|e| Error::Other(format!("The connection to Ollama was interrupted: {e}")))?;
            if line.trim().is_empty() {
                continue;
            }
            let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
            if let Some(err) = v["error"].as_str() {
                return Err(Error::Other(format!("Ollama said: {err}")));
            }
            if !on_line(&v) {
                return Ok(());
            }
        }
        Ok(())
    }

    /// Ask the model, passing each piece of its answer to `on_token` as it arrives. Return
    /// `false` from `on_token` to stop.
    pub fn chat(&self, model: &str, system: &str, user: &str, on_token: &mut dyn FnMut(&str) -> bool) -> Result<(), Error> {
        let body = json!({
            "model": model,
            "stream": true,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user },
            ],
            "options": { "temperature": 0.5, "num_predict": 700 },
        });
        self.stream("/api/chat", body, model, &mut |v| {
            let piece = v["message"]["content"].as_str().unwrap_or("");
            (piece.is_empty() || on_token(piece)) && v["done"].as_bool() != Some(true)
        })
    }

    /// Download a model, reporting progress. Return `false` from `on_progress` to cancel.
    pub fn pull(&self, model: &str, on_progress: &mut dyn FnMut(Progress) -> bool) -> Result<(), Error> {
        let body = json!({ "model": model, "name": model, "stream": true });
        self.stream("/api/pull", body, model, &mut |v| {
            on_progress(Progress {
                status: v["status"].as_str().unwrap_or("").to_string(),
                completed: v["completed"].as_u64().unwrap_or(0),
                total: v["total"].as_u64().unwrap_or(0),
            })
        })
    }
}

// ---------------------------------------------------------------------------------------------
// Finding, starting and installing Ollama
// ---------------------------------------------------------------------------------------------

/// Where `ollama.exe` is, if it is installed.
pub fn find_exe() -> Option<PathBuf> {
    let from_env = |var: &str, rest: &str| std::env::var_os(var).map(|b| PathBuf::from(b).join(rest));
    let known = [
        from_env("LOCALAPPDATA", "Programs\\Ollama\\ollama.exe"),
        from_env("ProgramFiles", "Ollama\\ollama.exe"),
        from_env("ProgramW6432", "Ollama\\ollama.exe"),
    ];
    if let Some(found) = known.into_iter().flatten().find(|p| p.is_file()) {
        return Some(found);
    }
    // Anywhere on the PATH.
    let out = std::process::Command::new("where").arg("ollama").output().ok()?;
    String::from_utf8_lossy(&out.stdout).lines().map(|l| PathBuf::from(l.trim())).find(|p| p.is_file())
}

/// Start the Ollama server in the background (no window).
pub fn start_server(exe: &std::path::Path) -> std::io::Result<()> {
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("serve").stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd.spawn().map(|_| ())
}

/// The arguments for installing Ollama with Windows' own package manager.
pub fn winget_args() -> [&'static str; 8] {
    ["install", "--id", "Ollama.Ollama", "-e", "--silent", "--accept-package-agreements", "--accept-source-agreements", "--disable-interactivity"]
}

pub fn winget_available() -> bool {
    std::process::Command::new("winget").arg("--version").stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().is_ok_and(|s| s.success())
}

/// Install Ollama with winget, passing its progress lines to `on_line`. The installer is
/// downloaded and checked by winget, and installs for the current user only.
pub fn install_with_winget(on_line: &mut dyn FnMut(&str)) -> Result<(), String> {
    use std::process::{Command, Stdio};
    let mut cmd = Command::new("winget");
    cmd.args(winget_args()).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let mut child = cmd.spawn().map_err(|e| format!("Couldn't start winget: {e}"))?;
    if let Some(out) = child.stdout.take() {
        // winget redraws progress bars with carriage returns; show each as its own line.
        for chunk in BufReader::new(out).split(b'\n') {
            let Ok(bytes) = chunk else { break };
            for part in String::from_utf8_lossy(&bytes).split('\r') {
                let t = part.trim();
                if !t.is_empty() {
                    on_line(t);
                }
            }
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    if status.success() { Ok(()) } else { Err(format!("winget finished with an error ({status}). You can install Ollama from ollama.com instead.")) }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};

    pub(crate) struct Request {
        pub method: String,
        pub path: String,
        pub body: String,
    }

    pub(crate) enum Reply {
        Json(u16, String),
        /// Streamed (chunked) newline-delimited JSON, like Ollama sends.
        Lines(Vec<String>),
    }

    fn read_request(stream: &mut TcpStream) -> Option<Request> {
        let mut buf = Vec::new();
        let mut byte = [0u8; 1];
        while !buf.ends_with(b"\r\n\r\n") {
            if stream.read(&mut byte).ok()? == 0 {
                return None;
            }
            buf.push(byte[0]);
        }
        let head = String::from_utf8_lossy(&buf).to_string();
        let mut first = head.lines().next()?.split_whitespace();
        let (method, path) = (first.next()?.to_string(), first.next()?.to_string());
        let len = head.lines().find_map(|l| l.to_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0))).unwrap_or(0);
        let mut body = vec![0u8; len];
        stream.read_exact(&mut body).ok()?;
        Some(Request { method, path, body: String::from_utf8_lossy(&body).to_string() })
    }

    /// A fake Ollama on this PC. Every request is passed to `handler`.
    pub(crate) fn serve(handler: impl Fn(&Request) -> Reply + Send + 'static) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                let Some(req) = read_request(&mut stream) else { continue };
                match handler(&req) {
                    Reply::Json(code, body) => {
                        let head = format!("HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                        stream.write_all(head.as_bytes()).ok();
                        stream.write_all(body.as_bytes()).ok();
                    }
                    Reply::Lines(lines) => {
                        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n").ok();
                        for line in lines {
                            let chunk = format!("{line}\n");
                            if stream.write_all(format!("{:x}\r\n{chunk}\r\n", chunk.len()).as_bytes()).is_err() {
                                break; // the client hung up (cancelled)
                            }
                            std::thread::sleep(Duration::from_millis(5));
                        }
                        stream.write_all(b"0\r\n\r\n").ok();
                    }
                }
            }
        });
        base
    }

    #[test]
    fn finds_the_version_and_the_installed_models() {
        let base = serve(|r| match r.path.as_str() {
            "/api/version" => Reply::Json(200, r#"{"version":"0.5.1"}"#.into()),
            "/api/tags" => Reply::Json(200, r#"{"models":[{"name":"llama3.2:3b","size":2019393189},{"name":"gemma2:2b","size":1629518495}]}"#.into()),
            _ => Reply::Json(404, "{}".into()),
        });
        let client = Client::with_base(&base);
        assert_eq!(client.version().unwrap(), "0.5.1");
        let models = client.models().unwrap();
        assert_eq!(models.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), vec!["llama3.2:3b", "gemma2:2b"]);
        assert_eq!(models[1].size, 1_629_518_495);
    }

    #[test]
    fn nothing_listening_means_ollama_is_not_running() {
        // Port 1 is never open.
        let client = Client::with_base("http://127.0.0.1:1");
        assert_eq!(client.version(), Err(Error::NotRunning));
        assert_eq!(client.models().unwrap_err(), Error::NotRunning);
        let mut got = String::new();
        assert_eq!(client.chat("m", "s", "u", &mut |t| { got.push_str(t); true }), Err(Error::NotRunning));
        assert!(Error::NotRunning.to_string().contains("isn't running"));
        // A different server on that port is not mistaken for Ollama.
        let other = serve(|_| Reply::Json(200, "<html>not ollama</html>".into()));
        assert!(matches!(Client::with_base(&other).version(), Err(Error::Other(_))));
    }

    #[test]
    fn a_chat_streams_tokens_and_sends_the_system_and_user_text() {
        let seen = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let seen_in = seen.clone();
        let base = serve(move |r| {
            assert_eq!((r.method.as_str(), r.path.as_str()), ("POST", "/api/chat"));
            *seen_in.lock().unwrap() = r.body.clone();
            Reply::Lines(vec![
                r#"{"message":{"role":"assistant","content":"Today "},"done":false}"#.into(),
                r#"{"message":{"role":"assistant","content":"was "},"done":false}"#.into(),
                r#"{"message":{"role":"assistant","content":"good."},"done":false}"#.into(),
                r#"{"message":{"role":"assistant","content":""},"done":true}"#.into(),
            ])
        });
        let mut text = String::new();
        Client::with_base(&base).chat("llama3.2:3b", "You write diaries.", "I walked the dog.", &mut |t| {
            text.push_str(t);
            true
        }).unwrap();
        assert_eq!(text, "Today was good.");

        let sent: Value = serde_json::from_str(&seen.lock().unwrap()).unwrap();
        assert_eq!(sent["model"], "llama3.2:3b");
        assert_eq!(sent["stream"], true);
        assert_eq!(sent["messages"][0], json!({"role": "system", "content": "You write diaries."}));
        assert_eq!(sent["messages"][1], json!({"role": "user", "content": "I walked the dog."}));
    }

    #[test]
    fn a_chat_can_be_cancelled_part_way() {
        let lines: Vec<String> = (0..200).map(|i| format!(r#"{{"message":{{"content":"w{i} "}},"done":false}}"#)).collect();
        let base = serve(move |_| Reply::Lines(lines.clone()));
        let mut count = 0;
        Client::with_base(&base).chat("m", "s", "u", &mut |_| {
            count += 1;
            count < 3
        }).unwrap();
        assert_eq!(count, 3, "it stopped as soon as asked");
    }

    #[test]
    fn errors_from_ollama_are_explained() {
        let base = serve(|r| {
            if r.body.contains("\"missing\"") {
                Reply::Json(404, r#"{"error":"model \"missing\" not found, try pulling it first"}"#.into())
            } else if r.body.contains("\"broken\"") {
                Reply::Json(500, r#"{"error":"out of memory"}"#.into())
            } else {
                Reply::Lines(vec![r#"{"message":{"content":"ok "}}"#.into(), r#"{"error":"the model crashed"}"#.into()])
            }
        });
        let client = Client::with_base(&base);
        let err = client.chat("missing", "s", "u", &mut |_| true).unwrap_err();
        assert_eq!(err, Error::NoSuchModel("missing".into()));
        assert!(err.to_string().contains("isn't installed"));
        assert_eq!(client.chat("broken", "s", "u", &mut |_| true).unwrap_err(), Error::Other("Ollama said: out of memory".into()));
        // An error part-way through the answer is reported after the words that did arrive.
        let mut got = String::new();
        let err = client.chat("other", "s", "u", &mut |t| { got.push_str(t); true }).unwrap_err();
        assert_eq!((got.as_str(), err), ("ok ", Error::Other("Ollama said: the model crashed".into())));
    }

    #[test]
    fn downloading_a_model_reports_progress_and_can_be_cancelled() {
        let lines = vec![
            r#"{"status":"pulling manifest"}"#.to_string(),
            r#"{"status":"pulling 6340dc3229b0","digest":"sha256:6340","total":2000,"completed":500}"#.to_string(),
            r#"{"status":"pulling 6340dc3229b0","digest":"sha256:6340","total":2000,"completed":2000}"#.to_string(),
            r#"{"status":"success"}"#.to_string(),
        ];
        let base = serve(move |r| {
            assert_eq!(r.path, "/api/pull");
            assert!(r.body.contains("gemma2:2b"));
            Reply::Lines(lines.clone())
        });
        let client = Client::with_base(&base);
        let mut seen = Vec::new();
        client.pull("gemma2:2b", &mut |p| {
            seen.push((p.status.clone(), p.completed, p.total));
            true
        }).unwrap();
        assert_eq!(seen.first().unwrap().0, "pulling manifest");
        assert_eq!(seen[1], ("pulling 6340dc3229b0".to_string(), 500, 2000));
        assert_eq!(seen.last().unwrap().0, "success");

        let mut n = 0;
        client.pull("gemma2:2b", &mut |_| {
            n += 1;
            false
        }).unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn install_command_and_model_list_are_sensible() {
        let args = winget_args();
        assert_eq!(&args[..4], &["install", "--id", "Ollama.Ollama", "-e"]);
        assert!(args.contains(&"--accept-package-agreements") && args.contains(&"--silent"));
        assert!(RECOMMENDED.iter().all(|(tag, _, size)| tag.contains(':') && size.starts_with("about")));
        // The client can only be built for this PC.
        assert!(HOST.starts_with("http://127.0.0.1:"));
    }
}
