//! Running examples against a live test handler (`spec/rules.md` section 12.4).

use std::time::Duration;

use veld_ir::{Example, Ir};
use veld_rules::{ExampleReport, fill_example};

use crate::error::LiveError;
use crate::judge::{Answer, judge};
use crate::request::{HttpRequest, PROBE_PATH, build_request};

const TIMEOUT: Duration = Duration::from_secs(30);

/// A server that has been probed and found to be a test handler.
#[derive(Debug, Clone)]
pub struct Live {
    base: String,
    dry_run: bool,
    agent: ureq::Agent,
}

impl Live {
    /// Probes `GET <base_url>/__veld/test`. A server that does not answer as a test handler is
    /// refused before any example is sent. With `dry_run`, an example without `returns` stops
    /// before the application's handler.
    ///
    /// # Errors
    /// [`LiveError::Connect`] when the server cannot be reached and
    /// [`LiveError::NotTestHandler`] when it answers the probe as anything but a test handler.
    pub fn connect(base_url: &str, dry_run: bool) -> Result<Live, LiveError> {
        let live = Live {
            base: base_url.trim_end_matches('/').to_owned(),
            dry_run,
            agent: ureq::AgentBuilder::new()
                .redirects(0)
                .timeout(TIMEOUT)
                .build(),
        };
        live.probe()?;
        Ok(live)
    }

    /// The base URL, without a trailing slash.
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base
    }

    fn probe(&self) -> Result<(), LiveError> {
        let probe = HttpRequest {
            method: "GET".to_owned(),
            target: PROBE_PATH.to_owned(),
            headers: vec![("Accept".to_owned(), "application/json".to_owned())],
            body: None,
        };
        let url = format!("{}{PROBE_PATH}", self.base);
        let not = |reason: String| LiveError::NotTestHandler {
            url: self.base.clone(),
            reason,
        };
        let answer = self.send(&probe)?;
        if answer.status != 200 {
            return Err(not(format!(
                "`GET {PROBE_PATH}` answered HTTP {}",
                answer.status
            )));
        }
        match serde_json::from_str::<serde_json::Value>(&answer.body) {
            Ok(serde_json::Value::Object(_)) => Ok(()),
            Ok(_) | Err(_) => Err(not(format!(
                "`GET {url}` did not answer with the rules digests as a JSON object"
            ))),
        }
    }

    /// Sends one request and returns whatever the server answered, including `4xx` and `5xx`.
    ///
    /// # Errors
    /// [`LiveError::Connect`] when there is no answer.
    pub fn send(&self, req: &HttpRequest) -> Result<Answer, LiveError> {
        let url = format!("{}{}", self.base, req.target);
        let mut call = self.agent.request(&req.method, &url);
        for (name, value) in &req.headers {
            call = call.set(name, value);
        }
        let sent = match &req.body {
            Some(body) => call.send_string(body),
            None => call.call(),
        };
        match sent {
            Ok(resp) | Err(ureq::Error::Status(_, resp)) => Ok(read_answer(resp)),
            Err(ureq::Error::Transport(t)) => Err(LiveError::Connect {
                url,
                message: t.to_string(),
            }),
        }
    }

    /// Fills in `ex`, sends it and compares the answer with its expected outcome.
    ///
    /// # Errors
    /// [`LiveError::Example`] when the example cannot be filled in, [`LiveError::Unsupported`]
    /// when no request can be built for its endpoint and [`LiveError::Connect`] when the
    /// connection fails.
    pub fn run_example(&self, ir: &Ir, ex: &Example) -> Result<ExampleReport, LiveError> {
        let filled = fill_example(ir, ex)?;
        let req = build_request(ir, &filled, self.dry_run)?;
        let answer = self.send(&req)?;
        Ok(judge(ir, &filled, self.dry_run, &answer))
    }
}

fn read_answer(resp: ureq::Response) -> Answer {
    let status = resp.status();
    let headers = resp
        .headers_names()
        .into_iter()
        .filter_map(|name| {
            let value = resp.header(&name)?.to_owned();
            Some((name.to_ascii_lowercase(), value))
        })
        .collect();
    let body = resp.into_string().unwrap_or_default();
    Answer {
        status,
        headers,
        body,
    }
}
