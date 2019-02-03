#!/usr/bin/env bash

export ROCKET_DATABASE_MYSQL_URL="mysql://$GITLAB_MIDDLEWARE_DB_USER:$GITLAB_MIDDLEWARE_DB_PASS@$GITLAB_MIDDLEWARE_DB_HOST:$GITLAB_MIDDLEWARE_DB_PORT/$GITLAB_MIDDLEWARE_DB_NAME"

MIGRATIONS=$(ls setup | awk '{print "source " $0 ";"}')

mysql -u $GITLAB_MIDDLEWARE_DB_USER -p $GITLAB_MIDDLEWARE_DB_PASS $GITLAB_MIDDLEWARE_DB_HOST:$GITLAB_MIDDLEWARE_DB_PORT \
    -e "use ${GITLAB_MIDDLEWARE_DB_NAME}; $MIGRATIONS; exec setup_1;"

cargo +nightly run --release --package oj-gitlab-middleware --bin oj-gitlab-middleware
