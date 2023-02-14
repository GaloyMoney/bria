#!/bin/bash

set -eu

. pipeline-tasks/ci/vendor/tasks/helpers.sh

pushd repo
make e2e-tests-in-container
