#!/usr/bin/env bash

#! Auto synced from Shared CI Resources repository
#! Don't change this file, instead change it in github.com/GaloyMoney/concourse-shared

set -euo pipefail

nix -L develop ./repo -c sh -exc '
set -euo pipefail

source pipeline-tasks/ci/vendor/tasks/helpers.sh

cd repo
make check-code
'
