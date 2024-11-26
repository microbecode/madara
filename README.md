<!-- markdownlint-disable -->
<div align="center">
  <img src="https://github.com/keep-starknet-strange/madara-branding/blob/main/logo/PNGs/Madara%20logomark%20-%20Red%20-%20Duotone.png?raw=true" width="500">
</div>
<div align="center">
<br />
<!-- markdownlint-restore -->

[![Workflow - Push](https://github.com/madara-alliance/madara/actions/workflows/push.yml/badge.svg)](https://github.com/madara-alliance/madara/actions/workflows/push.yml)
[![Project license](https://img.shields.io/github/license/madara-alliance/madara.svg?style=flat-square)](LICENSE)
[![Pull Requests welcome](https://img.shields.io/badge/PRs-welcome-ff69b4.svg?style=flat-square)](https://github.com/madara-alliance/madara/issues?q=is%3Aissue+is%3Aopen+label%3A%22help+wanted%22)
<a href="https://twitter.com/madara-alliance">
<img src="https://img.shields.io/twitter/follow/madara-alliance?style=social"/> </a>
<a href="https://github.com/madara-alliance/madara">
<img src="https://img.shields.io/github/stars/madara-alliance/madara?style=social"/>
</a>

</div>

# 🥷 Madara: Starknet Client

Madara is a powerful Starknet client written in Rust.

## Table of Contents

- ⬇️  [Installation](#%EF%B8%8F-installation)
  - [Run from Source](#run-from-source)
  - [Run with Docker](#run-with-docker)
- ⚙️  [Configuration](#%EF%B8%8F-configuration)
  - [Basic Command-Line Options](#basic-command-line-options)
  - [Environment variables](#environment-variables)
- 🌐 [Interactions](#-interactions)
  - [Supported JSON-RPC Methods](#supported-json-rpc-methods)
  - [Madara-specific JSON-RPC Methods](#madara-specific-json-rpc-methods)
  - [Example of Calling a JSON-RPC Method](#example-of-calling-a-json-rpc-method)
- ✅ [Supported Features](#-supported-features)
  - [Starknet Compliant](#starknet-compliant)
  - [Feeder-Gateway State Synchronization](#feeder-gateway-state-synchronization)
  - [State Commitment Computation](#state-commitment-computation)
  - [Analytics](#analytics)
- 💬 [Get in touch](#-get-in-touch)
  - [Contributing](#contributing)
  - [Partnerships](#partnerships)

## ⬇️ Installation

[⬅️  back to top](#-madara-starknet-client)

### Run from Source

#### 1. Install dependencies

   Ensure you have the necessary dependencies:

   | Dependency | Version    | Installation                                                                             |
   | ---------- | ---------- | ---------------------------------------------------------------------------------------- |
   | Rust       | rustc 1.81 | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh`                        |
   | Clang      | Latest     | `sudo apt-get install clang`                                                             |

   Once all dependencies are satisfied, you can clone the Madara repository:

   ```bash
   cd <your-destination-path>
   git clone https://github.com/madara-alliance/madara .
   ```

#### 2. Build Madara

   You can choose between different build modes:

   - **Debug** (low performance, fastest builds, _for testing purposes only_):

     ```bash
     cargo build
     ```

   - **Release** (fast performance, slower build times):

     ```bash
     cargo build --release
     ```

   - **Production** (fastest performance, _very slow build times_):

     ```bash
     cargo build --profile=production
     ```

#### 3. Run Madara

   Start the Madara client with a basic set of arguments depending on your chosen mode:

   **Full Node**

   A full node, synchronizing the state of the chain from genesis.

   ```bash
   cargo run --release --        \
     --name Madara               \
     --full                      \
     --base-path /var/lib/madara \
     --network mainnet           \
     --l1-endpoint ${ETHEREUM_API_URL}
   ```

   **Sequencer**

   Produces new blocks for other nodes to synchronize.

   ```bash
   cargo run --release --        \
     --name Madara               \
     --sequencer                 \
     --base-path /var/lib/madara \
     --preset test               \
     --l1-endpoint ${ETHEREUM_API_URL}
   ```

   **Devnet**

   A node in a private local network.

   ```bash
   cargo run --release --        \
     --name Madara               \
     --devnet                    \
     --base-path /var/lib/madara \
     --preset test
   ```

> [!NOTE]
> Head to the [Configuration](#configuration) section to learn how to
> customize your node parameters.

#### 4. Presets

   You can use cli presets for certain common node configurations, for example
   enabling rpc endpoints:

   ```
   cargo run --release -- \
      --name Madara       \
      --full              \
      --preset mainnet    \
      --rpc
   ```

   ...or the madara feeder gateway:

   ```
   cargo run --release -- \
      --name Madara       \
      --full              \
      --preset mainnet    \
      --fgw
   ```

---

### Run with Docker

#### 1. Manual Setup

   Ensure you have [Docker](https://docs.docker.com/engine/install/) installed
   on your machine. Once you have Docker installed, you will need to pull the
   madara image from the github container registry (ghr):

   ```bash
   docker pull ghcr.io/madara-alliance/madara:latest
   docker tag ghcr.io/madara-alliance/madara:latest madara:latest
   docker rmi ghcr.io/madara-alliance/madara:latest
   ```

   You can then launch madara as follows:

   ```bash
   docker run -d                    \
     -p 9944:9944                   \
     -v /var/lib/madara:/tmp/madara \
     --name Madara                  \
     madara:latest                  \
     --name Madara                  \
     --full                         \
     --network mainnet              \
     --l1-endpoint ${ETHEREUM_API_URL}
   ```

   To display the node's logs, you can use:

   ```bash
   docker logs -f -n 100 Madara
   ```

> [!WARNING]
> Make sure to change the volume `-v` of your container if ever you update
> `--base-path`.


#### 2. Using the project Makefile

   Alternatively, you can use the provided Makefile and `compose.yaml` to
   simplify this process.

> [!IMPORTANT]
> This requires you to have [Docker Compose](https://docs.docker.com/compose/install/)
> installed

   Start by saving your rpc key to a `.secrets` forlder:

   ```bash
   mkdir .secrets
   echo *** .secrets/rpc_api.secret
   ```

   Then, run madara with the following commands:

   ```bash
   make start    # This will automatically pull the madara image if not available
   make logs     # Displays the last 100 lines of logs
   make stop     # Stop the madara node
   make clean-db # Removes the madara db, including files on the host
   make restart  # Restarts the madara node
   ```

   To change runtime arguments, you can update the script in `madara-runner.sh`:

   ```bash
   #!/bin/sh
   export RPC_API_KEY=$(cat $RPC_API_KEY_FILE)

   ./madara                   \
     --name madara            \
     --network mainnet        \
     --rpc-external           \
     --rpc-cors all           \
     --full                   \
     --l1-endpoint $RPC_API_KEY
   ```

   For more information, run:

   ```bash
   make help
   ```

> [!TIP]
> When running Madara from a docker container, make sure to set options such
> as `--rpc-external`, `--gateway-external` and `--rpc-admin-external` so as
> to be able to access these services from outside the container.


## ⚙️ Configuration

[⬅️  back to top](#-madara-starknet-client)

For a comprehensive list of all command-line options, check out:

```bash
cargo run -- --help
```

Or if you are using docker, simply:

```bash
docker run madara:latest --help
```

---

### Basic Command-Line Options

Here are some recommended options to get up and started with your Madara client:

| Option | About |
| ------ | ----- |
| **`--name <NAME>`** | The human-readable name for this node. It's used as the network node name. |
| **`--base-path <PATH>`** | Sets the database location for Madara (default is`/tmp/madara`) |
| **`--full`** | The mode of your Madara client (either `--sequencer`, `--full`, or `devnet`) |
| **`--l1-endpoint <URL>`** | The Layer 1 endpoint the node will verify its state from |
| **`--rpc-port <PORT>`** | The JSON-RPC server TCP port, used to receive requests |
| **`--rpc-cors <ORIGINS>`** | Browser origins allowed to make calls to the RPC servers |
| **`--rpc-external`** | Exposes the rpc service on `0.0.0.0` |

---

### Environment Variables

Each cli argument has its own corresponding environment variable you can set to
change its value. For example:

- `MADARA_BASE_PATH=/path/to/data`
- `MADARA_RPC_PORT=1111`

These variables allow you to adjust the node's configuration without using
command-line arguments, which can be useful in CI pipelines or with docker.


> [!NOTE]
> If the command-line argument is specified then it takes precedent over the
> environment variable.

## 🌐 Interactions

[⬅️  back to top](#-madara-starknet-client)

Madara fully supports all the JSON-RPC methods as of the latest version of the
Starknet mainnet official [JSON-RPC specs](https://github.com/starkware-libs/starknet-specs).
These methods can be categorized into three main types: Read-Only Access Methods,
Trace Generation Methods, and Write Methods. They are accessible through port
**9944** unless specified otherwise with `--rpc-port`.

> [!TIP]
> You can use the special `rpc_methods` call to receive a list of all the
> methods which are available on an endpoint.

---

### Supported JSON-RPC Methods

Here is a list of all the supported methods with their current status:

<details>
  <summary>Read Methods</summary>

| Status | Method                                     |
| ------ | ------------------------------------------ |
| ✅     | `starknet_specVersion`                     |
| ✅     | `starknet_getBlockWithTxHashes`            |
| ✅     | `starknet_getBlockWithTxs`                 |
| ✅     | `starknet_getBlockWithReceipts`            |
| ✅     | `starknet_getStateUpdate`                  |
| ✅     | `starknet_getStorageAt`                    |
| ✅     | `starknet_getTransactionStatus`            |
| ✅     | `starknet_getTransactionByHash`            |
| ✅     | `starknet_getTransactionByBlockIdAndIndex` |
| ✅     | `starknet_getTransactionReceipt`           |
| ✅     | `starknet_getClass`                        |
| ✅     | `starknet_getClassHashAt`                  |
| ✅     | `starknet_getClassAt`                      |
| ✅     | `starknet_getBlockTransactionCount`        |
| ✅     | `starknet_call`                            |
| ✅     | `starknet_estimateFee`                     |
| ✅     | `starknet_estimateMessageFee`              |
| ✅     | `starknet_blockNumber`                     |
| ✅     | `starknet_blockHashAndNumber`              |
| ✅     | `starknet_chainId`                         |
| ✅     | `starknet_syncing`                         |
| ✅     | `starknet_getEvents`                       |
| ✅     | `starknet_getNonce`                        |

</details>

<details>
  <summary>Trace Methods</summary>

| Status | Method                            |
| ------ | --------------------------------- |
| ✅     | `starknet_traceTransaction`       |
| ✅     | `starknet_simulateTransactions`   |
| ✅     | `starknet_traceBlockTransactions` |

</details>

<details>
  <summary>Write Methods</summary>

| Status | Method                                 |
| ------ | -------------------------------------- |
| ✅     | `starknet_addInvokeTransaction`        |
| ✅     | `starknet_addDeclareTransaction`       |
| ✅     | `starknet_addDeployAccountTransaction` |

</details>

### Madara-specific JSON-RPC Methods

Beside this, Madara supports its own set of custom extensions to the starknet
specs. These are referred to as `admin` methods. They are exposed on a separate
port **9943** unless specified otherwise with `--rpc-admin-port`.

<details>
  <summary>Write Methods</summary>

| Method                          | About                                             |
| ------------------------------- | ------------------------------------------------- |
|`madara_addDeclareV0Transaction` | Adds a legacy Declare V0 Transaction to the state |

</details>

<details>
  <summary>Status Methods</summary>

| Method              | About                                                |
| --------------------| ---------------------------------------------------- |
| `madara_ping`       | Return the unix time at which this method was called |
| `madara_stopNode`   | Gracefully stops the running node                    |
| `madara_rpcDisable` | Disables user-facing rpc services                    |
| `madara_rpcEnable`  | Enables user-facing rpc services                     |
| `madara_rpcRestart` | Restarts user-facing rpc services                    |
| `madara_syncDisable`| Disables l1 and l2 sync services                     |
| `madara_syncEnable` | Enables l1 and l2 sync services                      |
| `madara_syncRestart`| Restarts l1 and l2 sync services                     |

</details>

> [!CAUTION]
> These methods are exposed on `locahost` by default for obvious security
> reasons. You can always exposes them externally using `--rpc-admin-external`,
> but be _very careful_ when doing so as you might be compromising your node!
> Madara does not do **any** authorization checks on the caller of these
> methods and instead leaves it up to the user to set up their own proxy to
> handle these situations.

---

### Example of Calling a JSON-RPC Method

Here is an example of how to call a JSON-RPC method using Madara. Before running
the bellow code, make sure you have a node running with rpc enabled on port 9944.

```bash
curl --location 'localhost:9944'            \
  --header 'Content-Type: application/json' \
  --data '{
    "jsonrpc": "2.0",
    "method": "rpc_methods",
    "params": [],
    "id": 1
  }' | jq --sort-keys
```

You can use any JSON-RPC client to interact with Madara, such as `curl`,
`httpie`, or a custom client in your preferred programming language. For more
detailed information on each method, please refer to the
[Starknet JSON-RPC specs](https://github.com/starkware-libs/starknet-specs).

> [!NOTE]
> Write methods are forwarded to the Sequencer and are not executed by Madara.
> These might fail if you provide the wrong arguments or in case of a
> conflicting state. Make sure to refer to the
> [Starknet JSON-RPC specs](https://github.com/starkware-libs/starknet-specs)
> for a list of potential errors.

## ✅ Supported Features

[⬅️  back to top](#-madara-starknet-client)

### Starknet compliant

Madara is compliant with the latest `v0.13.2` version of Starknet and `v0.7.1`
JSON-RPC specs. You can find out more about this in the [interactions](#-interactions)
section or at the official Starknet [JSON-RPC specs](https://github.com/starkware-libs/starknet-specs).

### Feeder-Gateway State Synchronization

Madara supports its own implementation of the Starknet feeder gateway, which
allows nodes to synchronize state from each other at much faster speeds than
a regular sync.

> [!NOTE]
> Starknet does not currently have a specification for its feeder-gateway
> protocol, so despite our best efforts at output parity, you might still notice
> some discrepancies between official feeder gateway endpoints and our own
> implementation.

### State Commitment Computation

Madara supports merkelized state verification through its own implementation of
Besu Bonsai Merkle Tries. See the [bonsai lib](https://github.com/madara-alliance/bonsai-trie).
You can read more about Starknet Block structure and how it affects state
commitment [here](https://docs.starknet.io/architecture-and-concepts/network-architecture/block-structure/).

### Analytics

Madara comes ready out of the box with Open Telemetry version `v0.25.0`
integration, supporting export of traces, metrics and logs.

#### Running Madara with Signoz as a dashboard

First, [install Signoz]((https://signoz.io/docs/install/docker/#install-signoz-using-docker-compose)):

```bash
git clone -b main https://github.com/SigNoz/signoz.git && cd signoz/deploy/
docker compose -f docker/clickhouse-setup/docker-compose.yaml up -d
docker ps
```

Wait for the above command to complete: you should see an output similar to the
following:

```bash
CONTAINER ID   IMAGE                                          COMMAND                  CREATED          STATUS                    PORTS                                                                            NAMES
01f044c4686a   signoz/frontend:0.38.2                       "nginx -g 'daemon of…"   2 minutes ago   Up 9 seconds                  80/tcp, 0.0.0.0:3301->3301/tcp                                                     signoz-frontend
86aa5b875f9f   gliderlabs/logspout:v3.2.14                  "/bin/logspout syslo…"   2 minutes ago   Up 1 second                   80/tcp                                                                             signoz-logspout
58746f684630   signoz/alertmanager:0.23.4                   "/bin/alertmanager -…"   2 minutes ago   Up 9 seconds                  9093/tcp                                                                           signoz-alertmanager
2cf1ec96bdb3   signoz/query-service:0.38.2                  "./query-service -co…"   2 minutes ago   Up About a minute (healthy)   8080/tcp                                                                           signoz-query-service
e9f0aa66d884   signoz/signoz-otel-collector:0.88.11          "/signoz-collector -…"   2 minutes ago   Up 10 seconds                 0.0.0.0:4317-4318->4317-4318/tcp                                                   signoz-otel-collector
d3d89d7d4581   clickhouse/clickhouse-server:23.11.1-alpine   "/entrypoint.sh"         2 minutes ago   Up 2 minutes (healthy)        0.0.0.0:8123->8123/tcp, 0.0.0.0:9000->9000/tcp, 0.0.0.0:9181->9181/tcp, 9009/tcp   signoz-clickhouse
9db88aefb6ed   signoz/locust:1.2.3                          "/docker-entrypoint.…"   2 minutes ago   Up 2 minutes                  5557-5558/tcp, 8089/tcp                                                            load-hotrod
60bb3b77b4f7   bitnami/zookeeper:3.7.1                      "/opt/bitnami/script…"   2 minutes ago   Up 2 minutes                  0.0.0.0:2181->2181/tcp, 0.0.0.0:2888->2888/tcp, 0.0.0.0:3888->3888/tcp, 8080/tcp   signoz-zookeeper-1
98c7178b4004   jaegertracing/example-hotrod:1.30            "/go/bin/hotrod-linu…"   9 days ago      Up 2 minutes                  8080-8083/tcp                                                                      hotrod
```

Next, navigate to your [signoz dashboard](http://localhost:3301). If you are
running Madara on a remote server this will be `http://your-server-ip:3301`.
Create an admin login, then go to `Dashboards` in the left drawer and click on
`New dashboard`->`Import JSON` and copy over the contents of
[infra/Signoz/dashboards/overview.json](https://github.com/madara-alliance/madara/blob/docs/readme/infra/Signoz/dashboards/overview.json).

Finally, run Madara with analytics enabled and refresh your Signoz dashboard.

```bash
cargo run --release --                                  \
  --name madara                                         \
  --network mainnet                                     \
  --full                                                \
  --l1-endpoint ***                                     \
  --analytics-collection-endpoint http://localhost:4317 \
  --analytics-service-name Madara
```

## 💬 Get in touch

[⬅️  back to top](#-madara-starknet-client)

### Contributing

For guidelines on how to contribute to Madara, please see the [Contribution Guidelines](https://github.com/madara-alliance/madara/blob/main/CONTRIBUTING.md).

###  Partnerships

To establish a partnership with the Madara team, or if you have any suggestions or
special requests, feel free to reach us on [Telegram](https://t.me/madara-alliance).

### License

Madara is open-source software licensed under the
[Apache-2.0 License](https://github.com/madara-alliance/madara/blob/main/LICENSE).
