# gitlab-middleware

## Overview

This middleware translate gitlab webhook push into digestible format to be pulled by scheduler.

[Rust](https://rust-lang.org/) and [Rocket](https://rocket.rs/) are used because, why not?

## Web interface

Post to `https?://(?P<host>[^/]+)/hooks/(?P<courseId>\d+)/(?P<assignmentId>\d+)` with a valid payload. 
Expecting gitlab push event only.

## Redis storage format

On receiving event, first parse the upstream url from json payload. 

Then, add the url to the redis with current timestamp as score using `ZADD` under key `<courseId>:<assignmentId>`.

## Building & Deploying

This service is expected to be built and deployed with docker. 

## Configuring

This is a Rocket application, so visit its [document](https://rocket.rs/v0.4/guide/configuration/#environment-variables)
to know how to configure via environment variables.

In addition, supply at least one of these Rocket configuration element or face a panic! 
If you however set environment variable `mute_security=true`, it won't work, as you are supposed to set `ROCKET_MUTE_SECURITY=true`. 

token name|description
---|---
`gitlab_token`|[GitLab Webhooks Secret token](https://docs.gitlab.com/ee/user/project/integrations/webhooks.html#secret-token)
`gitlab_domain`|The domain of gitlab server. Notice that you can add multiple A record for same domain name.
`mute_security`|set to True to bypass this check

## TODOs

1. Add tests.
2. Perhaps add inbound payload validation instead of just using regular expression?
