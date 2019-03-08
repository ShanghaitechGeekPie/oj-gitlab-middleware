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

#![feature(proc_macro_hygiene, decl_macro, result_map_or_else, ip)]

#[macro_use]
extern crate log;
extern crate log4rs;
extern crate reqwest;
#[macro_use]
extern crate rocket;
#[macro_use]
extern crate rocket_contrib;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate url;
extern crate percent_encoding;
extern crate hex;
extern crate sha2;
extern crate uuid;

use std::borrow::{Borrow, Cow};
use std::net::ToSocketAddrs;
use std::io::Cursor;
use std::str::Utf8Error;
use std::ops::Deref;

use reqwest::Method;
use reqwest::header::HeaderValue;

use rocket::State;
use rocket::fairing::AdHoc;
use rocket::http::{ContentType, Header, RawStr, Status};
use rocket::request::{FromParam, FromFormValue};
use rocket::response::Response;

use rocket_contrib::databases::mysql;
use rocket_contrib::json::{Json, JsonValue};

use serde_json::Value;

use mysql::uuid::Uuid as UuidRaw;

use url::Url;

mod apis;
mod err;

use apis::*;
use err::*;
use log4rs::config::Config as LogConfig;
use log::Log;
use log4rs::config::Root;

struct Uuid<'a> {
    parsed: UuidRaw,
    original: Cow<'a, str>,
}

impl<'a> FromParam<'a> for Uuid<'a> {
    type Error = Error;

    fn from_param(param: &'a RawStr) -> Result<Self, Self::Error> {
        let decoded = param.percent_decode().map_err(|_| Error::NotFound)?;
        let parsed = UuidRaw::parse_str(&decoded).map_err(|_| Error::NotFound)?;
        Ok(Uuid { parsed, original: decoded })
    }
}

#[derive(Serialize)]
struct ForwardedWebHookRequest<'a> {
    course_uid: &'a str,
    assignment_uid: &'a str,
    upstream: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    additional_data: Option<String>,
}

impl<'a> APIFunction for ForwardedWebHookRequest<'a> {
    fn path(&self) -> Cow<str> {
        Cow::Borrowed(".")
    }
}

#[post("/hooks/<course>/<assignment>?<data>", data = "<message>")]
fn webhook(course: Uuid, assignment: Uuid, message: Json<JsonValue>, data: Option<String>,
           _event: Push,
           backend: State<BackendAPI>)
           -> GMResult<()> {
    let upstream = message["project"]["git_ssh_url"].as_str().expect("Schema changed");
    let request = ForwardedWebHookRequest { course_uid: &course.original, assignment_uid: &assignment.original, upstream, additional_data: data };
    backend.call(&request)?.error_for_status()?;
    Ok(())
}

#[derive(Deserialize)]
struct CreateUser<'a> {
    email: &'a str,
    password: &'a str,
}

#[derive(Serialize)]
struct CreateUserGitLab<'a> {
    email: &'a str,
    username: &'a str,
    password: &'a str,
    name: &'a str,
}

impl<'a> CreateUserGitLab<'a> {
    fn from(inbound: &'a CreateUser<'a>) -> Result<Self, ()> {
        if let Some(at_pos) = inbound.email.find('@') {
            if &inbound.email[0..at_pos] == "admin" {
                Err(())
            } else {
                Ok(CreateUserGitLab {
                    email: inbound.email,
                    username: &inbound.email[0..at_pos],
                    password: inbound.password,
                    name: &inbound.email[0..at_pos],
                })
            }
        } else {
            Err(())
        }
    }
}

impl<'a> APIFunction for CreateUserGitLab<'a> {
    fn path(&self) -> Cow<str> {
        Cow::Borrowed("users")
    }
}

#[post("/users", data = "<user>")]
fn create_user<'a>(user: Json<CreateUser>,
                   mut db: DBAccess, gitlab_api: State<'a, GitLabAPI>)
                   -> GMResult<Response<'a>> {
    let mut builder = Response::build();
    if user.password.len() < 8 {
        return Ok(builder.status(Status::BadRequest)
            .header(ContentType::JSON)
            .sized_body(Cursor::new(r#"{"cause":"Password too short (len<8)"}"#))
            .finalize());
    }
    if let Ok(outbound) = CreateUserGitLab::from(&*user) {
        let response: Value = gitlab_api.call(&outbound)?.json()?;
        db.remember_uid(&user.email, response["id"].as_u64().expect("Gitlab schema changed"))?;
    } else {
        return Ok(builder.status(Status::BadRequest)
            .header(ContentType::JSON)
            .sized_body(Cursor::new(r#"{"cause":"Invalid email"}"#))
            .finalize());
    }
    Ok(builder.status(Status::Created).finalize())
}

#[derive(Deserialize)]
struct UpdateKey {
    key: String,
}

#[derive(Serialize)]
struct AddKeyGitlab<'a> {
    #[serde(skip)]
    id: u64,
    title: &'static str,
    // we are only giving one PK, so use default
    key: &'a str,
}

impl<'a> AddKeyGitlab<'a> {
    fn new(id: u64, key: &'a str) -> Self {
        AddKeyGitlab { id, title: "key", key }
    }
}

impl<'a> APIFunction for AddKeyGitlab<'a> {
    fn path(&self) -> Cow<str> {
        Cow::Owned(format!("users/{}/keys", self.id))
    }
}

struct StrInUri<'a>(Cow<'a, str>);

impl<'a> Deref for StrInUri<'a> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}

impl<'a> Borrow<str> for StrInUri<'a> {
    fn borrow(&self) -> &str {
        &*self
    }
}

impl<'a> FromParam<'a> for StrInUri<'a> {
    type Error = Utf8Error;

    fn from_param(param: &'a RawStr) -> Result<Self, Self::Error> {
        param.percent_decode().map(StrInUri)
    }
}

impl<'a> FromFormValue<'a> for StrInUri<'a> {
    type Error = Utf8Error;

    fn from_form_value(form_value: &'a RawStr) -> Result<Self, Self::Error> {
        form_value.percent_decode().map(StrInUri)
    }
}

#[post("/users/<user_email>/key", data = "<message>")]
fn update_key(user_email: StrInUri, message: Json<UpdateKey>,
              mut db: DBAccess, gitlab_api: State<GitLabAPI>)
              -> GMResult<Status> {
    let id = db.translate_uid(&user_email)?;
    gitlab_api.remove_keys(id)?;
    gitlab_api.call(&AddKeyGitlab::new(id, &message.key))?;

    Ok(Status::Ok)
}

#[derive(Deserialize)]
struct CreateGroup<'a> {
    name: &'a str,
    uuid: UuidRaw,
}

#[derive(Serialize)]
struct CreateGroupGitlab<'a> {
    name: &'a str,
    path: &'a str,
    visibility: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_id: Option<u64>,
}

impl<'a> From<&'a CreateGroup<'a>> for CreateGroupGitlab<'a> {
    fn from(inbound: &'a CreateGroup) -> Self {
        CreateGroupGitlab {
            name: inbound.name,
            path: inbound.name,
            visibility: "private",
            parent_id: None,
        }
    }
}

impl<'a> APIFunction for CreateGroupGitlab<'a> {
    fn path(&self) -> Cow<str> {
        Cow::Borrowed("groups")
    }
}

#[post("/courses", data = "<message>")]
fn create_course(message: Json<CreateGroup>,
                 mut db: DBAccess, gitlab_api: State<GitLabAPI>)
                 -> GMResult<Status> {
    let r: Value = gitlab_api.call(&CreateGroupGitlab::from(&*message))?.json()?;
    db.remember_uuid(&message.uuid, r["id"].as_u64().expect("Gitlab schema changed")).map(|_| Status::Created)
}

#[delete("/courses/<course_uid>")]
fn delete_course(course_uid: Uuid,
                 mut db: DBAccess, gitlab_api: State<GitLabAPI>) -> GMResult<()> {
    let course_id = db.translate_uuid(&course_uid.parsed)?;

    let res: JsonValue = gitlab_api.call_no_body(Method::GET, &format!("groups/{}/subgroups", course_id))?.json()?;

    for assignment in res.as_array().expect("Gitlab schema changed") {
        db.forget_uuid_by_id(assignment["id"].as_u64().expect("Gitlab schema changed"))?;
    }

    gitlab_api.call_no_body(Method::DELETE, &format!("groups/{}", course_id))?;

    db.forget_uuid_by_id(course_id)?;

    Ok(())
}

#[derive(Deserialize)]
struct CreateAssignment<'a> {
    name: &'a str,
    uuid: UuidRaw,
}

impl<'a> CreateGroupGitlab<'a> {
    fn assignment(inbound: &'a CreateAssignment<'a>, parent: u64) -> Self {
        CreateGroupGitlab {
            name: inbound.name,
            path: inbound.name,
            visibility: "private",
            parent_id: Some(parent),
        }
    }
}

#[post("/courses/<parent_uid>/assignments", data = "<message>")]
fn create_assignment(parent_uid: Uuid, message: Json<CreateAssignment>,
                     mut db: DBAccess, gitlab_api: State<GitLabAPI>)
                     -> GMResult<Status> {
    let parent_id = db.translate_uuid(&parent_uid.parsed)?;
    let response: Value = gitlab_api.call(&CreateGroupGitlab::assignment(&*message, parent_id))?.json()?;
    db.remember_uuid(&message.uuid, response["id"].as_u64().expect("Gitlab schema changed"))
        .map(|_| Status::Created)
}

#[delete("/courses/<_course_uid>/assignments/<assignment_uid>")]
fn delete_assignment(_course_uid: Uuid, assignment_uid: Uuid,
                     mut db: DBAccess, gitlab_api: State<GitLabAPI>) -> GMResult<()> {
    let assignment_id = db.translate_uuid(&assignment_uid.parsed)?;

    gitlab_api.call_no_body(Method::DELETE, &format!("groups/{}", assignment_id))?;

    db.forget_uuid_by_id(assignment_id)?;

    Ok(())
}

#[derive(Deserialize)]
struct AddInstructorToCourse<'a> {
    instructor_name: &'a str
}

#[derive(Serialize)]
struct AddUserToGroupGitlab {
    #[serde(skip)]
    group_id: u64,
    user_id: u64,
    access_level: u8,
}

impl AddUserToGroupGitlab {
    fn new(user_id: u64, course: u64, access_level: u8) -> Self {
        AddUserToGroupGitlab { group_id: course, user_id, access_level }
    }
}

impl APIFunction for AddUserToGroupGitlab {
    fn path(&self) -> Cow<str> {
        Cow::Owned(format!("groups/{}/members", self.group_id))
    }
}

#[post("/courses/<course_uuid>/instructors", data = "<message>")]
fn add_instructor_to_course<'r>(course_uuid: Uuid, message: Json<AddInstructorToCourse>,
                                mut db: DBAccess, gitlab_api: State<'r, GitLabAPI>)
                                -> GMResult<()> {
    let course_id = db.translate_uuid(&course_uuid.parsed)?;
    let user_id = db.translate_uid(message.instructor_name)?;
    gitlab_api.call(&AddUserToGroupGitlab::new(user_id, course_id, 50))?;
    Ok(())
}

#[derive(Deserialize)]
struct CreateRepo<'a> {
    owners: Vec<&'a str>,
    repo_name: &'a str,
    ddl: &'a str,
    #[serde(default)]
    additional_data: Option<Cow<'a, str>>,
}

#[derive(Serialize, Clone, Copy, Debug, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Visibility { Public, Internal, Private }

#[derive(Serialize)]
struct CreateRepoGitlab<'a> {
    name: &'a str,
    namespace_id: u64,
    visibility: Visibility,
}

impl<'a> CreateRepoGitlab<'a> {
    fn new(name: &'a str, assignment_id: u64) -> Self {
        CreateRepoGitlab { name, namespace_id: assignment_id, visibility: Visibility::Private }
    }
}

impl<'a> APIFunction for CreateRepoGitlab<'a> {
    fn path(&self) -> Cow<str> {
        Cow::Borrowed("projects/")
    }
}

#[derive(Serialize)]
struct CreateWebhookGitlab<'a> {
    #[serde(skip)]
    project_id: u64,
    url: &'a str,
    push_events: bool,
    token: &'a str,
}

impl<'a> CreateWebhookGitlab<'a> {
    fn new(project_id: u64, url: &'a str, token: &'a str) -> Self {
        CreateWebhookGitlab { project_id, url, push_events: true, token }
    }
}

impl<'a> APIFunction for CreateWebhookGitlab<'a> {
    fn path(&self) -> Cow<str> {
        Cow::Owned(format!("projects/{}/hooks", self.project_id))
    }
}

#[derive(Serialize)]
struct AddUserToProjectGitlab<'a> {
    #[serde(skip)]
    project_id: u64,
    user_id: u64,
    access_level: u8,
    expires_at: &'a str,
}

impl<'a> AddUserToProjectGitlab<'a> {
    fn new(project_id: u64, user_id: u64, expires_at: &'a str) -> Self {
        // 40 is maintainer access, so users can push
        AddUserToProjectGitlab { project_id, user_id, access_level: 40, expires_at }
    }
}

impl<'a> APIFunction for AddUserToProjectGitlab<'a> {
    fn path(&self) -> Cow<str> {
        Cow::Owned(format!("projects/{}/members", self.project_id))
    }
}

#[post("/courses/<course_uid>/assignments/<assignment_uid>/repos", data = "<message>")]
fn create_repo(course_uid: Uuid, assignment_uid: Uuid, message: Json<CreateRepo>,
               token_salt: State<TokenSalt>, middleware_base: State<MiddlewareBase>,
               mut db: DBAccess, gitlab_api: State<GitLabAPI>)
               -> GMResult<String> {
    if db.translate_repo_id(&course_uid.parsed, &assignment_uid.parsed, &message.repo_name).is_ok() {
        return Err(Error::AlreadyExists);
    }
    let assignment_id = db.translate_uuid(&assignment_uid.parsed)?;
    let owners: Vec<u64> = {
        let mut ret: Vec<u64> = Vec::with_capacity(message.owners.len());

        for owner in &message.owners {
            ret.push(db.translate_uid(owner)?);
        }

        ret
    };

    // create repo
    let response: Value = gitlab_api.call(&CreateRepoGitlab::new(message.repo_name, assignment_id))?.json()?;
    let repo_id = response["id"].as_u64().expect("Gitlab schema changed");
    let repo_url = response["ssh_url_to_repo"].as_str().expect("Gitlab schema changed");
    db.remember_repo_id(&course_uid.parsed, &assignment_uid.parsed, message.repo_name, repo_id)?;
    // setup webhook
    let mut webhook = if let Some(d) = &message.additional_data {
        let data = ::percent_encoding::percent_encode(d.as_bytes(), percent_encoding::USERINFO_ENCODE_SET);
        format!("/hooks/{}/{}?data={}", &course_uid.original, &assignment_uid.original, data)
    } else {
        format!("/hooks/{}/{}", &course_uid.original, &assignment_uid.original)
    };
    let token = calc_token(&webhook, &*token_salt);
    webhook.insert_str(0, &middleware_base.0);
    gitlab_api.call(&CreateWebhookGitlab::new(repo_id, &webhook, &token))?;
    // set all branches as protected branch to prevent force push
    gitlab_api.call_no_body(Method::POST, &format!("projects/{}/protected_branches?name=*", repo_id))?;
    // setup student permission
    for owner in owners {
        gitlab_api.call(&AddUserToProjectGitlab::new(repo_id, owner, message.ddl))?;
    }
    Ok(format!(r#"{{"ssh_url_to_repo":"{}"}}"#, repo_url))
}

#[delete("/courses/<course_uid>/assignments/<assignment_uid>/repos/<repo_name>")]
fn delete_repo(course_uid: Uuid, assignment_uid: Uuid, repo_name: StrInUri,
               mut db: DBAccess, gitlab_api: State<GitLabAPI>) -> GMResult<()> {
    let repo_id = db.translate_repo_id(&course_uid.parsed, &assignment_uid.parsed, &repo_name)?;

    gitlab_api.call_no_body(Method::DELETE, &format!("projects/{}", repo_id))?;

    db.forget_repo_id(repo_id)?;

    Ok(())
}

struct DownloadFormat<'a>(Cow<'a, str>);

impl<'a> Deref for DownloadFormat<'a> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}

impl<'a> FromFormValue<'a> for DownloadFormat<'a> {
    type Error = Utf8Error;

    fn from_form_value(form_value: &'a RawStr) -> Result<Self, Self::Error> {
        form_value.percent_decode().map(DownloadFormat)
    }

    fn default() -> Option<Self> {
        Some(DownloadFormat(Cow::Borrowed("tar.gz")))
    }
}

#[get("/courses/<course_uid>/assignments/<assignment_uid>/repos/<repo_name>/download?<format>")]
fn download_repo<'r>(course_uid: Uuid, assignment_uid: Uuid, repo_name: StrInUri, format: DownloadFormat,
                     mut db: DBAccess, gitlab_api: State<'r, GitLabAPI>)
                     -> GMResult<Response<'r>> {
    let repo_id = db.translate_repo_id(&course_uid.parsed, &assignment_uid.parsed, &repo_name)?;
    let response = gitlab_api.call_no_body(Method::GET, &format!("projects/{}/repository/archive.{}", repo_id, &*format))?;
    let mut ret = Response::build();
    {
        if let Some(Ok(desposition)) = response.headers().get("Content-Disposition").map(HeaderValue::to_str) {
            ret.header(Header::new("Content-Disposition", desposition.to_string()));
        }
        if let Some(Ok(etag)) = response.headers().get("Etag").map(HeaderValue::to_str) {
            ret.header(Header::new("Etag", etag.to_string()));
        }
    }
    ret.header(Header::new("Content-Transfer-Encoding", "binary"));
    ret.header(ContentType::Binary);
    ret.streamed_body(response);
    Ok(ret.finalize())
}

#[get("/courses/<course_uid>/assignments/<assignment_uid>/repos/<repo_name>/commits?<page>")]
fn commits<'r>(course_uid: Uuid, assignment_uid: Uuid, repo_name: StrInUri, page: Option<StrInUri>,
               mut db: DBAccess, gitlab_api: State<'r, GitLabAPI>)
               -> GMResult<Response<'r>> {
    let repo_id = db.translate_repo_id(&course_uid.parsed, &assignment_uid.parsed, &repo_name)?;
    let response = if let Some(next_page) = page {
        gitlab_api.call_no_body(Method::GET, &next_page)
    } else {
        gitlab_api.call_no_body(Method::GET, &format!("projects/{}/repository/commits?per_page=100", repo_id))
    }?;
    let mut ret = Response::build();
    {
        if let Some(Ok(link)) = response.headers().get("Link").map(HeaderValue::to_str) {
            if let Some(next) = link.split(",").find(|s| s.trim().ends_with(r#"rel="next"#)) {
                let gitlab_link: &str = &next[next.find("<").expect("gitlab schema changed") + 1..next.find(">").expect("gitlab schema changed")];
                let encoded = percent_encoding::percent_encode(gitlab_link.as_bytes(), percent_encoding::QUERY_ENCODE_SET);
                ret.header(Header::new("Link", format!(r#"<commits?page={}>; rel="next""#, encoded)));// replace url TODO
            }
        }
    }
    ret.header(ContentType::JSON);
    ret.streamed_body(response);
    Ok(ret.finalize())
}

//================================================================================
#[get("/healthcheck")]
fn healthcheck(mut db: DBAccess, gitlab_api: State<GitLabAPI>/*, backend: State<BackendAPI>*/) -> Response {
//fn healthcheck<'r>(mut db: DBAccess, gitlab_api: State<'r, GitLabAPI>, backend: State<BackendAPI>) -> Response<'r> {
    if !db.0.ping() {
        Response::build().status(Status::InternalServerError).sized_body(Cursor::new("db offline")).finalize()
    } else if gitlab_api.call_no_body(Method::GET, "../../-/health").is_err() {
        Response::build().status(Status::InternalServerError).sized_body(Cursor::new("gitlab offline")).finalize()
    } else {
        Response::build().sized_body(Cursor::new("OK")).finalize()
    }
}

//================================================================================
#[database("mysql")]
struct DBAccess(mysql::Conn);

impl DBAccess {
    fn translate_uid(&mut self, username: &str) -> GMResult<u64> {
        self.0.first_exec(r"SELECT uid FROM uid WHERE username=?", (&*username, ))
            ?.ok_or(Error::NotFound)
    }

    fn remember_uid(&mut self, username: &str, id: u64) -> GMResult<()> {
        self.0.prep_exec(r"INSERT INTO uid(uid, username) VALUES (?, ?)", (id, username))?;

        Ok(())
    }

    fn translate_uuid(&mut self, uuid: &UuidRaw) -> GMResult<u64> {
        self.0.first_exec(r"SELECT gitlab_id FROM uuids WHERE uuid=?", (uuid, ))
            ?.ok_or(Error::NotFound)
    }

    fn remember_uuid(&mut self, uuid: &UuidRaw, id: u64) -> GMResult<()> {
        self.0.prep_exec(r"INSERT INTO uuids(gitlab_id, uuid) VALUES (?, ?)", (id, uuid))?;

        Ok(())
    }

    fn forget_uuid_by_id(&mut self, id: u64) -> GMResult<()> {
        self.0.prep_exec(r"DELETE FROM uuids WHERE gitlab_id=?", (id, ))?;

        Ok(())
    }

    fn translate_repo_id(&mut self, course_uid: &UuidRaw, assignment_uid: &UuidRaw, name: &str) -> GMResult<u64> {
        self.0.first_exec(r"SELECT repo_id FROM repo_ids WHERE course_uid=? AND assignment_uid=? AND name=?", (course_uid, assignment_uid, name))
            ?.ok_or(Error::NotFound)
    }

    fn remember_repo_id(&mut self, course_uid: &UuidRaw, assignment_uid: &UuidRaw, name: &str, id: u64) -> GMResult<()> {
        self.0.prep_exec(r"INSERT INTO repo_ids(repo_id, course_uid, assignment_uid, name) VALUES (?, ?, ?, ?)", (id, course_uid, assignment_uid, name))?;

        Ok(())
    }

    fn forget_repo_id(&mut self, id: u64) -> GMResult<()> {
        self.0.prep_exec(r"DELETE FROM repo_ids WHERE repo_id=?", (id, ))?;

        Ok(())
    }
}

struct MiddlewareBase(String);

fn main() {
    log4rs::init_file("log4rs.yml", Default::default()).unwrap();

    rocket::ignite()
        .attach(DBAccess::fairing())
        .attach(AdHoc::on_attach("BackendAPI", |r| {
            let c = r.config().get_string("backend_url").expect("backend_url not set");
            let url = Url::options().parse(&c).expect("backend_url invalid");
            match url.scheme() {
                "http" | "https" => {}
                "" => panic!("backend_url scheme not specified"),
                _ => panic!("backend_url scheme not supported (only http/https)"),
            }
            let c = r.config().get_string("backend_auth_header").expect("backend_auth_header not set");
            let backend = BackendAPI::new(url, &c);
            Ok(r.manage(backend))
        }))
        .attach(AdHoc::on_attach("GitlabAPI", |r| {
            let api = {
                let base_url_str = r.config().get_str("gitlab_base_url").expect("gitlab_base_url not set");
                let base_url = Url::options().parse(base_url_str).expect("Please properly set gitlab_base_url");
                let token = r.config().get_str("gitlab_auth_token").expect("gitlab_auth_token not set");
                GitLabAPI::new(token, base_url)
            };
            Ok(r.manage(api))
        }))
        .attach(AdHoc::on_attach("TokenSaltRetriever", |r| {
            let token = r.config().get_string("gitlab_webhook_token_salt").unwrap_or("CAFEDEAD".to_string());
            Ok(r.manage(TokenSalt(token)))
        }))
        .attach(AdHoc::on_attach("MiddlewareBaseRetriever", |r| {
            let mut token: String = r.config().get_string("middleware_base").unwrap_or(String::new());
            if let Some('/') = token.chars().last() { token.pop(); }
            Ok(if !token.is_empty() {
                r.manage(MiddlewareBase(token))
            } else {
                panic!("middleware_base not set")
            })
        }))
        .attach(AdHoc::on_attach("GitlabDomainRetriever", |r| {
            // Add IP whitelist, if present
            let domains = r.config().get_string("gitlab_domain")
                .map_or_else(|_| Ok(Vec::new()), |dn| {
                    dn.to_socket_addrs().map(|addrs| addrs.map(|sa| sa.ip()).collect())
                })
                .unwrap_or(Vec::new());
            Ok(if !domains.is_empty() {
                r.manage(Domain::new(Some(domains)))
            } else {
                r.manage(Domain::new(None))
            })
        }))
        .mount("/", routes![
            webhook,create_user,update_key,create_course,create_assignment,
            add_instructor_to_course,create_repo,download_repo,healthcheck,commits,
            delete_course, delete_assignment, delete_repo
        ])
        .launch();
}
