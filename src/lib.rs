//! A simple [Neocities](https://neocities.org) API client wrapper.
//!
//! # Usage:
//!
//! Start by constructing a [`Neocities`] instance using an API key with [`Neocities::key`]
//! or a username/password combo using [`Neocities::login`].
//!
//! After that you are free to call any methods on the [`Neocities`]
//! instance to use their respective API calls
use reqwest::{
    multipart::{Form, Part},
    Body, RequestBuilder,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const API_URL: &str = "https://neocities.org/api/";

enum Auth {
    Login { username: String, password: String },
    Key(String),
}

/// The main Neocities API client wrapper.
pub struct Neocities {
    auth: Auth,
    client: reqwest::Client,
}

/// A path and its metadata returned by the server.
#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum ListEntry {
    File {
        path: String,
        size: i64,
        updated_at: String,
        sha1_hash: String,
    },
    Directory {
        path: String,
        updated_at: String,
    },
}

/// Info about a Neocities site
#[derive(Serialize, Deserialize, Debug)]
pub struct Info {
    #[serde(rename = "sitename")]
    pub site_name: String,
    pub hits: i64,
    pub created_at: String,
    pub last_updated: String,
    pub domain: Option<String>,
    pub tags: Vec<String>,
}

// Generic type for handling the `result` field in all API responses
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "result")]
enum ApiResult<T> {
    #[serde(rename = "error")]
    Error {
        #[serde(default)]
        error_type: String,
        #[serde(default)]
        message: String,
    },
    #[serde(rename = "success")]
    Success {
        #[serde(alias = "info")]
        #[serde(alias = "files")]
        #[serde(alias = "api_key")]
        #[serde(alias = "message")]
        data: T,
    },
}

impl<T> ApiResult<T> {
    fn to_result(self) -> Result<T, NeocitiesError> {
        match self {
            ApiResult::Success { data } => Ok(data),
            ApiResult::Error {
                error_type,
                message,
            } => Err(NeocitiesError::ApiErr(
                error_type.to_string(),
                message.to_string(),
            )),
        }
    }
}

impl Neocities {
    /// Create a new [`Neocities`] client authenticated using an API key
    pub fn new(key: String) -> Self {
        let client = reqwest::Client::new();

        Self {
            auth: Auth::Key(key),
            client,
        }
    }

    /// Create a new [`Neocities`] client authenticated using a username and password
    pub fn login(username: String, password: String) -> Self {
        let client = reqwest::Client::new();
        let auth = Auth::Login { username, password };

        Self { client, auth }
    }

    /// Get a list of files in the authorized site. `path` can be used to specify
    /// which directory to list the files in. If `path` is empty it will list all items.
    pub async fn list<T: AsRef<str>>(&self, path: T) -> Result<Vec<ListEntry>, NeocitiesError> {
        let mut request = self.client.get(API_URL.to_string() + "list");
        request = add_authorization_header(request, &self.auth);

        if !path.as_ref().is_empty() {
            request = request.form(&[("path", path.as_ref())]);
        }

        let response = request.send().await?.error_for_status()?;
        response
            .json::<ApiResult<Vec<ListEntry>>>()
            .await?
            .to_result()
    }

    /// Get info about a Neocities site.
    /// If `site_name` is empty it will get info about the site used for authentication
    pub async fn info<T: AsRef<str>>(&self, site_name: T) -> Result<Info, NeocitiesError> {
        let mut request = self.client.get(API_URL.to_string() + "info");
        request = add_authorization_header(request, &self.auth);

        if !site_name.as_ref().is_empty() {
            request = request.form(&[("sitename", site_name.as_ref())]);
        }

        let response = request.send().await?.error_for_status()?;
        response.json::<ApiResult<Info>>().await?.to_result()
    }

    /// Get the API key for the currently authorized account.
    /// If the account has no current key, one will be newly generated
    pub async fn key(&self) -> Result<String, NeocitiesError> {
        let mut request = self.client.get(API_URL.to_string() + "key");
        request = add_authorization_header(request, &self.auth);

        let response = request.send().await?.error_for_status()?;
        response.json::<ApiResult<String>>().await?.to_result()
    }

    /// Upload a file to the current [`Neocities`] site.
    /// Returns the success message sent by the server
    pub async fn upload<T: Into<Body>>(
        &self,
        file_path: String,
        file: T,
    ) -> Result<String, NeocitiesError> {
        let part = Part::stream(file).file_name(file_path.clone());
        let form = Form::new().part(file_path, part);

        let mut request = self.client.post(API_URL.to_string() + "upload");
        request = add_authorization_header(request, &self.auth);
        request = request.multipart(form);

        let response = request.send().await?;

        response.json::<ApiResult<String>>().await?.to_result()
    }

    /// Delete files from the current [`Neocities`] site.
    /// Returns the success message sent by the server
    pub async fn delete<T: AsRef<[String]>>(
        &self,
        file_paths: T,
    ) -> Result<String, NeocitiesError> {
        let mut request = self.client.post(API_URL.to_string() + "delete");
        request = add_authorization_header(request, &self.auth);

        for path in file_paths.as_ref() {
            request = request.query(&[("filenames[]", path.as_str())]);
        }

        request
            .send()
            .await?
            .json::<ApiResult<String>>()
            .await?
            .to_result()
    }
}

fn add_authorization_header(request: RequestBuilder, auth: &Auth) -> RequestBuilder {
    match auth {
        Auth::Login { username, password } => request.basic_auth(username, Some(password)),
        Auth::Key(key) => request.bearer_auth(key),
    }
}

/// The `neocities` error type.
#[derive(Error, Debug)]
pub enum NeocitiesError {
    #[error("API returned error `{0}` with message `{1}`")]
    ApiErr(String, String),
    #[error(transparent)]
    ReqwestErr(#[from] reqwest::Error),
}
