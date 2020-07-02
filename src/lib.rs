use reqwest::multipart::Part;
use reqwest::{multipart, Body, Client, Method};
use std::borrow::Cow;
use tokio::io::AsyncWriteExt;
use tokio::stream::StreamExt;

/*
   --form paperWidth=8.27 \
   --form paperHeight=11.69 \
   --form marginTop=0 \
   --form marginBottom=0 \
   --form marginLeft=0 \
   --form marginRight=0 \
   --form landscape=true \
   --form scale=0.75 \
*/

pub struct Html {
    client: Client,
    url: String,
    form: multipart::Form,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("failed to write output: {0}")]
    Io(#[from] std::io::Error),
    #[error("received {0}: {1}")]
    Gotenberg(u16, String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Html {
    pub fn new(client: Client, endpoint: impl Into<String>) -> Self {
        Self {
            client,
            url: build_url(endpoint, "convert/html"),
            form: multipart::Form::new(),
        }
    }

    pub fn pages<'a>(mut self, pages: impl Into<Cow<'static, str>>) -> Self {
        self.form = self.form.part("pageRanges", Part::text(pages));
        self
    }

    pub fn file(
        mut self,
        filenames: impl Into<Cow<'static, str>>,
        content: impl Into<Body>,
    ) -> Self {
        let part = Part::stream(content)
            .file_name(filenames)
            .mime_str("application/octet-stream")
            .unwrap();

        self.form = self.form.part("files", part);
        self
    }

    pub async fn convert(self, mut out: impl tokio::io::AsyncWrite + Unpin) -> Result<()> {
        let request = self
            .client
            .request(Method::POST, &self.url)
            .multipart(self.form)
            .build()?;

        let response = self.client.execute(request).await?;
        if !response.status().is_success() {
            return Err(Error::Gotenberg(
                response.status().as_u16(),
                response.text().await.unwrap_or_else(|err| err.to_string()),
            ));
        }

        let mut bytes = response.bytes_stream();

        while let Some(chunk) = bytes.try_next().await? {
            out.write_all(&chunk).await?
        }

        Ok(())
    }
}

fn build_url(endpoint: impl Into<String>, path: &str) -> String {
    let mut url = endpoint.into();
    if !url.ends_with("/") {
        url.push_str("/");
    }
    url.push_str(path);
    url
}
