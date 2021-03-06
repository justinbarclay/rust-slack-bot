extern crate regex;
extern crate futures;
extern crate hyper;
extern crate tokio_core;
extern crate hyper_tls;
extern crate serde_json;
extern crate serde;
extern crate rand;

use futures::Stream;
use futures::Future;

use serde_json::Value;
use rand::prelude::*;
use self::hyper::{Chunk, Client, Get, StatusCode};
use hyper_tls::HttpsConnector;
use self::hyper::error::Error as Error;
use self::hyper::server::{Request, Response};
use self::hyper::header::{Headers, ContentType};

use regex::Regex;

#[derive(Serialize, Deserialize)]
struct Attachment {
  title: String,
  image_url: String,
}

#[derive(Serialize, Deserialize)]
struct SlackMessage {
  response_type: String,
  channel: String,
  attachments: [Attachment; 1],
}

fn find_good_url(children: &Value, index: usize, max: usize, start: usize) -> String {
  let url = children[index]["data"]["url"].to_string().replace("\"", "");
  let copied_url = url.clone();
  let pattern = Regex::new(r"(\.gif|\.jpg|\.png|\.bmp)\b").unwrap();

  let image = match pattern.captures(&copied_url) {
    Some(_) => true,
    None => false
  };

  // I am way too lazy to exhaustively check all 10
  if image {
    url
  } else if index == 0 {
    find_good_url(children, max, max, start)
  }else if index == start + 1{
    "http://i.imgur.com/5qMAsSS.gif".to_string()
  } else {
    find_good_url(children, index-1, max, start)
  }
}

fn parse_response(body: &Chunk) -> ::std::result::Result<String, Error> {
  let mut rng = thread_rng();
  let index = rng.gen_range(0, 9);

  let v: Value = serde_json::from_slice(&body).unwrap();
  let children = &v["data"]["children"];

  let url = find_good_url(children, index, 10, index);

  if url.is_empty() || url == "null" {
    Err(hyper::error::Error::Status)
  } else {
    Ok(url)
  }
}

fn make_slack_response(url: String) -> String {
  let attachment = Attachment {
    title: "Don't panic! Here is a cute picture to soothe you.".to_string(),
    image_url: url
  };

  let message = SlackMessage {
    response_type: "ephemeral".to_string(),
    channel: "#general".to_string(),
    attachments: [attachment],
  };
  serde_json::to_string(&message).unwrap()
}

pub fn get_top_aww_post(
  handler: &tokio_core::reactor::Handle,
) -> Box<Future<Item = hyper::Response, Error = hyper::Error>> {

  let client = Client::configure()
    .connector(HttpsConnector::new(4, handler).unwrap())
    .build(handler);
  let req = Request::new(
    Get,
    "https://www.reddit.com/r/aww/top/.json?limit=10"
      .parse()
      .unwrap(),
  );
  let web_res_future = client.request(req);

  Box::new(web_res_future.and_then(|web_res| {
    web_res.body().concat2().and_then(move |body| {
      let slack_message = match parse_response(&body) {
        Ok(response) => {
          make_slack_response(response)
        },
        Err(_e) => "Error parsing JSON".to_string(),
      };
      println!("{:?}", slack_message);
      let mut headers = Headers::new();
      headers.set(
        ContentType::json()
      );
      Ok(
        Response::new()
          .with_headers(headers)
          .with_status(StatusCode::Ok)
          .with_body(slack_message),
      )
    })
  }))
}
