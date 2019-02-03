/*
 * Copyright (c)  2019,  llk89.
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
use serde_json::Error as JSONError;
use std::str::Utf8Error;
use uuid::parser::ParseError;

#[derive(Debug)]
pub enum Error {
    MySQLError(MySQLError),
    HTTPError(HTTPError),
    JSONError(JSONError),
    Utf8Error(Utf8Error),
    UuidError(ParseError),
    SomeError(&'static str),
}

pub type GMResult<T> = Result<T, Error>;

impl Error {
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

impl From<Utf8Error> for Error {
    fn from(exception: Utf8Error) -> Self {
        Error::Utf8Error(exception)
    }
}

impl From<ParseError> for Error {
    fn from(exception: ParseError) -> Self {
        Error::UuidError(exception)
    }
}