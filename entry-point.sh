#!/usr/bin/env bash

if [[ -f $GITLAB_MIDDLEWARE_DB_PASS_FILE ]] ; then
    GITLAB_MIDDLEWARE_DB_PASS="$(cat $GITLAB_MIDDLEWARE_DB_PASS_FILE)"
fi

if [ -z $GITLAB_MIDDLEWARE_DB_PORT ] ; then
    GITLAB_MIDDLEWARE_DB_PORT=3306
fi

export ROCKET_DATABASES="{mysql={url=mysql://$GITLAB_MIDDLEWARE_DB_USER:$GITLAB_MIDDLEWARE_DB_PASS@$GITLAB_MIDDLEWARE_DB_HOST:$GITLAB_MIDDLEWARE_DB_PORT/$GITLAB_MIDDLEWARE_DB_NAME}}"

MIGRATIONS=$(ls setup/*.sql | awk '{print "source " $0 ";"}')

mysql -u $GITLAB_MIDDLEWARE_DB_USER -p$GITLAB_MIDDLEWARE_DB_PASS -h $GITLAB_MIDDLEWARE_DB_HOST -P $GITLAB_MIDDLEWARE_DB_PORT \
    -e "use ${GITLAB_MIDDLEWARE_DB_NAME}; ${MIGRATIONS}; call setup_1;"

cargo +nightly run --release --package oj-gitlab-middleware --bin oj-gitlab-middleware
