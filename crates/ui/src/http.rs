//! What GPUI fetches an image of the web with, which an application gives
//! it at startup: each GET on a thread of its own, through ureq, with a
//! time and a size it gives up past. GPUI asks it only for images, and a
//! markdown text asks GPUI only for those the person chose to show.

use std::thread;
use std::time::Duration;

use futures::FutureExt as _;
use futures::channel::oneshot;
use futures::future::BoxFuture;
use gpui::http_client::http::HeaderValue;
use gpui::http_client::{self, AsyncBody, Request, Response, Url};

/// The most an image may weigh, and the longest it may take.
const MOST_BYTES: u64 = 20 * 1024 * 1024;
const MOST_TIME: Duration = Duration::from_secs(30);

pub struct ImageClient {
    user_agent: HeaderValue,
}

impl ImageClient {
    /// `user_agent` is what it says it is, such as the window's name.
    pub fn new(user_agent: &str) -> Self {
        ImageClient {
            user_agent: HeaderValue::from_str(user_agent)
                .unwrap_or_else(|_| HeaderValue::from_static("gpui")),
        }
    }
}

/// What `uri` answers with a GET, its body read up to the most an image
/// may weigh.
fn fetch(uri: &str, user_agent: &str) -> anyhow::Result<Response<AsyncBody>> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(MOST_TIME))
        .user_agent(user_agent)
        .http_status_as_error(false)
        .build()
        .into();

    let response = agent.get(uri).call()?;
    let status = response.status();
    let bytes = response
        .into_body()
        .with_config()
        .limit(MOST_BYTES)
        .read_to_vec()?;

    Ok(Response::builder()
        .status(status.as_u16())
        .body(AsyncBody::from(bytes))?)
}

impl http_client::HttpClient for ImageClient {
    fn user_agent(&self) -> Option<&HeaderValue> {
        Some(&self.user_agent)
    }

    fn proxy(&self) -> Option<&Url> {
        None
    }

    /// Only a GET of the request's address: what GPUI asks for an image.
    fn send(
        &self,
        request: Request<AsyncBody>,
    ) -> BoxFuture<'static, anyhow::Result<Response<AsyncBody>>> {
        let uri = request.uri().to_string();
        let user_agent = self.user_agent.to_str().unwrap_or("gpui").to_string();
        let (answer, answered) = oneshot::channel();

        // ureq blocks, and GPUI's futures must not.
        thread::spawn(move || {
            let _ = answer.send(fetch(&uri, &user_agent));
        });

        async move { answered.await? }.boxed()
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read as _, Write as _};
    use std::net::TcpListener;

    use futures::AsyncReadExt as _;
    use gpui::http_client::HttpClient as _;

    use super::ImageClient;

    /// A server on this machine that answers one GET with `body`.
    fn serve(body: &'static [u8]) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();

        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 1024];
            let _ = stream.read(&mut request);
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );

            stream.write_all(head.as_bytes()).unwrap();
            stream.write_all(body).unwrap();
        });

        format!("http://{address}/chart.png")
    }

    #[test]
    fn fetches_what_an_address_answers() {
        let uri = serve(b"\x89PNG not really");
        let client = ImageClient::new("rumbo-gui");

        let mut response = futures::executor::block_on(client.get(&uri, ().into(), true)).unwrap();
        let mut body = Vec::new();

        futures::executor::block_on(response.body_mut().read_to_end(&mut body)).unwrap();

        assert!(response.status().is_success());
        assert_eq!(body, b"\x89PNG not really");
    }
}
