#!/bin/bash

set -e

NAME=aleph-node
AWS_ACCOUNT_ID=436875894086
TAG=latest

# BUILD

IMG=$NAME:$TAG

docker build --tag $IMG -f ./docker/Dockerfile .

# authenticate docker to use aws ecr registry
aws ecr get-login-password --region $API_AWS_REGION | docker login --username AWS --password-stdin $AWS_ACCOUNT_ID.dkr.ecr.$API_AWS_REGION.amazonaws.com
# tag image
docker tag $IMG $AWS_ACCOUNT_ID.dkr.ecr.$API_AWS_REGION.amazonaws.com/$NAME:$TAG
# push to ECR
docker push $AWS_ACCOUNT_ID.dkr.ecr.$API_AWS_REGION.amazonaws.com/$NAME:$TAG

echo "Done"
exit $?
