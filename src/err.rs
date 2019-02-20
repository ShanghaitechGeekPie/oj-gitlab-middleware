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


use mysql::Error as MySQLError;
use reqwest::Error as HTTPError;
use serde_json::error::Error as JSONError;

use rocket::{Request, Response};
use rocket::http::Status;
use rocket::response::Responder;
use std::io::Cursor;

#[derive(Debug)]
pub enum Error {
    MySQLError(MySQLError),
    HTTPError(HTTPError),
    JSONError(JSONError),
    AlreadyExists,
    NotFound,
    UpstreamError(u16, String),
    SomeError(&'static str),
}

pub type GMResult<T> = Result<T, Error>;

impl Error {
    pub fn upstream(code: u16, reason: String) -> Error {
        Error::UpstreamError(code, reason)
    }
    pub fn new(reason: &'static str) -> Error {
        Error::SomeError(reason)
    }
}

impl From<MySQLError> for Error {
    fn from(exception: MySQLError) -> Self {
        Error::MySQLError(exception)
    }
}

impl From<JSONError> for Error {
    fn from(exception: JSONError) -> Self {
        Error::JSONError(exception)
    }
}

impl From<HTTPError> for Error {
    fn from(exception: HTTPError) -> Self {
        Error::HTTPError(exception)
    }
}

impl<'r> Responder<'r> for Error {
    fn respond_to(self, _: &Request) -> Result<Response<'r>, Status> {
        warn!("Caught error: {:?}", self);
        match self {
            Error::AlreadyExists => Err(Status::Conflict),
            Error::NotFound => Err(Status::NotFound),
            Error::UpstreamError(code, message) => Ok(Response::build()
                .status(Status::from_code(code).unwrap_or(Status::InternalServerError))
                .sized_body(Cursor::new(message))
                .finalize()),
            _ => Err(Status::InternalServerError)
        }
    }
}