package dev.veld.runtime;

/**
 * One validation problem.
 *
 * @param path where the problem is, for example {@code body.items[0].sku}
 * @param code a stable machine-readable code such as {@code required} or {@code min_length}
 * @param message a human-readable description
 */
public record Issue(String path, String code, String message) {}
