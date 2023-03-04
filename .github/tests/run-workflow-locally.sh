#!/usr/bin/env bash

set -euo pipefail


DOCKER_NETWORK=a0-act
DOCKER_NETWORK_SUBNET=172.66.0.0/16
DOCKER_REGISTRY=a0-act-alephnode-registry
DOCKER_REGISTRY_IP=172.66.0.2
DOCKER_REGISTRY_PORT=5000


function usage() {
  cat << EOF
Usage:
  $0
    --workflow-path path
        path to a specific workflow yaml file
    --tests-path path
        path to directory with tests
EOF
}

function cleanup() {
  delete_dockers
  exit 0
}

function create_dockers() {
  echo "Creating docker network called ${DOCKER_NETWORK}..."
  if [[ $(docker network list | grep "${DOCKER_NETWORK}" | wc -l) == 0 ]]; then
    docker network create --subnet="${DOCKER_NETWORK_SUBNET}" ${DOCKER_NETWORK}
  fi

  echo "Creating docker containers..."
  if [[ $(docker ps -a | grep "${DOCKER_REGISTRY}" | wc -l) == 1 ]]; then
    docker rm -f ${DOCKER_REGISTRY}
  fi
  docker run -d -p ${DOCKER_REGISTRY_PORT}:${DOCKER_REGISTRY_PORT} --restart always --name ${DOCKER_REGISTRY} --network ${DOCKER_NETWORK} --ip ${DOCKER_REGISTRY_IP} registry:2
}

function delete_dockers() {
  echo "Deleting all created docker containers..."
  docker rm -f ${DOCKER_REGISTRY}
}

function run_act() {
  # act already adds '-v /var/run/docker.sock:/var/run/docker.sock' when starting a container
  act -P self-hosted=mikogs/tmp-github-runner:1.0.0 -b --insecure-secrets \
    --container-architecture=linux/amd64 \
    -W "$1" \
    -s ECR_PUBLIC="http://${DOCKER_REGISTRY_IP}:${DOCKER_REGISTRY_PORT}" -s AWS_MAINNET_ACCESS_KEY_ID="mikogs" -s AWS_MAINNET_SECRET_ACCESS_KEY="gen64" \
    -s ECR_REPO_CLIAIN="${DOCKER_REGISTRY_IP}:${DOCKER_REGISTRY_PORT}/cliain" \
    --env 'DOCKER_BUILDKIT=0' \
    --container-options "--network ${DOCKER_NETWORK}  --privileged"
}

function run_test() {
  if [[ ! -z "$2" ]]; then
    TEST_FILENAME=$(basename "$1")
    echo ""
    echo ""
    echo "Looking for a test-${TEST_FILENAME} in $2 ..."
    if [[ -f "$2/test-${TEST_FILENAME}" ]]; then
      act -P self-hosted=mikogs/tmp-github-runner:1.0.0 -b --insecure-secrets \
      --container-architecture=linux/amd64 \
      -W "$2/test-${TEST_FILENAME}" \
      -s ECR_PUBLIC="http://${DOCKER_REGISTRY_IP}:${DOCKER_REGISTRY_PORT}" -s AWS_MAINNET_ACCESS_KEY_ID="mikogs" -s AWS_MAINNET_SECRET_ACCESS_KEY="gen64" \
      -s ECR_REPO_CLIAIN="${DOCKER_REGISTRY_IP}:${DOCKER_REGISTRY_PORT}/cliain" \
      --env 'DOCKER_BUILDKIT=0' \
      --container-options "--network ${DOCKER_NETWORK}  --privileged"
    fi
  fi
}


WORKFLOW_PATH="${1:-}"
if [[ -z "${WORKFLOW_PATH}" || ! -f "${WORKFLOW_PATH}" ]]; then
  echo "Workflow path is not specified or file does not exist! Exiting."
  echo ""
  usage
  exit 1                    
fi
TESTS_PATH="${2:-}"
if [[ ! -z "${TESTS_PATH}" && ! -d "${TESTS_PATH}" ]]; then
  echo "Tests path is not a valid directory! Exiting."
  echo ""
  usage
  exit 1
fi

create_dockers
run_act "${WORKFLOW_PATH}"
run_test "${WORKFLOW_PATH}" "${TESTS_PATH}"
delete_dockers