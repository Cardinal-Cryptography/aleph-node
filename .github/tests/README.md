# Tests for GitHub workflows

## Introduction
This directory contains scripts for testing CI workflows that can be found in the
`.github/workflows/` directory.  

Each workflow has a simple test written in bash which checks the result of running
the workflow.  Workflow is executed locally using a tool called (act)[https://github.com/nektos/act].

Jobs inside the workflow require docker, S3 buckets and other resources, and these are started in
docker containers just before the run.  No job should have hardcoded values, and variables or
secrets should be used instead so that we can point to resources set temporarily on a local computer.

Have a butcher's at `run-workflow-locally.sh` file to get more details.


## Requirements
The following software is required to run workflows locally and test them:

* docker
* (act)[https://github.com/nektos/act]

At the time of writing this README, act 0.2.43 and docker 23.0.0 have been used so
these version can be considered as working.


## Running a test locally
Run the following bash script from the main directory in the repository with two arguments:

* absolute path to workflow YAML file as its argument
* absolute path to `tests` directory

    ./.github/tests/run-workflow-locally.sh \
      $(pwd)/.github/workflows/build-and-push-cliain.yml \
      $(pwd)/.github/tests

