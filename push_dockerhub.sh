export DOCKERHUB_USER="ourstudio"
export IMAGE_NAME="$DOCKERHUB_USER/rust-multi-solver-api"

docker login

docker buildx create --use --name multi || docker buildx use multi
docker buildx inspect --bootstrap

docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -f deploy/Dockerfile.multi \
  -t $IMAGE_NAME:latest \
  --push \
  .
