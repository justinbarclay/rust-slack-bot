extern crate futures;
extern crate hyper;
extern crate pretty_env_logger;
extern crate tokio_core;
extern crate hyper_tls;
extern crate serde_json;
extern crate serde;

#[macro_use]
extern crate serde_derive;

use futures::Stream;
use futures::Future;

use serde_json::Value;

use hyper::{Body, Chunk, Client, Get, Post, StatusCode};
use hyper_tls::HttpsConnector;
use hyper::error::Error;
use hyper::header::ContentLength;
use hyper::server::{Http, Service, Request, Response};

static NOTFOUND: &[u8] = b"Not Found";

struct ResponseExample(tokio_core::reactor::Handle);

#[derive(Serialize, Deserialize)]
struct Attachment{
  title: String,
  image_url: String
}

#[derive(Serialize, Deserialize)]
struct SlackMessage{
  channel: String,
  attachments: [Attachment; 1],
}

fn parse_response(body: &Chunk) -> Result<String, Error> {
  let v: Value = serde_json::from_slice(&body).unwrap();
  let parsed_result = v["data"]["children"][0]["data"]["url"].to_string();
  if parsed_result.is_empty() || parsed_result == "null"  {
    Err(Error::Status)
  } else {
    Ok(parsed_result)
  }
}

fn make_slack_response(url: String) -> String {
  let attachment = Attachment {
    title: "Someone is panicing".to_string(),
    image_url: url.to_string()
  };

  let message = SlackMessage {channel:"#general".to_string(),
                              attachments: [attachment]};
  serde_json::to_string(&message).unwrap()
}

fn get_top_aww_post(handler: &tokio_core::reactor::Handle) -> Box<Future<Item=hyper::Response, Error=hyper::Error>>{
  let client = Client::configure()
    .connector(HttpsConnector::new(4, handler).unwrap())
    .build(handler);
  let mut req = Request::new(Get, "https://www.reddit.com/r/aww/top/.json?limit=1".parse().unwrap());
  let web_res_future = client.request(req);

  Box::new(web_res_future.and_then(|web_res| {
    web_res.body().concat2().and_then( move |body| {
      let slack_message = match parse_response(&body){
        Ok(response) => make_slack_response(response),
        Err(_e) => "Error".to_string()
      };
      println!("{:?}", slack_message);
      Ok(
        Response::new()
          .with_status(StatusCode::Ok)
          .with_body(slack_message))
    })
  }))
}

impl Service for ResponseExample {
  type Request = Request;
  type Response = Response;
  type Error = hyper::Error;
  type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

  fn call(&self, req: Request) -> Self::Future {
    match (req.method(), req.path()) {
      (&Get, "/panic") => {
        get_top_aww_post(&self.0)
      }
      _ => {
        let body = Body::from("Not found");
        Box::new(futures::future::ok(Response::new()
                                     .with_status(StatusCode::NotFound)
                                     .with_header(ContentLength(NOTFOUND.len() as u64))
                                     .with_body("Not found")))
      }
    }
  }
}

fn main() {
  pretty_env_logger::init();

  let mut addr: std::net::SocketAddr = "127.0.0.1:3000".parse().unwrap();

  let port = match std::env::var("RUST_PORT") {
    Ok(val) => val.parse::<u16>().unwrap(),
    Err(_) => "3000".parse::<u16>().unwrap()
  };

  addr.set_port(port);
  let mut core = tokio_core::reactor::Core::new().unwrap();
  let server_handle = core.handle();
  let client_handle = core.handle();

  let serve = Http::new().serve_addr_handle(&addr, &server_handle, move || Ok(ResponseExample(client_handle.clone()))).unwrap();
  println!("Listening on http://{} with 1 thread.", serve.incoming_ref().local_addr());


   let h2 = server_handle.clone();
    server_handle.spawn(serve.for_each(move |conn| {
        h2.spawn(conn.map(|_| ()).map_err(|err| println!("serve error: {:?}", err)));
        Ok(())
    }).map_err(|_| ()));
  core.run(futures::future::empty::<(), ()>()).unwrap();
}
