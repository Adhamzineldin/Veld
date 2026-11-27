package dev.veld.runtime;

import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;

/**
 * The transport that lives in the process, for tests and for development (spec/events.md section
 * 9). A message goes to one subscriber of each group that was subscribed when it was published; a
 * topic nobody subscribed to drops it. A group gets its messages in publish order, one at a time,
 * on a daemon thread of its own, so the order per key holds. A failed handler gets the message
 * again at once, up to {@code maxAttempts}; then it goes to {@link #deadLetters()}.
 */
public final class MemoryTransport implements Transport {

  /**
   * A message the transport gave up on after {@code maxAttempts} deliveries.
   *
   * @param topic the topic
   * @param group the consumer group that never acknowledged it
   * @param key the key text, or {@code null}
   * @param bytes the bytes of the message
   * @param attempts how often it was delivered
   */
  public record DeadLetter(String topic, String group, String key, byte[] bytes, int attempts) {}

  private record Item(String key, byte[] bytes) {}

  private static final class Member {
    private final Deliver deliver;

    Member(Deliver deliver) {
      this.deliver = deliver;
    }
  }

  private static final class Group {
    private final String topic;
    private final String name;
    private final List<Member> members = new ArrayList<>();
    private final ArrayDeque<Item> queue = new ArrayDeque<>();
    private int next;
    private boolean running;

    Group(String topic, String name) {
      this.topic = topic;
      this.name = name;
    }
  }

  private final int maxAttempts;
  private final Object lock = new Object();
  private final Map<String, Map<String, Group>> topics = new LinkedHashMap<>();
  private final List<DeadLetter> dead = new ArrayList<>();
  private int pending;

  /** A transport that delivers a message up to 3 times while its handler fails. */
  public MemoryTransport() {
    this(3);
  }

  /** A transport that delivers a message up to {@code maxAttempts} times (at least once). */
  public MemoryTransport(int maxAttempts) {
    this.maxAttempts = Math.max(1, maxAttempts);
  }

  @Override
  public void publish(String topic, String key, byte[] bytes) {
    Objects.requireNonNull(topic, "topic");
    byte[] copy = bytes.clone();
    synchronized (lock) {
      Map<String, Group> groups = topics.get(topic);
      if (groups == null) {
        return;
      }
      for (Group group : groups.values()) {
        group.queue.add(new Item(key, copy));
        pending++;
        if (!group.running) {
          group.running = true;
          start(group);
        }
      }
    }
  }

  /** The memory transport ignores the options: it is already ordered, and a dead-letter topic is a topic. */
  @Override
  public void publish(String topic, String key, byte[] bytes, ChannelOptions options) {
    publish(topic, key, bytes);
  }

  /** The memory transport ignores the options. */
  @Override
  public Subscription subscribe(String topic, String group, Deliver deliver, ChannelOptions options) {
    return subscribe(topic, group, deliver);
  }

  @Override
  public Subscription subscribe(String topic, String group, Deliver deliver) {
    Objects.requireNonNull(deliver, "deliver");
    Member member = new Member(deliver);
    Group joined;
    synchronized (lock) {
      Map<String, Group> groups = topics.computeIfAbsent(topic, t -> new LinkedHashMap<>());
      joined = groups.computeIfAbsent(group, g -> new Group(topic, group));
      joined.members.add(member);
    }
    return () -> leave(joined, member);
  }

  /** The messages that were delivered {@code maxAttempts} times and never acknowledged. */
  public List<DeadLetter> deadLetters() {
    synchronized (lock) {
      return List.copyOf(dead);
    }
  }

  /**
   * Returns when every published message has been delivered or dead-lettered, the messages that
   * handlers publish included. Do not call it from a handler.
   *
   * @throws InterruptedException when the waiting thread is interrupted
   */
  public void idle() throws InterruptedException {
    synchronized (lock) {
      while (pending > 0) {
        lock.wait();
      }
    }
  }

  private void leave(Group group, Member member) {
    synchronized (lock) {
      group.members.remove(member);
      if (group.members.isEmpty()) {
        Map<String, Group> groups = topics.get(group.topic);
        if (groups != null && groups.get(group.name) == group) {
          groups.remove(group.name);
        }
      }
    }
  }

  private void start(Group group) {
    Thread thread = new Thread(() -> drain(group), "veld-memory-transport-" + group.topic + "-" + group.name);
    thread.setDaemon(true);
    thread.start();
  }

  private void drain(Group group) {
    while (true) {
      Item item;
      synchronized (lock) {
        item = group.queue.poll();
        if (item == null) {
          group.running = false;
          return;
        }
      }
      try {
        deliver(group, item);
      } finally {
        settle();
      }
    }
  }

  private void deliver(Group group, Item item) {
    for (int attempt = 1; ; attempt++) {
      Member member = pick(group);
      if (member == null) {
        return;
      }
      Outcome outcome;
      try {
        outcome = member.deliver.deliver(new Message(item.key(), item.bytes().clone(), attempt));
      } catch (RuntimeException | Error e) {
        outcome = Outcome.AGAIN;
      }
      if (outcome == Outcome.DONE) {
        return;
      }
      if (attempt >= maxAttempts) {
        synchronized (lock) {
          dead.add(new DeadLetter(group.topic, group.name, item.key(), item.bytes(), attempt));
        }
        return;
      }
    }
  }

  private Member pick(Group group) {
    synchronized (lock) {
      int count = group.members.size();
      if (count == 0) {
        return null;
      }
      group.next %= count;
      Member member = group.members.get(group.next);
      group.next = (group.next + 1) % count;
      return member;
    }
  }

  private void settle() {
    synchronized (lock) {
      pending--;
      if (pending == 0) {
        lock.notifyAll();
      }
    }
  }
}
