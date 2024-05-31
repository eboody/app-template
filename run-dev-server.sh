
#!/bin/bash

# Check if inside a tmux session
if [ -z "$TMUX" ]; then
    echo "You must be inside a tmux session to run this script."
    exit 1
fi

# Specify the Docker container name
CONTAINER_NAME="$(basename "$PWD")-postgres-1"

# Create a new tmux window
tmux new-window -n 'CargoWatch'

# Run Docker container check and start if not exists in the current pane
# tmux send-keys "docker ps -q --filter \"name=^/${CONTAINER_NAME}$\" | grep -q . || docker run --rm -d --name ${CONTAINER_NAME} -p 5432:5432 -e POSTGRES_PASSWORD=welcome postgres:16 &" C-m

# Wait for the Docker container to start
# tmux send-keys "while ! docker ps -q --filter \"status=running\" --filter \"name=^/${CONTAINER_NAME}$\" | grep -q .; do echo 'Waiting for Docker container ${CONTAINER_NAME} to start...'; sleep 1; done" C-m

# Split the window horizontally into two panes for cargo watch commands, ensuring variable is expanded
DOCKER_CHECK="while ! docker ps -q --filter \"status=running\" --filter \"name=^/${CONTAINER_NAME}$\" | grep -q .; do sleep 1; done;"
CARGO_CMD0="npm run dev --open --prefix frontend"
BUILD_TYPES="echo \"skipping types\""
CARGO_CMD1="cargo watch -w crates/services/web-server/src/ -w crates/libs/ -w .cargo/ -w sql/ -s \"${BUILD_TYPES} && cargo run -p web-server\" -i \"templates\" -i \"web-folder\""
CARGO_CMD2="cargo watch -q -c -w crates/services/web-server/examples/ -x \"run -p web-server --example quick_dev\""

tmux split-window -h
tmux send-keys "bash -c '${DOCKER_CHECK} ${CARGO_CMD0}'" C-m


tmux select-pane -t 0
tmux split-window -v
tmux send-keys "bash -c '${DOCKER_CHECK} ${CARGO_CMD1}'" C-m

tmux select-pane -t 1
tmux split-window -v
tmux send-keys "bash -c 'sleep 2; ${DOCKER_CHECK} ${CARGO_CMD2}'" C-m

tmux select-pane -t 1
tmux send-keys "bash -c 'sleep 2; ${DOCKER_CHECK} docker exec -it -u postgres ${CONTAINER_NAME} psql -d app_db -U app_user'" C-m
