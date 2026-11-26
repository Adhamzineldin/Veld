# Broker tests

Toolchain tests of the broker adapters of events phase E2 (`spec/events.md` section 10.7). Each
target has a program that drives its generated adapter through the rows of the matrix of
section 10.1 against a real broker: publish and consume, the key, the order per key, a handler
that fails and is delivered again, `@ordered`, the dead-letter topic, a message published before
the group subscribed, two consumers of one group, `close()`, and (Kafka) a delivery that outlives
the session. Every topic, queue and group name is new for each run, so the brokers can stay up
between runs.

The tests run only when the broker is named by an environment variable. Without it they are
skipped; with `VELD_REQUIRE_BROKERS=1` a missing variable is a failure.

| Variable | Value | Broker |
|---|---|---|
| `VELD_KAFKA` | bootstrap servers, `localhost:9092` | Kafka |
| `VELD_RABBITMQ` | an AMQP URL, `amqp://guest:guest@localhost:5672` | RabbitMQ |
| `VELD_REQUIRE_BROKERS` | `1` makes a missing variable a failure | both |

## Starting the brokers

```bash
# Kafka (KRaft, one node). The advertised listener must be what the test process connects to.
docker run -d --rm --name veld-kafka -p 9092:9092 apache/kafka-native:3.9.0

# RabbitMQ, with the management plugin (port 15672) for looking at queues and exchanges.
docker run -d --rm --name veld-rabbitmq -p 5672:5672 -p 15672:15672 rabbitmq:4-management-alpine

export VELD_KAFKA=localhost:9092
export VELD_RABBITMQ=amqp://guest:guest@localhost:5672
export VELD_REQUIRE_BROKERS=1
```

`apache/kafka-native:3.9.0` starts with a listener on `localhost:9092` and creates topics
automatically; the Kafka test creates its own topics with the admin client anyway, because the
adapter creates nothing. A topic that the test creates has two or three partitions.

## Node (`crates/emitters/node`)

| | |
|---|---|
| Programs | `tests/toolchain/ts/events_kafka.ts` (`kafkajs` 2.2.4) and `events_rabbitmq.ts` (`amqplib` 2.4.2) |
| Runner | `cargo test -p veld-emit-node --test brokers` |
| Generated with | `messaging = ["kafka", "rabbitmq"]` on `tests/events/delivery.veld`, compiled with `tsc --strict` for `es2020` and `es2022` |

The adapters are always compiled, with the Node types, even when no broker is named. The
programs print `events kafka ok` and `events rabbitmq ok` when every row holds. The Kafka program
takes about 40 seconds (one row keeps a delivery running for nine seconds, longer than the
smallest session timeout of the broker).

## Java

(The Java agent adds its section here.)

## Rust

(The Rust agent adds its section here.)
