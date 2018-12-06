/*
 Copyright (c) 2018 llk89.

 This program is free software: you can redistribute it and/or modify
 it under the terms of the GNU Affero General Public License as
 published by the Free Software Foundation, either version 3 of the
 License, or (at your option) any later version.

 This program is distributed in the hope that it will be useful,
 but WITHOUT ANY WARRANTY; without even the implied warranty of
 MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 GNU Affero General Public License for more details.

 You should have received a copy of the GNU Affero General Public License
 along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

#![feature(proc_macro_hygiene, decl_macro, result_map_or_else)]

#[macro_use]
extern crate lazy_static;
extern crate regex;

extern crate redis;

#[macro_use]
extern crate rocket;
#[macro_use]
extern crate rocket_contrib;

use std::io::Read;
use std::net::{IpAddr, ToSocketAddrs};
use std::time::{SystemTime, UNIX_EPOCH};

use regex::Regex;

use rocket_contrib::databases::redis::Commands;

use rocket::{State, Request, Data, Outcome};
use rocket::data::{self, FromDataSimple};
use rocket::http::{Status, ContentType};
use rocket::request::{self, FromRequest};

struct Upstream(String);

impl FromDataSimple for Upstream {
    type Error = ();

    fn from_data(req: &Request, data: Data) -> data::Outcome<Self, ()> {
        lazy_static! {
            static ref UPSTREAM: Regex = Regex::new(r##""git_ssh_url" *: *"(.*?)""##).unwrap();
        }
        if req.content_type().is_some() && req.content_type().unwrap() != &ContentType::JSON {
            return Outcome::Forward(data);
        }
        let mut s = String::with_capacity(4096);
        if let Err(_) = data.open().take(1024 * 1024 * 1024).read_to_string(&mut s) {
            return Outcome::Failure((Status::PayloadTooLarge, ()));
        }
        let upstream = if let Some(upstream) = UPSTREAM.captures_iter(&s).next() {
            upstream[1].to_string()
        } else {
            return Outcome::Failure((Status::UnprocessableEntity, ()));
        };
        return Outcome::Success(Upstream(upstream));
    }
}

macro_rules! gitlab_event {
    ($clz: tt, $name: expr) => {
        struct $clz();

        impl<'a, 'r> FromRequest<'a, 'r> for $clz {
            type Error = &'r str;

            fn from_request(request: &'a Request<'r>) -> request::Outcome<Self, Self::Error> {
                if let Outcome::Success(s) = request.guard::<State<Domain>>() {
                    if let Some(ref domains ) = s.0 {
                        if let Some(ip) = request.client_ip() {
                            if !domains.iter().any(|d| is_ip_same(d, &ip)) {
                                return Outcome::Failure((Status::Unauthorized, "IP not whitelisted"))
                            }
                        } else {
                            return Outcome::Failure((Status::Unauthorized, "IP not whitelisted"))
                        }
                    }
                }
                if let Outcome::Success(s) = request.guard::<State<Token>>() {
                    if let Some(ref token )= s.0 {
                        if !request.headers().get("x-gitlab-token").any(|t| t==token) {
                            return Outcome::Failure((Status::Unauthorized, "Require valid token"))
                        }
                    }
                }
                let name: Vec<_> = request.headers().get("x-gitlab-event").collect();
                if name.len() != 1 {
                    //println!(stringify!(No gitlab $name));
                    return Outcome::Failure((Status::BadRequest, stringify!(No gitlab $name)))
                }
                if name[0] != $name {
                    //println!(stringify!(Not gitlab $name));
                    return Outcome::Failure((Status::BadRequest, stringify!(Not gitlab $name)))
                }
                return Outcome::Success($clz());
            }
        }
    };
}

//fn from_request(request: &Request) -> request::Outcome<Self, Self::Error> {
//    if let Outcome::Success(s) = request.guard::<State<Domain>>() {
//        let domains = (s.0).0.to_vec();
//        if let Some(&ip) = request.client_ip() {
//            if !domains.iter().any(|d| is_ip_same(d, ip)) {
//                return Outcome::Failure((Status::Unauthorized, "IP not whitelisted"))
//            }
//        } else {
//            return Outcome::Failure((Status::Unauthorized, "IP not whitelisted"))
//        }
//    }
//    if let Outcome::Success(s) = request.guard::<State<Token>>() {
//        let token = (s.0).0;
//        if !request.headers().get("x-gitlab-token").any(token) {
//            return Outcome::Failure((Status::Unauthorized, "Require valid token"))
//        }
//    }
//    return Outcome::Success("");
//}
gitlab_event!(Push, "Push Hook");

#[database("redis")]
struct QueueRedis(redis::Connection);

fn current_time_millis() -> u64 {
    let d = SystemTime::now().duration_since(UNIX_EPOCH).expect("Time travel");
    d.as_secs() * 1000 + d.subsec_millis() as u64
}

#[post("/hooks/<course>/<assignment>", data = "<message>")]
fn handle(course: u32, assignment: u32, redis: QueueRedis, message: Upstream, _event: Push) -> Status {
    if let Err(_) = redis.zadd::<String, u64, &str, u8>(format!("{}:{}", course, assignment), &message.0, current_time_millis()) {
        return Status::InternalServerError;
    };
    return Status::Ok;
}

struct Token(Option<String>);

struct Domain(Option<Vec<IpAddr>>);

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

fn main() {
    let mut rocket = rocket::ignite()
        .attach(QueueRedis::fairing())
        .mount("/", routes![handle]);

    let mut security: u8 = 0;

    if rocket.config().get_bool("mute_security").unwrap_or(false) { security += 1; }

    // Add token, if present
    let token = rocket.config().get_string("gitlab_token").unwrap_or(String::new());
    if !token.is_empty() {
        rocket = rocket.manage(Token(Some(token)));
        security += 1;
    } else {
        rocket = rocket.manage(Token(None));
    }

    // Add IP whitelist, if present
    let domains = rocket.config().get_string("gitlab_domain")
        .map_or_else(|_| Ok(Vec::new()), |dn| {
            dn.to_socket_addrs().map(|addrs| addrs.map(|sa| sa.ip()).collect())
        })
        .unwrap_or(Vec::new());
    if !domains.is_empty() {
        rocket = rocket.manage(Domain(Some(domains)));
        security += 1;
    } else {
        rocket = rocket.manage(Domain(None));
    }

    if security == 0 {
        panic!("Alert! You have no security measures enabled! Either supply gitlab_token or gitlab_domain, or set mute_security to true if you hosts on loopback interface.")
    }

    rocket.launch();
}
