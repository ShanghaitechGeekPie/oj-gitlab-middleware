/*
 *  Copyright (c) 2018-2019, llk89.
 *
 *  This program is free software: you can redistribute it and/or modify
 *  it under the terms of the GNU Affero General Public License as
 *  published by the Free Software Foundation, either version 3 of the
 *  License, or (at your option) any later version.
 *
 *  This program is distributed in the hope that it will be useful,
 *  but WITHOUT ANY WARRANTY; without even the implied warranty of
 *  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 *  GNU Affero General Public License for more details.
 *
 *  You should have received a copy of the GNU Affero General Public License
 *  along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

use std::borrow::Cow;
use std::net::IpAddr;
use std::convert::From;

use ::{Error, GMResult};

use reqwest::{Client, ClientBuilder, Method, Response};
use reqwest::header::{HeaderMap, HeaderValue};
use url::Url;

use rocket::{Outcome, Request, State};
use rocket::http::Status;
use rocket::request::{self, FromRequest};
use rocket_contrib::json::JsonValue;
use serde::Serialize;

use hex::encode;
use sha2::{Digest, Sha512};

pub trait APIFunction: Serialize {
    fn method() -> Method { Method::POST }
    fn path(&self) -> Cow<str>;
}

pub trait APIAccessor {
    #[inline]
    fn call<T: ?Sized + APIFunction>(&self, api_call: &T) -> GMResult<Response> {
        self.execute(T::method(), &api_call.path(), api_call, None)
    }

    #[inline]
    fn call_sudo<T: ?Sized + APIFunction>(&self, api_call: &T, sudo: &str) -> GMResult<Response> {
        self.execute(T::method(), &api_call.path(), api_call, Some(sudo))
    }

    #[inline]
    fn call_no_body(&self, method: Method, path: &str) -> GMResult<Response> {
        self.execute(method, path, "", None)
    }

    #[inline]
    fn call_sudo_no_body(&self, method: Method, path: &str, sudo: &str) -> GMResult<Response> {
        self.execute(method, path, "", Some(sudo))
    }

    fn execute<T: Serialize + ?Sized>(&self, method: Method, path: &str, body: &T, sudo: Option<&str>) -> GMResult<Response> {
        if let Some(user) = sudo {
            self.client().request(method, self.base().join(path).expect("Invalid URL"))
                .json(body)
                .header("sudo", user)
                .send()
        } else {
            self.client().request(method, self.base().join(path).expect("Invalid URL"))
                .json(body)
                .send()
        }?.error_for_status().map_err(|e| Error::from(e))
    }

    fn client(&self) -> &Client;
    fn base(&self) -> &Url;
}

pub struct GitLabAPI {
    client: Client,
    _base_url: Url,
}

impl GitLabAPI {
    pub fn new(token: &str, base_url: Url) -> GitLabAPI {
        let mut default_header = HeaderMap::new();
        default_header.insert("Private-Token", HeaderValue::from_str(token).expect("Token malformed"));
        GitLabAPI {
            client: ClientBuilder::new()
                .default_headers(default_header)
                .build().expect("What?"),
            _base_url: base_url,
        }
    }

    pub fn lookup_user_id(&self, username: &str) -> GMResult<u64> {
        let mut res = self.execute(Method::GET, &format!["users?username={}", username], "", None)?
            .error_for_status()?;
        let body = res.text()?;

        if body.is_empty() {
            Err(Error::new("Not found"))
        } else {
            Ok(serde_json::from_str::<serde_json::Value>(&body)?[0]["id"].as_u64().expect("Schema changed."))
        }
    }

    pub fn remove_keys(&self, id: u64) -> GMResult<()> {
        let keys: JsonValue = self.call_no_body(Method::GET, &format!("users/{}/keys", id))?.json()?;

        for val in keys.as_array().iter().map(|k| k.iter()).flatten() {
            self.call_no_body(Method::DELETE, &format!("users/{}/keys/{}", id, val["id"].as_u64().expect("Gitlab schema changed.")))?;
        }

        Ok(())
    }
}

impl APIAccessor for GitLabAPI {
    #[inline]
    fn client(&self) -> &Client {
        &self.client
    }

    #[inline]
    fn base(&self) -> &Url {
        &self._base_url
    }
}

pub struct BackendAPI {
    client: Client,
    _base_url: Url,
}

impl BackendAPI {
    pub fn new(base_url: Url) -> BackendAPI {
        BackendAPI {
            client: ClientBuilder::new()
                .build().expect("What?"),
            _base_url: base_url,
        }
    }
}

impl APIAccessor for BackendAPI {
    fn client(&self) -> &Client {
        &self.client
    }

    fn base(&self) -> &Url {
        &self._base_url
    }
}

fn is_ip_same(lhs: &IpAddr, rhs: &IpAddr) -> bool {
    match lhs {
        IpAddr::V4(lhs4) => match rhs {
            IpAddr::V4(rhs4) => lhs4.octets() == rhs4.octets(),
            IpAddr::V6(_) => false
        }
        IpAddr::V6(lhs6) => match rhs {
            IpAddr::V4(_) => false,
            IpAddr::V6(rhs6) => lhs6.octets() == rhs6.octets()
        }
    }
}

macro_rules! gitlab_event {
    ($clz: tt, $name: expr) => {
        pub struct $clz();

        impl<'a, 'r> FromRequest<'a, 'r> for $clz {
            type Error = Error;

            fn from_request(request: &'a Request<'r>) -> request::Outcome<Self, Self::Error> {
                $clz::from_request0(request).map_failure(|(s,f)|(s,Error::new(f)))
            }
        }

        impl<'a,'r> $clz {
            fn from_request0(request: &'a Request<'r>) -> request::Outcome<Self, &'static str> {
                // Rocket's implementation of guard isn't quite friendly...
                if let Outcome::Success(s) = request.guard::<State<Domain>>() {
                    if let Some(ref domains) = s.0 {
                        if let Some(ip) = request.client_ip() {
                            if !domains.iter().any(|d| is_ip_same(d, &ip)) {
                                return Outcome::Failure((Status::Forbidden, "IP not whitelisted"))
                            }
                        } else {
                            return Outcome::Failure((Status::Forbidden, "IP not whitelisted"))
                        }
                    }
                }
                if let Outcome::Success(s) = request.guard::<State<TokenSalt>>() {
                    let token = calc_token(request.uri().path(), &*s);
                    if !request.headers().get("x-gitlab-token").any(|t| t==token) {
                        return Outcome::Failure((Status::Forbidden, "Require valid token"))
                    }
                }
                let name: Vec<_> = request.headers().get("x-gitlab-event").collect();
                if name.len() != 1 {
                    return Outcome::Failure((Status::BadRequest, stringify!(No gitlab $name)))
                }
                if name[0] != $name {
                    return Outcome::Failure((Status::BadRequest, stringify!(Not gitlab $name)))
                }
                return Outcome::Success($clz());
            }
        }
    };
}

gitlab_event!(Push, "Push Hook");

pub struct TokenSalt(pub String);

impl ::Deref for TokenSalt {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct Domain(Option<Vec<IpAddr>>);

impl Domain {
    pub fn new(domain: Option<Vec<IpAddr>>) -> Domain {
        Domain(domain)
    }
}

pub fn calc_token(path: &str, token_salt: &str) -> String {
    let mut first = encode(Sha512::digest(path.as_bytes()).as_slice());
    first.push_str(token_salt);
    encode(first)
}