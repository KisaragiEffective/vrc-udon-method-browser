#![deny(clippy::all)]
#![warn(clippy::nursery)]
#![allow(clippy::future_not_send)]

mod config;
mod github_client;
mod jwt;
mod responses;
mod routing;
mod time;
mod token_broker;

use worker::{Context, Env, Request, Response, Result, event};

#[event(fetch)]
pub async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    routing::fetch(req, env).await
}
