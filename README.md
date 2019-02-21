# gitlab-middleware

# Overview

This middleware acts as an abstraction layer between backend and an actual git server implementation.
Currently the only supported git server is gitlab, hence the name gitlab-middleware.

[Rust](https://rust-lang.org/) and [Rocket](https://rocket.rs/) are used because, why not?

# Licensing

Any file in this repository not owned by another party is licensed to the public under AGPL3.0 unless explicitly
stated otherwise. 

# Building & Deploying

This service is expected to be built and deployed with docker. This service requires you to supply a environment variable
named `TOKEN_MAGIC_SALT` which could be an arbitrary string of 3~8 characters long. Longer may have *negligible* security 
improvements for *negligible* performance penalty. Leaving it empty will not 

# Configuring

This is a Rocket application, so visit its [document](https://rocket.rs/v0.4/guide/configuration/#environment-variables)
to know how to configure via environment variables.

Here is an exhaustive list of configuration that this middleware accepts. Unknowns will be ignored. TODO really exhaustive.

config name|description|required
---|---|---
`backend_url`|A middleware visible url pointing towards the backend.|true
`backend_auth_header`|The value of header `Authorization` that will be sent to backend|true
`middleware_base`|A gitlab visible url pointing towards the middleware. This must not be something like `http://middleware:8000`. It must be  `http://middleware.localnetwork:8000` or something.|true
`gitlab_auth_token`|The access token of gitlab server.|true
`gitlab_base_url`|A middleware visible url pointing towards the gitlab.|true
`gitlab_domain`|The domain of ip of inbound gitlab webhook.|false
`gitlab_webhook_token_salt`|A salt used to enhance security. A default value will be used if not provided|false

A mysql DB needs to be set up too. The name should be `mysql` while the exact format is available [here](https://rocket.rs/v0.4/guide/state/#usage).

# Setting up gitlab

The backing gitlab server has to have these set:

1. An admin account and its access token.
2. Base url and stuff. 
3. You have to read [this](https://docs.gitlab.com/ce/security/webhooks.html). 
Since gitlab is completely hidden behind this middleware, 
and you are probably deploying these two together on the same machine,
you will probably want to turn that option on.

# Clustering

More than often, you don't have to worry about this middleware. 
It is highly unlikely for this part of the oj suite to get overloaded first.
Anyway, this middleware can scale up well with load balancer, since requests are all http.
The only thing that could be called a hard cap is this middleware is not supposed to handle
webhooks from multiple instances of gitlab, because different instances use different ids, ATs, etc.

# TODOs

1. Add tests.
1. Standardize error responding.
2. Finalize all APIs.
3. Minimize copying
4. DB setup script. Migrations. Stuff. Remove dependency over `mysql-client`, which bloat the image size 3 folds.

# Development notes

This is a mix of style guide, reminder and spec.

## Error handling
1. Unpredicted schema change should PANIC, with no effort to rollback changes.
2. UID/UUID not found should result in 404.
3. Malformed/Invalid requests, e.g. missing fields, invalid email, should be 400 but not guaranteed, could also be 500/404.
The only guarantee is nothing will be changed.
4. 500 otherwise.

## `APIFunction` vs `json!()`/string literal for outbound
When these criteria are met, prefer `APIFunction` over `json!()`/string literal.

1. Outbound message has to be composed from an inbound json message.
2. There is some invariant in outbound message. 
3. The same kind of message will be sent in multiple endpoints.

## Endpoint function style
    #[<method>("/endpoint/<dynamic>", ...)
    fn endpoint_name(<FromParam>, <FromData>,   // these should be necessary to compose outbound message
            <FromRequest>, <Configuration>      // these should be validated states and config
            DBAccess, State<GitlabAPI>) {       // DBAcess and outbound apis.

## Data notes
1. Admin has owner access to all groups. Admin is the owner of all projects. 

## Migrations
Manually create migration sql in `setup/` directories. 
Wrap the changes in a stored procedure.
Call the dependant procedure.
Add the source node in `entry-point.sh`, remove intermediate nodes as well.
 
# Web interface with backend

## Outbound

Please see [documentation at oj-backend](https://github.com/ShanghaitechGeekPie/oj-backend/blob/master/README.md#interface-with-oj-middleware).

## Inbound

### Several notes
Both course and assignment can't have display name among those listed [here](https://gitlab.com/gitlab-org/gitlab-ce/blob/master/lib/gitlab/path_regex.rb#L84-117).  
(Case insensitive)
###  `/users`
Request 

    POST /users
    {
        "email": "wangdch@shanghaitech.edu.cn",
        "password": "dummy"
    }

Response

    HTTP 202 Created 
    
Request 

    POST /users
    {
        "email": "wangdch",
        "password": "dummy"
    }

Response

    HTTP 400 Bad Request
    {"cause":"Invalid email"} 
    
Request 

    POST /users
    {
        "email": "wangdch@shanghaitech.edu.cn",
        "password": "dummy"
    }

Response

    HTTP 500 Bad Request

###  `/users/<user_email>/key`
Request 

    POST /users/wangdch%40shanghaitech.edu.cn/key
    {
        "key": "---BEGIN RSA KEY---....",
    }

Response

    HTTP 200 Ok 

###  `/courses`
Request 

    POST /courses
    {
        "name": "SI100c",
        "uuid": "00000000-0000-0000-0000-000000000000",
    }

Response

    HTTP 202 Created

###  `/courses/<course_uid>/instructors`
Request 

    POST /courses/00000000-0000-0000-0000-000000000000/instructors
    {
        "instructor_name": "chenhao@shanghaitech.edu.cn",
    }

Response

    HTTP 200 Ok

###  `/courses/<course_uid>/assignments`
Request 

    POST /courses/00000000-0000-0000-0000-000000000000/assignments
    {
        "name": "hw0",
        "uuid": "00000000-0000-0000-0000-000000000001",
    }

Response

    HTTP 202 Created 

###  `/courses/<course_uid>/assignments/<assignment_uid>/repos`
`additional_data` field is optional.
Request 

    POST /courses/00000000-0000-0000-0000-000000000000/assignments/00000000-0000-0000-0000-000000000000/repos
    {
        "owners": ["wangdch@shanghaitech.edu.cn"],
        "repo_name": "wangdch",
        "additional_data": "lolwhatsthis"
    }

Response

    HTTP 202 Created 

###  `/courses/<course_uid>/assignments/<assignment_uid>/repos/<repo_name>/download?format=<format>`
Possible argument for `<format>` is `tar.gz`, `tar.bz2`, `tbz`, `tbz2`, `tb2`, `bz2`, `tar`, and `zip`.
This would return HTTP error (500 or 404) if the underlying repo is empty.

Request 

    GET /courses/00000000-0000-0000-0000-000000000000/assignments/00000000-0000-0000-0000-000000000001/repos/wangdch/download?format=tar.gz

Response

    HTTP/1.1 200 OK 
    Content-Type: application/octet-stream
    Content-Disposition: attachment; filename="gitlab-pub01-master-0000000000000000000000000000000000000000.tar.gz"
    Etag: W/"66b236dce2a26ba5c409bcefead3a673"
    Content-Transfer-Encoding: binary
    <binary>

###  `/courses/<course_uid>/assignments/<assignment_uid>/repos/<repo_name>/commits?page=<page>`
page query prama should be omitted on first call. 
This would return HTTP error (500 or 404) if the underlying repo is empty.

Request 

    GET /courses/00000000-0000-0000-0000-000000000000/assignments/00000000-0000-0000-0000-000000000001/repos/wangdch/commits

Response

    HTTP/1.1 200 OK 
    Content-Type: application/json
    Link: <commits?page=http%3A%2F%2Flol%2Ffoo%2Fbar%3Fnext%3D102>; rel="next"
    <a large json>
    
Next page:

Request 

    GET /courses/00000000-0000-0000-0000-000000000000/assignments/00000000-0000-0000-0000-000000000001/repos/wangdch/commits?page=http%3A%2F%2Flol%2Ffoo%2Fbar%3Fnext%3D102

Response

    HTTP/1.1 200 OK 
    Content-Type: application/json
    <a large json>
    
Clients should make no assumption over the content of page. It should consider it to be something like a token that
has no meaning.
