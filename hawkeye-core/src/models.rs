use crate::config::{SLATE_URL_FILE_EXTENSIONS, SLATE_URL_SCHEMES};
use crate::models::VideoMode::Slate;
use color_eyre::{eyre::eyre, Result};
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::Path;
use url::Url;

#[skip_serializing_none]
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct Watcher {
    pub id: Option<String>,
    pub description: Option<String>,
    pub status: Option<Status>,
    pub status_description: Option<String>,
    pub source: Source,
    pub transitions: Vec<Transition>,
}

impl Watcher {
    pub fn is_valid(&self) -> Result<()> {
        self.transitions
            .clone()
            .into_iter()
            .try_for_each(|t| t.is_valid())
            // Validate the source.
            .and(self.source.is_valid())
    }
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Running,
    Pending,
    Ready,
    Error,
}

#[skip_serializing_none]
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct Source {
    pub ingest_ip: Option<String>,
    pub ingest_port: u32,
    pub container: Container,
    pub codec: Codec,
    pub transport: Protocol,
}

impl Source {
    fn is_valid(&self) -> Result<()> {
        if self.ingest_port > 1024 && self.ingest_port < 60_000 {
            Ok(())
        } else {
            Err(eyre!(
                "Source port {} is not in within the valid range (1024-60000)",
                self.ingest_port
            ))
        }
    }
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Container {
    RawVideo,
    MpegTs,
    Fmp4,
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Codec {
    H264,
    H265,
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq)]
#[serde(tag = "protocol", rename_all = "lowercase")]
pub enum Protocol {
    Rtp,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct Transition {
    pub from: VideoMode,
    pub from_context: Option<FromContext>,
    pub to: VideoMode,
    pub to_context: Option<ToContext>,
    pub actions: Vec<Action>,
}

impl Transition {
    fn is_valid(&self) -> Result<()> {
        self._validate_from_context()
            .and(self._validate_to_context())
    }

    fn _validate_from_context(&self) -> Result<()> {
        if self.from == Slate {
            match self.from_context.as_ref() {
                Some(fc) => fc.is_valid(&self.from),
                None => Err(eyre!("A `from_context` is required for from=Slate")),
            }
        } else {
            // FromContext is only valid for from=slate at the moment.
            match self.from_context {
                Some(_) => Err(eyre!(
                    "A `from_context` is not supported for the `from` state of {:?}",
                    self.from
                )),
                None => Ok(()),
            }
        }
    }

    fn _validate_to_context(&self) -> Result<()> {
        if self.to == Slate {
            match self.to_context.as_ref() {
                Some(fc) => fc.is_valid(&self.from),
                None => Err(eyre!("A `to_context` is required for to=Slate")),
            }
        } else {
            // ToContext is only valid for to=slate at the moment.
            match self.to_context {
                Some(_) => Err(eyre!(
                    "A `to_context` is not supported for the `to` state of {:?}",
                    self.to
                )),
                None => Ok(()),
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct FromContext {
    pub slate_context: Option<SlateContext>,
}

impl FromContext {
    pub fn is_valid(&self, state: &VideoMode) -> Result<()> {
        if state == &VideoMode::Slate {
            match &self.slate_context {
                Some(slate) => slate.is_valid(),
                None => Err(eyre!("A `slate_context` is required for from=Slate")),
            }
        } else {
            Ok(())
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct ToContext {
    pub slate_context: Option<SlateContext>,
}

impl ToContext {
    pub fn is_valid(&self, state: &VideoMode) -> Result<()> {
        if state == &VideoMode::Slate {
            match &self.slate_context {
                Some(slate) => slate.is_valid(),
                None => Err(eyre!("A `slate_context` is required for to=Slate")),
            }
        } else {
            Ok(())
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct SlateContext {
    pub slate_url: String,
}

impl SlateContext {
    fn is_valid(&self) -> Result<()> {
        // Validate slate_url particulars.
        let parsed = Url::parse(&self.slate_url)?;
        let ext = Path::new(parsed.path())
            .extension()
            .and_then(OsStr::to_str)
            .unwrap();
        let scheme = parsed.scheme();

        if !SLATE_URL_FILE_EXTENSIONS.contains(&ext.to_string()) {
            Err(eyre!(
                "Invalid `slate_url` file extension. Valid values are: {}",
                SLATE_URL_FILE_EXTENSIONS.join(", "),
            ))
        } else if !SLATE_URL_SCHEMES.contains(&scheme.to_string()) {
            Err(eyre!(
                "Invalid `slate_url` URL scheme. Valid values are: {}",
                SLATE_URL_SCHEMES.join(", "),
            ))
        } else {
            Ok(())
        }
    }
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum VideoMode {
    Slate,
    Content,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    HttpCall(HttpCall),

    // #[cfg(test)]
    #[serde(skip_serializing, skip_deserializing)]
    FakeAction(FakeAction),
}

// #[cfg(test)]
#[derive(Clone, Debug)]
pub struct FakeAction {
    pub called: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub execute_returns: Option<Result<(), ()>>,
}

// #[cfg(test)]
impl PartialEq for FakeAction {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

// #[cfg(test)]
impl Eq for FakeAction {}

// #[cfg(test)]
impl FakeAction {
    pub fn execute(&mut self) -> color_eyre::Result<()> {
        self.called
            .store(true, std::sync::atomic::Ordering::Release);
        if let Some(result) = self.execute_returns.take() {
            match result {
                Ok(()) => Ok(()),
                Err(_) => Err(color_eyre::Report::msg("Err")),
            }
        } else {
            Err(color_eyre::Report::msg("Err"))
        }
    }
}

#[skip_serializing_none]
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct HttpCall {
    pub method: HttpMethod,
    pub url: String,
    pub description: Option<String>,
    pub authorization: Option<HttpAuth>,
    pub headers: Option<HashMap<String, String>>,
    pub body: Option<String>,
    pub retries: Option<u8>,
    pub timeout: Option<u32>,
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq)]
pub enum HttpMethod {
    POST,
    GET,
    PUT,
    PATCH,
    DELETE,
}

impl ToString for HttpMethod {
    fn to_string(&self) -> String {
        match self {
            HttpMethod::POST => "POST".to_string(),
            HttpMethod::GET => "GET".to_string(),
            HttpMethod::PUT => "PUT".to_string(),
            HttpMethod::PATCH => "PATCH".to_string(),
            HttpMethod::DELETE => "DELETE".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HttpAuth {
    Basic { username: String, password: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::fs::File;
    use std::io::Read;

    fn get_watcher() -> Watcher {
        Watcher {
            id: Some("ee21fc9a-7225-450b-a2a7-2faf914e35b8".to_string()),
            description: Some("UEFA 2020 - Lyon vs. Bayern".to_string()),
            status: Some(Status::Running),
            status_description: None,
            source: Source {
                ingest_ip: None,
                ingest_port: 5000,
                container: Container::MpegTs,
                codec: Codec::H264,
                transport: Protocol::Rtp
            },
            transitions: vec![
                Transition {
                    from: VideoMode::Content,
                    from_context: None,
                    to: VideoMode::Slate,
                    to_context: Option::from(ToContext {
                        slate_context: Option::from(SlateContext {
                            slate_url: "file://./resources/slate_fixtures/slate-0-cbsaa-213x120.jpg".to_string()
                        })
                    }),
                    actions: vec![
                        Action::HttpCall( HttpCall {
                            description: Some("Trigger AdBreak using API".to_string()),
                            method: HttpMethod::POST,
                            url: "http://non-existent.cbs.com/v1/organization/cbsa/channel/slate4/ad-break".to_string(),
                            authorization: Some(HttpAuth::Basic {
                                username: "dev_user".to_string(),
                                password: "something".to_string()
                            }),
                            headers: Some([("Content-Type", "application/json")].iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<HashMap<String, String>>()),
                            body: Some("{\"duration\":300}".to_string()),
                            retries: Some(3),
                            timeout: Some(10),
                        })
                    ]
                },
                Transition {
                from: VideoMode::Slate,
                from_context: Option::from(FromContext {
                    slate_context: Option::from(SlateContext {
                        slate_url: "file://./resources/slate_fixtures/slate-0-cbsaa-213x120.jpg".to_string()
                    })
                }),
                to: VideoMode::Content,
                to_context: None,
                actions: vec ![
                        Action::HttpCall( HttpCall {
                            description: Some("Use dump out of AdBreak API call".to_string()),
                            method: HttpMethod::DELETE,
                            url: "http://non-existent.cbs.com/v1/organization/cbsa/channel/slate4/ad-break".to_string(),
                            authorization: Some(HttpAuth::Basic {
                                username: "dev_user".to_string(),
                                password: "something".to_string()
                            }),
                            headers: None,
                            body: None,
                            retries: None,
                            timeout: Some(10),
                        })
                    ]
                }
            ]
        }
    }

    #[test]
    fn check_slate_url_accepts_valid_url() {
        let slate_context = SlateContext {
            slate_url: "http://bar.baz/zing.png".to_string(),
        };
        assert!(slate_context.is_valid().is_ok());
    }

    #[test]
    fn check_slate_url_validates_scheme() {
        let slate_context = SlateContext {
            slate_url: "foo://bar.baz/zing.png".to_string(),
        };
        assert!(slate_context.is_valid().is_err());
        assert!(slate_context
            .is_valid()
            .err()
            .unwrap()
            .to_string()
            .contains(&"URL scheme"));
    }

    #[test]
    fn check_slate_url_validates_extension() {
        let slate_context = SlateContext {
            slate_url: "http://bar.baz/zing.foobar".to_string(),
        };
        assert!(slate_context.is_valid().is_err());
        assert!(slate_context
            .is_valid()
            .err()
            .unwrap()
            .to_string()
            .contains(&"file extension"));
    }

    #[test]
    fn check_source_port_is_in_range() {
        let mut w = get_watcher();
        assert!(w.is_valid().is_ok());

        w.source.ingest_port = 1000;
        assert!(w.is_valid().is_err());
    }

    #[test]
    fn deserialize_as_expected() {
        let mut fixture =
            File::open("../fixtures/watcher-basic.json").expect("Fixture was not found!");
        let mut expected_value = String::new();
        fixture.read_to_string(&mut expected_value).unwrap();
        let expected: Watcher = serde_json::from_str(expected_value.as_str()).unwrap();

        assert_eq!(get_watcher(), expected);
    }

    #[test]
    fn serialize_as_expected() {
        let mut fixture =
            File::open("../fixtures/watcher-basic.json").expect("Fixture was not found!");
        let mut expected_value = String::new();
        fixture.read_to_string(&mut expected_value).unwrap();
        let fixture: serde_json::Value = serde_json::from_str(expected_value.as_str()).unwrap();

        let watcher = get_watcher();
        let watcher_json = serde_json::to_string(&watcher).unwrap();
        let watcher_as_value: serde_json::Value =
            serde_json::from_str(watcher_json.as_str()).unwrap();

        assert_eq!(watcher_as_value, fixture);
    }
}
